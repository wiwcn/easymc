use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use easymc_core::{EasyMcError, McVersion, Paths, Result};
use serde::Serialize;
use tokio::task::JoinSet;
use zip::ZipArchive;

use crate::download::{download_file, FetchResult};
use crate::rules::{is_native_library, rules_allow};
use crate::types::{AssetIndex, Library, VersionInfo};
use crate::{MetaProvider, PistonMetaProvider};

/// Progress event emitted during `install_version`.
#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    /// Machine stage id: meta | client | libraries | natives | assets | done | error
    pub stage: String,
    /// zh-CN label for the stage
    pub stage_label: String,
    /// 1-based phase index among planned phases
    pub phase: u32,
    pub phase_total: u32,
    pub current: u64,
    pub total: Option<u64>,
    /// 0–100 for the current stage when `total` is known
    pub percent: Option<f32>,
    pub detail: String,
    /// Files already present and skipped in this stage
    pub skipped: u64,
    /// Files newly downloaded in this stage
    pub downloaded: u64,
}

impl InstallProgress {
    pub fn new(
        stage: &str,
        phase: u32,
        phase_total: u32,
        current: u64,
        total: Option<u64>,
        detail: impl Into<String>,
    ) -> Self {
        let percent = total.filter(|&t| t > 0).map(|t| {
            ((current as f64 / t as f64) * 100.0).clamp(0.0, 100.0) as f32
        });
        Self {
            stage: stage.into(),
            stage_label: stage_label(stage).into(),
            phase,
            phase_total,
            current,
            total,
            percent,
            detail: detail.into(),
            skipped: 0,
            downloaded: 0,
        }
    }

    pub fn with_counts(mut self, skipped: u64, downloaded: u64) -> Self {
        self.skipped = skipped;
        self.downloaded = downloaded;
        self
    }

    /// One-line CLI / log friendly summary (zh-CN).
    pub fn format_line(&self) -> String {
        let pct = self
            .percent
            .map(|p| format!("{p:.0}%"))
            .unwrap_or_else(|| "—".into());
        let frac = match self.total {
            Some(t) => format!("{}/{}", self.current, t),
            None => format!("{}", self.current),
        };
        let mut extra = String::new();
        // Avoid duplicating counts when `detail` already summarizes them.
        if (self.skipped > 0 || self.downloaded > 0)
            && !self.detail.contains("跳过")
            && !self.detail.contains("新下")
        {
            extra = format!(" · 新下 {} / 跳过 {}", self.downloaded, self.skipped);
        }
        format!(
            "[{}/{} {}] {pct}  {frac}  {}{extra}",
            self.phase, self.phase_total, self.stage_label, self.detail
        )
    }
}

fn stage_label(stage: &str) -> &'static str {
    match stage {
        "meta" => "元数据",
        "client" => "客户端",
        "libraries" => "依赖库",
        "natives" => "本地库",
        "assets" => "游戏资源",
        "done" => "完成",
        "error" => "失败",
        _ => "进行中",
    }
}

pub type ProgressFn = Box<dyn Fn(InstallProgress) + Send + Sync>;

pub struct InstallOptions {
    /// Download asset objects (required for a full title-screen launch). Default true.
    pub assets: bool,
    /// Extract natives-* jars into natives/<version>. Default true.
    pub natives: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            assets: true,
            natives: true,
        }
    }
}

fn phase_plan(opts: &InstallOptions) -> u32 {
    let mut n = 3u32; // meta + client + libraries
    if opts.natives {
        n += 1;
    }
    if opts.assets {
        n += 1;
    }
    n
}

fn map_stage_err(stage: &str, e: EasyMcError) -> EasyMcError {
    EasyMcError::msg(format!(
        "安装失败（阶段：{}）：{e}",
        stage_label(stage)
    ))
}

