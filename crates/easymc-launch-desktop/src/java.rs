use std::path::{Path, PathBuf};
use std::process::Command;

use easymc_core::{EasyMcError, Paths, Result};
use flate2::read::GzDecoder;
use tar::Archive;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JavaInstall {
    pub path: PathBuf,
    pub version_string: String,
    pub major: u32,
    pub source: String,
}

/// Detect usable Java on PATH / JAVA_HOME / common locations / EasyMC runtimes.
///
/// The previous version only knew about `JAVA_HOME`, `which java` (a Unix tool)
/// and `/usr/lib/jvm`, so a normal Windows or macOS install was invisible. The
/// scan below is cross-platform: it walks the real install roots each vendor
/// uses instead of shelling out.
pub fn detect_java() -> Vec<JavaInstall> {
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut candidates: Vec<(PathBuf, String)> = Vec::new();
    if let Ok(home) = std::env::var("JAVA_HOME") {
        candidates.push((PathBuf::from(home).join("bin").join(java_bin()), "JAVA_HOME".into()));
    }
    if let Some(p) = path_java() {
        candidates.push((p, "PATH".into()));
    }
    if let Ok(paths) = Paths::discover() {
        if let Ok(rd) = std::fs::read_dir(paths.runtimes_dir()) {
            for ent in rd.flatten() {
                let bin = find_java_binary(&ent.path());
                if let Some(bin) = bin {
                    candidates.push((bin, format!("easymc-runtime:{}", ent.file_name().to_string_lossy())));
                }
            }
        }
    }
    candidates.extend(system_java_candidates());

    for (path, source) in candidates {
        let key = path.to_string_lossy().to_string();
        if !seen.insert(key) {
            continue;
        }
        if let Ok(info) = probe_java(&path) {
            found.push(JavaInstall {
                path,
                version_string: info.0,
                major: info.1,
                source,
            });
        }
    }
    found.sort_by(|a, b| b.major.cmp(&a.major));
    found
}

pub fn select_java(required_major: u32, preferred: Option<&Path>) -> Result<JavaInstall> {
    if let Some(p) = preferred {
        if let Ok((ver, major)) = probe_java(p) {
            if major >= required_major {
                return Ok(JavaInstall {
                    path: p.to_path_buf(),
                    version_string: ver,
                    major,
                    source: "preferred".into(),
                });
            }
        }
    }
    detect_java()
        .into_iter()
        .find(|j| j.major >= required_major)
        .ok_or_else(|| {
            EasyMcError::Missing(format!(
                "未找到 Java {required_major}+。请安装 JDK 或运行 ensure_java 下载 Temurin。"
            ))
        })
}

fn java_bin() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

/// Find `java` by walking PATH. Deliberately no `which`/`where` subprocess:
/// `which` does not exist on Windows, which is why detection used to come up
/// empty there.
fn path_java() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let cand = dir.join(java_bin());
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// JDK install roots that ship as `…/<jdk>/[Contents/Home/]bin/java`.
fn scan_jdk_root(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // The root may itself be a JDK home (JAVA_HOME-style paths).
    let direct = root.join("bin").join(java_bin());
    if direct.is_file() {
        out.push(direct);
    }
    let Ok(rd) = std::fs::read_dir(root) else {
        return out;
    };
    for ent in rd.flatten() {
        let base = ent.path();
        #[cfg(target_os = "macos")]
        {
            // Oracle/OpenJDK bundles nest the runtime under Contents/Home.
            let bundled = base.join("Contents/Home/bin").join(java_bin());
            if bundled.is_file() {
                out.push(bundled);
                continue;
            }
        }
        let bin = base.join("bin").join(java_bin());
        if bin.is_file() {
            out.push(bin);
        }
    }
    out
}

#[cfg(windows)]
fn windows_program_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for key in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Ok(p) = std::env::var(key) {
            if !p.is_empty() {
                v.push(PathBuf::from(p));
            }
        }
    }
    v
}

#[cfg(not(windows))]
fn windows_program_dirs() -> Vec<PathBuf> {
    Vec::new()
}