pub async fn install_version(
    paths: &Paths,
    version: &McVersion,
    opts: InstallOptions,
    on_progress: impl Fn(InstallProgress) + Send + Sync,
) -> Result<VersionInfo> {
    paths.ensure()?;
    let phase_total = phase_plan(&opts);
    let mut phase = 1u32;
    let provider = PistonMetaProvider::new();

    let emit_err = |stage: &str, phase: u32, e: &EasyMcError| {
        let mut p = InstallProgress::new("error", phase, phase_total, 0, None, e.to_string());
        p.stage_label = format!("{} · {}", stage_label("error"), stage_label(stage));
        on_progress(p);
    };

    on_progress(InstallProgress::new(
        "meta",
        phase,
        phase_total,
        0,
        Some(1),
        format!("获取版本元数据 {version}"),
    ));
    let info = match provider.resolve(version).await {
        Ok(i) => i,
        Err(e) => {
            let e = map_stage_err("meta", e);
            emit_err("meta", phase, &e);
            return Err(e);
        }
    };
    on_progress(InstallProgress::new(
        "meta",
        phase,
        phase_total,
        1,
        Some(1),
        format!("已解析 {}", info.id),
    ));

    let vdir = paths.version_dir(&info.id);
    tokio::fs::create_dir_all(&vdir).await?;
    let json_path = paths.version_json(&info.id);
    tokio::fs::write(&json_path, serde_json::to_vec_pretty(&info)?).await?;

    // —— client ——
    phase += 1;
    let client = &info.downloads.client;
    let jar_path = paths.version_jar(&info.id);
    on_progress(InstallProgress::new(
        "client",
        phase,
        phase_total,
        0,
        Some(client.size),
        format!("下载客户端 {}", info.id),
    ));
    let client_res = download_file(
        provider.client(),
        &client.url,
        &jar_path,
        Some(&client.sha1),
        Some(client.size),
        |c, t| {
            on_progress(InstallProgress::new(
                "client",
                phase,
                phase_total,
                c,
                t,
                format!("客户端 {}", info.id),
            ));
        },
    )
    .await;
    let client_res = match client_res {
        Ok(r) => r,
        Err(e) => {
            let e = map_stage_err("client", e);
            emit_err("client", phase, &e);
            return Err(e);
        }
    };
    let (sk, dl) = match client_res {
        FetchResult::Skipped => (1, 0),
        FetchResult::Downloaded => (0, 1),
    };
    on_progress(
        InstallProgress::new(
            "client",
            phase,
            phase_total,
            client.size,
            Some(client.size),
            if sk > 0 {
                format!("客户端已存在，跳过 {}", info.id)
            } else {
                format!("客户端已下载 {}", info.id)
            },
        )
        .with_counts(sk, dl),
    );

    // —— libraries ——
    phase += 1;
    let libs: Vec<&Library> = info
        .libraries
        .iter()
        .filter(|l| rules_allow(l.rules.as_deref(), &[]))
        .collect();
    let lib_total = libs.len() as u64;
    let mut skipped = 0u64;
    let mut downloaded = 0u64;
    on_progress(
        InstallProgress::new(
            "libraries",
            phase,
            phase_total,
            0,
            Some(lib_total.max(1)),
            format!("共 {lib_total} 个依赖库"),
        )
        .with_counts(0, 0),
    );

    for (i, lib) in libs.iter().enumerate() {
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let rel = artifact
                .path
                .clone()
                .unwrap_or_else(|| maven_path_from_name(&lib.name));
            let dest = paths.library_path(&rel);
            let res = download_file(
                provider.client(),
                &artifact.url,
                &dest,
                Some(&artifact.sha1),
                Some(artifact.size),
                |_, _| {},
            )
            .await;
            match res {
                Ok(FetchResult::Skipped) => skipped += 1,
                Ok(FetchResult::Downloaded) => downloaded += 1,
                Err(e) => {
                    let e = map_stage_err("libraries", e);
                    emit_err("libraries", phase, &e);
                    return Err(e);
                }
            }
        }
        if let Some(classifiers) = lib.downloads.as_ref().and_then(|d| d.classifiers.as_ref()) {
            if let Some(natives_map) = &lib.natives {
                if let Some(key) = natives_key(natives_map) {
                    if let Some(art) = classifiers.get(&key) {
                        let rel = art.path.clone().unwrap_or_else(|| format!("{key}.jar"));
                        let dest = paths.library_path(&rel);
                        let res = download_file(
                            provider.client(),
                            &art.url,
                            &dest,
                            Some(&art.sha1),
                            Some(art.size),
                            |_, _| {},
                        )
                        .await;
                        match res {
                            Ok(FetchResult::Skipped) => skipped += 1,
                            Ok(FetchResult::Downloaded) => downloaded += 1,
                            Err(e) => {
                                let e = map_stage_err("libraries", e);
                                emit_err("libraries", phase, &e);
                                return Err(e);
                            }
                        }
                    }
                }
            }
        }
        let cur = i as u64 + 1;
        if cur == lib_total || cur % 3 == 0 || cur == 1 {
            on_progress(
                InstallProgress::new(
                    "libraries",
                    phase,
                    phase_total,
                    cur,
                    Some(lib_total.max(1)),
                    lib.name.clone(),
                )
                .with_counts(skipped, downloaded),
            );
        }
    }
    on_progress(
        InstallProgress::new(
            "libraries",
            phase,
            phase_total,
            lib_total,
            Some(lib_total.max(1)),
            format!("依赖库完成（新下 {downloaded}，跳过 {skipped}）"),
        )
        .with_counts(skipped, downloaded),
    );

    // —— natives ——
    if opts.natives {
        phase += 1;
        on_progress(InstallProgress::new(
            "natives",
            phase,
            phase_total,
            0,
            Some(1),
            "解压 natives…",
        ));
        if let Err(e) = extract_natives(paths, &info) {
            let e = map_stage_err("natives", e);
            emit_err("natives", phase, &e);
            return Err(e);
        }
        on_progress(InstallProgress::new(
            "natives",
            phase,
            phase_total,
            1,
            Some(1),
            "natives 已就绪",
        ));
    }

    // —— assets ——
    if opts.assets {
        phase += 1;
        if let Err(e) =
            install_assets(paths, &info, provider.client(), phase, phase_total, &on_progress).await
        {
            let e = map_stage_err("assets", e);
            emit_err("assets", phase, &e);
            return Err(e);
        }
    }

    on_progress(InstallProgress::new(
        "done",
        phase_total,
        phase_total,
        1,
        Some(1),
        format!("{} 安装完成", info.id),
    ));
    Ok(info)
}

async fn install_assets(
    paths: &Paths,
    info: &VersionInfo,
    client: &reqwest::Client,
    phase: u32,
    phase_total: u32,
    on_progress: &(impl Fn(InstallProgress) + Send + Sync),
) -> Result<()> {
    let idx_ref = &info.asset_index;
    let idx_path = paths
        .assets_indexes_dir()
        .join(format!("{}.json", idx_ref.id));
    on_progress(InstallProgress::new(
        "assets",
        phase,
        phase_total,
        0,
        Some(1),
        format!("获取资源索引 {}", idx_ref.id),
    ));
    download_file(
        client,
        &idx_ref.url,
        &idx_path,
        Some(&idx_ref.sha1),
        Some(idx_ref.size),
        |_, _| {},
    )
    .await?;

    let index: AssetIndex = serde_json::from_str(&std::fs::read_to_string(&idx_path)?)?;
    let objects: Vec<_> = index.objects.values().cloned().collect();
    let total = objects.len() as u64;
    on_progress(
        InstallProgress::new(
            "assets",
            phase,
            phase_total,
            0,
            Some(total.max(1)),
            format!("开始下载 {total} 个资源对象"),
        )
        .with_counts(0, 0),
    );

    let mut set = JoinSet::new();
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(12));
    let objects_dir = paths.assets_objects_dir();

    for obj in objects {
        let client = client.clone();
        let hash = obj.hash.clone();
        let size = obj.size;
        let dest = objects_dir.join(&hash[..2]).join(&hash);
        let sem = sem.clone();
        let url = format!(
            "https://resources.download.minecraft.net/{}/{}",
            &hash[..2],
            hash
        );
        set.spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| EasyMcError::msg(e.to_string()))?;
            download_file(&client, &url, &dest, Some(&hash), Some(size), |_, _| {}).await
        });
    }

    let mut finished = 0u64;
    let mut skipped = 0u64;
    let mut downloaded = 0u64;
    let mut first_err: Option<EasyMcError> = None;

    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(Ok(FetchResult::Skipped)) => skipped += 1,
            Ok(Ok(FetchResult::Downloaded)) => downloaded += 1,
            Ok(Err(e)) => {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(EasyMcError::msg(format!("资源下载任务失败: {e}")));
                }
            }
        }
        finished += 1;
        if finished == total || finished % 20 == 0 || finished == 1 {
            on_progress(
                InstallProgress::new(
                    "assets",
                    phase,
                    phase_total,
                    finished,
                    Some(total.max(1)),
                    "下载游戏资源",
                )
                .with_counts(skipped, downloaded),
            );
        }
    }

    if let Some(e) = first_err {
        // Partial assets remain on disk (idempotent retry); surface a clear message.
        return Err(EasyMcError::msg(format!(
            "{e}（已完成 {finished}/{total}，其中跳过 {skipped}、新下 {downloaded}；可重试续传）"
        )));
    }

    on_progress(
        InstallProgress::new(
            "assets",
            phase,
            phase_total,
            total,
            Some(total.max(1)),
            format!("资源完成（新下 {downloaded}，跳过 {skipped}）"),
        )
        .with_counts(skipped, downloaded),
    );
    Ok(())
}