/// Vendor install roots for the current OS, so an existing JDK is reused
/// instead of downloading Temurin on top of it.
fn system_java_candidates() -> Vec<(PathBuf, String)> {
    let mut roots: Vec<(PathBuf, &'static str)> = Vec::new();
    if cfg!(windows) {
        for base in windows_program_dirs() {
            for vendor in [
                "Java",
                "Eclipse Adoptium",
                "Eclipse Foundation",
                "Microsoft",
                "Zulu",
                "Amazon Corretto",
                "BellSoft",
                "IBM",
                "Semeru",
                "JetBrains",
            ] {
                roots.push((base.join(vendor), "windows"));
            }
        }
        if let Some(local) = dirs::data_local_dir() {
            roots.push((local.join("Programs/Eclipse Adoptium"), "windows"));
            roots.push((local.join("Programs/Microsoft JDK"), "windows"));
        }
    } else if cfg!(target_os = "macos") {
        roots.push((PathBuf::from("/Library/Java/JavaVirtualMachines"), "macos"));
        roots.push((PathBuf::from("/System/Library/Java/JavaVirtualMachines"), "macos"));
        roots.push((PathBuf::from("/opt/homebrew/opt"), "homebrew"));
        roots.push((PathBuf::from("/usr/local/opt"), "homebrew"));
        if let Some(h) = dirs::home_dir() {
            roots.push((h.join("Library/Java/JavaVirtualMachines"), "macos"));
        }
    } else {
        roots.push((PathBuf::from("/usr/lib/jvm"), "linux"));
        roots.push((PathBuf::from("/usr/lib64/jvm"), "linux"));
        roots.push((PathBuf::from("/usr/java"), "linux"));
        roots.push((PathBuf::from("/opt/java"), "linux"));
        if let Some(h) = dirs::home_dir() {
            roots.push((h.join(".sdkman/candidates/java"), "sdkman"));
        }
    }

    let mut out = Vec::new();
    for (root, src) in roots {
        for bin in scan_jdk_root(&root) {
            out.push((bin, format!("system:{src}")));
        }
    }
    out
}

fn find_java_binary(runtime_root: &Path) -> Option<PathBuf> {
    // Temurin tarball extracts to jdk-21.../bin/java
    let direct = runtime_root.join("bin").join(java_bin());
    if direct.exists() {
        return Some(direct);
    }
    if let Ok(rd) = std::fs::read_dir(runtime_root) {
        for ent in rd.flatten() {
            let p = ent.path().join("bin").join(java_bin());
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

/// `java -version`, without flashing a console window in the Windows GUI build.
fn java_version_command(java: &Path) -> Command {
    let mut cmd = Command::new(java);
    cmd.arg("-version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn probe_java(java: &Path) -> Result<(String, u32)> {
    let output = java_version_command(java)
        .output()
        .map_err(|e| EasyMcError::msg(e.to_string()))?;
    let text = {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        if !err.trim().is_empty() {
            err.into_owned()
        } else {
            out.into_owned()
        }
    };
    let major = parse_java_major(&text).ok_or_else(|| EasyMcError::msg(format!("cannot parse java version: {text}")))?;
    Ok((text.lines().next().unwrap_or("unknown").to_string(), major))
}

pub fn parse_java_major(version_output: &str) -> Option<u32> {
    // openjdk version "21.0.1"  or java version "1.8.0_402"
    let line = version_output.lines().next()?;
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    let ver = &line[start..end];
    if let Some(rest) = ver.strip_prefix("1.") {
        let major = rest.split(|c| c == '.' || c == '_').next()?.parse().ok()?;
        Some(major)
    } else {
        ver.split('.').next()?.parse().ok()
    }
}

/// Download Eclipse Temurin JDK into EasyMC runtimes (Linux x64/aarch64, macOS, Windows).
pub async fn ensure_java(paths: &Paths, major: u32, on_log: impl Fn(String)) -> Result<JavaInstall> {
    if let Ok(j) = select_java(major, None) {
        on_log(format!("已找到 Java {}: {} ({})", j.major, j.path.display(), j.source));
        return Ok(j);
    }

    paths.ensure()?;
    let (os, arch, ext, pack) = platform_adoptium();
    let api = format!(
        "https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse?project=jdk"
    );
    on_log(format!("下载 Temurin JDK {major} ({os}/{arch})…"));
    let client = reqwest::Client::builder()
        .user_agent(concat!("EasyMC/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| EasyMcError::Http(e.to_string()))?;

    let resp = client
        .get(&api)
        .send()
        .await
        .map_err(|e| EasyMcError::Http(e.to_string()))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(e.to_string()))?;

    let dest_dir = paths.runtimes_dir().join(format!("temurin-{major}"));
    tokio::fs::create_dir_all(&dest_dir).await?;
    let archive_path = paths.runtimes_dir().join(format!("temurin-{major}.{ext}"));

    let total = resp.content_length();
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::File::create(&archive_path).await?;
    let mut written = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| EasyMcError::Http(e.to_string()))?;
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
        if written % (8 * 1024 * 1024) < chunk.len() as u64 {
            on_log(format!(
                "JDK 下载中… {:.1} / {} MiB",
                written as f64 / 1e6,
                total.map(|t| format!("{:.1}", t as f64 / 1e6)).unwrap_or("?".into())
            ));
        }
    }
    file.flush().await?;
    drop(file);

    on_log("解压 JDK…".into());
    // clean dest
    if dest_dir.exists() {
        let _ = std::fs::remove_dir_all(&dest_dir);
    }
    std::fs::create_dir_all(&dest_dir)?;

    match pack {
        "tar.gz" => {
            let file = std::fs::File::open(&archive_path)?;
            let dec = GzDecoder::new(file);
            let mut archive = Archive::new(dec);
            archive
                .unpack(&dest_dir)
                .map_err(|e| EasyMcError::msg(e.to_string()))?;
        }
        "zip" => {
            return Err(EasyMcError::NotImplemented(
                "Windows JDK zip extract — install system Java 21+ or unpack manually into runtimes/",
            ));
        }
        _ => return Err(EasyMcError::msg("unknown jdk package")),
    }

    let _ = std::fs::remove_file(&archive_path);
    let bin = find_java_binary(&dest_dir).ok_or_else(|| EasyMcError::msg("JDK extracted but java binary missing"))?;
    let (ver, maj) = probe_java(&bin)?;
    on_log(format!("JDK 就绪: {ver}"));
    Ok(JavaInstall {
        path: bin,
        version_string: ver,
        major: maj,
        source: format!("downloaded-temurin-{major}"),
    })
}

fn platform_adoptium() -> (&'static str, &'static str, &'static str, &'static str) {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    let (ext, pack) = if cfg!(target_os = "windows") {
        ("zip", "zip")
    } else {
        ("tar.gz", "tar.gz")
    };
    (os, arch, ext, pack)
}