pub fn extract_natives(paths: &Paths, info: &VersionInfo) -> Result<()> {
    let out = paths.natives_for_version(&info.id);
    std::fs::create_dir_all(&out)?;
    for lib in &info.libraries {
        if !rules_allow(lib.rules.as_deref(), &[]) {
            continue;
        }
        let mut jars: Vec<PathBuf> = Vec::new();
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let rel = artifact
                .path
                .clone()
                .unwrap_or_else(|| maven_path_from_name(&lib.name));
            if is_native_library(&lib.name, Some(&rel)) {
                jars.push(paths.library_path(&rel));
            }
        }
        if let (Some(classifiers), Some(natives_map)) = (
            lib.downloads.as_ref().and_then(|d| d.classifiers.as_ref()),
            lib.natives.as_ref(),
        ) {
            if let Some(key) = natives_key(natives_map) {
                if let Some(art) = classifiers.get(&key) {
                    let rel = art.path.clone().unwrap_or_default();
                    jars.push(paths.library_path(rel));
                }
            }
        }
        for jar in jars {
            if jar.exists() {
                extract_zip_natives(&jar, &out)?;
            }
        }
    }
    Ok(())
}

fn extract_zip_natives(jar: &Path, out: &Path) -> Result<()> {
    let file = File::open(jar)?;
    let mut archive = ZipArchive::new(file).map_err(|e| EasyMcError::msg(e.to_string()))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| EasyMcError::msg(e.to_string()))?;
        let name = file.name().to_string();
        if name.ends_with('/') || name.contains("META-INF") {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".so")
            || lower.ends_with(".dll")
            || lower.ends_with(".dylib")
            || lower.ends_with(".jnilib"))
        {
            continue;
        }
        let filename = Path::new(&name)
            .file_name()
            .ok_or_else(|| EasyMcError::msg("bad zip entry"))?;
        let dest = out.join(filename);
        let mut outfile = File::create(&dest)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        outfile.write_all(&buf)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = outfile.metadata()?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest, perms)?;
        }
    }
    Ok(())
}

fn natives_key(map: &std::collections::HashMap<String, String>) -> Option<String> {
    let os = crate::rules::current_os_name();
    map.get(os).cloned().map(|k| {
        k.replace(
            "${arch}",
            if cfg!(target_arch = "x86") {
                "32"
            } else {
                "64"
            },
        )
    })
}

pub fn maven_path_from_name(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return format!("{name}.jar");
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts.get(3);
    match classifier {
        Some(c) => format!("{group}/{artifact}/{version}/{artifact}-{version}-{c}.jar"),
        None => format!("{group}/{artifact}/{version}/{artifact}-{version}.jar"),
    }
}

pub fn load_installed_version(paths: &Paths, id: &str) -> Result<VersionInfo> {
    let p = paths.version_json(id);
    let raw = std::fs::read_to_string(&p)
        .map_err(|_| EasyMcError::Missing(format!("version json {}", p.display())))?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn is_version_installed(paths: &Paths, id: &str) -> bool {
    paths.version_jar(id).exists() && paths.version_json(id).exists()
}

/// Classpath entries (non-native library jars + client jar), colon/semicolon joined by caller.
pub fn classpath_jars(paths: &Paths, info: &VersionInfo) -> Result<Vec<PathBuf>> {
    let mut jars = Vec::new();
    for lib in &info.libraries {
        if !rules_allow(lib.rules.as_deref(), &[]) {
            continue;
        }
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let rel = artifact
                .path
                .clone()
                .unwrap_or_else(|| maven_path_from_name(&lib.name));
            if is_native_library(&lib.name, Some(&rel)) {
                continue;
            }
            jars.push(paths.library_path(rel));
        }
    }
    jars.push(paths.version_jar(&info.id));
    Ok(jars)
}
