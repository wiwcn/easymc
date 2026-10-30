//! Keyword / phrase intent router — offline, zh-CN + English aliases.
//! Not an LLM. Expand the table as needed.

use serde::{Deserialize, Serialize};

use crate::actions::GameFolderKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    /// Analyze crash / launch logs (optional pasted file path).
    Diagnose {
        instance_id: Option<String>,
        /// Absolute/relative path to a log or crash-report the player pasted.
        log_path: Option<String>,
    },
    EnsureJava { major: u32 },
    /// Immediate write (shortcut「内存 4G」).
    BumpMemory { mb: u32 },
    /// Analyze + propose_changes card (memory + JVM); user must 应用更改.
    TuneConfig { instance_id: Option<String> },
    /// ensure_java + status summary for selected instance.
    SetupEnv { instance_id: Option<String> },
    /// Export share pack / create modpack draft (host runs easymc-share).
    BuildPack { instance_id: Option<String> },
    SetDownloadSource { source: String },
    OpenInstanceDir { instance_id: Option<String> },
    /// List jars in instance mods/ (player: 已装模组 / 检查模组).
    CheckMods { instance_id: Option<String> },
    /// Enable / disable a mod jar (PCL/HMCL-style rename).
    ToggleMod {
        query: String,
        /// Some(true)=enable, Some(false)=disable, None=flip.
        enable: Option<bool>,
        instance_id: Option<String>,
    },
    /// List installed instances / versions.
    ListInstances,
    /// Switch the active instance by name / version / id query.
    SelectInstance { query: String },
    /// Rename a version's display name (active if query omitted).
    RenameInstance {
        /// Optional name/version/loader/id to pick which version; None = current.
        query: Option<String>,
        new_name: String,
    },
    /// Duplicate a version (mods/resourcepacks/shaderpacks/config/datapacks; not saves).
    DuplicateInstance {
        query: Option<String>,
        new_name: Option<String>,
    },
    /// Permanently delete a version (and its game folder). Query required (or 当前).
    DeleteInstance { query: String },
    /// Permanently delete a mod jar by filename keyword.
    DeleteMod {
        query: String,
        instance_id: Option<String>,
    },
    /// Reveal latest.log or newest crash-report path for the current version.
    OpenLatestLog { instance_id: Option<String> },
    /// List saved accounts (no tokens).
    ListAccounts,
    /// Switch the active account by username / id query.
    SelectAccount { query: String },
    /// Open (or create) the screenshots folder for the current version.
    OpenScreenshots { instance_id: Option<String> },
    /// Open (or create) a common game subfolder: mods / resourcepacks / shaderpacks / saves / config / logs / crash-reports.
    OpenGameFolder {
        folder: GameFolderKind,
        instance_id: Option<String>,
    },
    /// Import a local Modrinth `.mrpack` (or EasyMC share) into a new version.
    ImportMrpack { path: String },
    /// Search Modrinth (+ CF if key) without installing — structured hits for agent cite / InstallMod.
    SearchMods { query: String },
    /// Suggest complementary popular mods for a pack theme via Modrinth.
    SuggestMods { theme: String },
    ShowDataDir,
    DiagnosticsBundle,
    InstallMod { query: String },
    Help,
    Unknown { raw: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentResult {
    pub intent: Intent,
    pub confidence: f32,
    pub matched: String,
    pub reply_zh: String,
}

fn normalize(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .replace('　', " ")
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
}

fn extract_java_major(t: &str) -> u32 {
    for m in [21u32, 17, 8, 11] {
        if t.contains(&m.to_string()) {
            return m;
        }
    }
    21
}

fn extract_memory_mb(t: &str) -> u32 {
    // 「内存 4096」「4g」「8G」「4096m」
    let re = regex::Regex::new(r"(?i)(\d+)\s*(g|gb|m|mb)?").ok();
    if let Some(re) = re {
        for cap in re.captures_iter(t) {
            let n: u32 = cap.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let unit = cap
                .get(2)
                .map(|m| m.as_str().to_ascii_lowercase())
                .unwrap_or_default();
            if n == 0 {
                continue;
            }
            if unit.starts_with('g') {
                return n.saturating_mul(1024);
            }
            if unit.starts_with('m') || unit.is_empty() {
                if n >= 1024 {
                    return n;
                }
                // bare small number near 内存 → treat as GB if <= 32
                if n <= 32 {
                    return n.saturating_mul(1024);
                }
                return n;
            }
        }
    }
    4096
}

fn extract_mod_query(t: &str) -> Option<String> {
    // 「装 sodium」「安装 iris」「install sodium」「下载 lithium」
    let prefixes = [
        "安装模组",
        "安装",
        "装模组",
        "装 ",
        "装",
        "下载",
        "install mod",
        "install ",
        "get ",
    ];
    let t = t.trim();
    for p in prefixes {
        if let Some(rest) = t.strip_prefix(p).or_else(|| {
            let lower = t.to_lowercase();
            let pl = p.to_lowercase();
            lower
                .strip_prefix(&pl)
                .map(|r| &t[t.len() - r.len()..])
        }) {
            let q = rest
                .trim()
                .trim_matches(|c: char| c == '"' || c == '\'' || c == '「' || c == '」')
                .trim();
            if !q.is_empty() && q.len() < 64 {
                return Some(q.to_string());
            }
        }
    }
    // 「sodium」alone as install hint when short ascii token
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() >= 3
        && compact.len() <= 32
        && compact
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Some(compact);
    }
    None
}


fn extract_log_path(t: &str) -> Option<String> {
    // Quoted path: "...latest.log" or 「...crash.txt」
    let re_q = regex::Regex::new(r#"["「]([^"」]+\.(?:log|txt|gz))["」]"#).ok();
    if let Some(re) = re_q {
        if let Some(c) = re.captures(t) {
            let p = c.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            if !p.is_empty() {
                return Some(p.to_string());
            }
        }
    }
    // Bare unix / windows path ending in .log / .txt
    let re = regex::Regex::new(
        r#"(?i)(?:[a-z]:\\[^\s"']+\.(?:log|txt)|/(?:[^\s"']+/)*[^\s"']+\.(?:log|txt))"#,
    )
    .ok();
    if let Some(re) = re {
        if let Some(m) = re.find(t) {
            return Some(m.as_str().to_string());
        }
    }
    None
}

fn extract_search_query(t: &str) -> Option<String> {
    let prefixes = [
        "搜索模组",
        "搜模组",
        "查找模组",
        "找模组",
        "search mods",
        "search mod",
        "search ",
        "搜 ",
        "搜索 ",
    ];
    let t = t.trim();
    let lower = t.to_lowercase();
    for p in prefixes {
        let pl = p.to_lowercase();
        if !lower.starts_with(&pl) {
            continue;
        }
        let rest: String = t.chars().skip(pl.chars().count()).collect();
        let q = rest
            .trim()
            .trim_matches(|c: char| c == '"' || c == '\'' || c == '「' || c == '」')
            .trim();
        if !q.is_empty() && q.chars().count() < 64 {
            return Some(q.to_string());
        }
    }
    None
}

fn extract_suggest_theme(t: &str) -> String {
    let themes = [
        ("科技", "tech"),
        ("魔法", "magic"),
        ("生存", "survival"),
        ("性能", "performance"),
        ("优化", "performance"),
        ("光影", "shader"),
        ("冒险", "adventure"),
        ("建筑", "building"),
        ("tech", "tech"),
        ("magic", "magic"),
        ("survival", "survival"),
        ("performance", "performance"),
        ("qos", "qos"),
        ("生活质量", "qos"),
    ];
    for (k, v) in themes {
        if t.contains(k) {
            return v.into();
        }
    }
    "survival".into()
}

/// Map natural language (or slash-like) text to a local Intent.
pub fn route(input: &str) -> IntentResult {
    let raw = input.trim().to_string();
    let t = normalize(&raw);

    if t.is_empty() {
        return IntentResult {
            intent: Intent::Help,
            confidence: 1.0,
            matched: "(empty)".into(),
            reply_zh: help_text(),
        };
    }

    // setup env (before generic java / diagnose)
    let setup_keys = [
        "一键环境",
        "准备环境",
        "setup env",
        "setupenv",
        "环境准备",
        "配环境",
    ];
    for k in setup_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::SetupEnv { instance_id: None },
                confidence: 0.95,
                matched: k.into(),
                reply_zh: "正在检查 Java、内存与下载源…".into(),
            };
        }
    }

    // tune config / optimize (propose, not silent write)
    let tune_keys = [
        "优化配置",
        "优化启动",
        "tune config",
        "tuneconfig",
        "调优",
        "优化 jvm",
        "优化jvm",
        "优化参数",
        "建议更改",
        "提出更改",
    ];
    for k in tune_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::TuneConfig { instance_id: None },
                confidence: 0.93,
                matched: k.into(),
                reply_zh: "正在分析内存与启动参数，生成建议更改…".into(),
            };
        }
    }

    // build pack / share
    let pack_keys = [
        "制作整合包",
        "生成整合包",
        "创建整合包",
        "分享包",
        "导出整合包",
        "build pack",
        "buildpack",
        "create modpack",
        "export modpack",
        "导出分享",
        "打包分享",
        "导出模组包",
    ];
    for k in pack_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::BuildPack { instance_id: None },
                confidence: 0.92,
                matched: k.into(),
                reply_zh: "正在为当前版本制作整合包…".into(),
            };
        }
    }

    // BMCLAPI / download source
    let mirror_keys = [
        "bmclapi",
        "切换加速",
        "下载加速",
        "镜像源",
        "加速下载",
        "切换 bmcl",
        "用官方源",
        "官方下载",
    ];
    for k in mirror_keys {
        if t.contains(k) {
            let source = if t.contains("官方") {
                "official"
            } else {
                "bmclapi"
            };
            return IntentResult {
                intent: Intent::SetDownloadSource {
                    source: source.into(),
                },
                confidence: 0.9,
                matched: k.into(),
                reply_zh: if source == "bmclapi" {
                    "将切换到 BMCLAPI 加速下载。".into()
                } else {
                    "将切换回官方下载源。".into()
                },
            };
        }
    }

    // open instance / version folder
    let inst_dir_keys = [
        "打开版本文件夹",
        "版本文件夹",
        "实例文件夹",
        "打开实例",
        "游戏版本目录",
        "open instance",
        "version folder",
    ];
    for k in inst_dir_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::OpenInstanceDir { instance_id: None },
                confidence: 0.92,
                matched: k.into(),
                reply_zh: "正在定位当前版本文件夹…".into(),
            };
        }
    }


    // Open game subfolders (before「检查模组」so 「打开模组文件夹」wins)
    let folder_specs: &[(&[&str], GameFolderKind)] = &[
        (
            &[
                "打开模组文件夹",
                "模组文件夹",
                "打开 mods",
                "mods 文件夹",
                "mods文件夹",
                "open mods folder",
                "mods folder",
            ],
            GameFolderKind::Mods,
        ),
        (
            &[
                "打开资源包",
                "资源包文件夹",
                "打开材质",
                "材质包文件夹",
                "打开 resourcepacks",
                "resourcepacks",
                "open resource packs",
                "resource pack folder",
            ],
            GameFolderKind::ResourcePacks,
        ),
        (
            &[
                "打开光影",
                "光影文件夹",
                "光影包文件夹",
                "打开 shaderpacks",
                "shaderpacks",
                "open shaders",
                "shader pack folder",
            ],
            GameFolderKind::ShaderPacks,
        ),
        (
            &[
                "打开存档",
                "存档文件夹",
                "打开世界",
                "世界文件夹",
                "打开 saves",
                "saves 文件夹",
                "saves文件夹",
                "open saves",
                "worlds folder",
                "saves folder",
            ],
            GameFolderKind::Saves,
        ),
        (
            &[
                "打开配置文件夹",
                "配置文件夹",
                "打开 config",
                "config 文件夹",
                "config文件夹",
                "open config folder",
                "config folder",
            ],
            GameFolderKind::Config,
        ),
        (
            &[
                "打开日志文件夹",
                "日志文件夹",
                "打开 logs",
                "logs 文件夹",
                "logs文件夹",
                "open logs folder",
                "logs folder",
            ],
            GameFolderKind::Logs,
        ),
        (
            &[
                "打开崩溃文件夹",
                "崩溃报告文件夹",
                "打开 crash-reports",
                "crash-reports",
                "crash reports folder",
                "open crash reports",
            ],
            GameFolderKind::CrashReports,
        ),
    ];
    for (keys, kind) in folder_specs {
        for k in *keys {
            if t.contains(k) {
                return IntentResult {
                    intent: Intent::OpenGameFolder {
                        folder: *kind,
                        instance_id: None,
                    },
                    confidence: 0.9,
                    matched: (*k).into(),
                    reply_zh: format!("正在定位{}文件夹…", kind.label_zh()),
                };
            }
        }
    }

    // toggle / enable / disable mod
    let toggle_enable_keys = ["启用模组", "开启模组", "enable mod", "enablemod"];
    let toggle_disable_keys = ["禁用模组", "关闭模组", "停用模组", "disable mod", "disablemod"];
    let toggle_flip_keys = ["切换模组", "toggle mod", "togglemod"];
    for (keys, enable) in [
        (&toggle_enable_keys[..], Some(true)),
        (&toggle_disable_keys[..], Some(false)),
        (&toggle_flip_keys[..], None),
    ] {
        for k in keys {
            if t.contains(k) {
                let idx = t.find(k).unwrap_or(0) + k.len();
                let mut q = t[idx..].trim().trim_start_matches(['：', ':', ' ']).to_string();
                if q.is_empty() {
                    q = extract_mod_query(&t).unwrap_or_default();
                }
                return IntentResult {
                    intent: Intent::ToggleMod {
                        query: q,
                        enable,
                        instance_id: None,
                    },
                    confidence: 0.92,
                    matched: (*k).into(),
                    reply_zh: "正在改模组启用状态…".into(),
                };
            }
        }
    }
    for (prefix, enable) in [
        ("禁用 ", Some(false)),
        ("启用 ", Some(true)),
        ("disable ", Some(false)),
        ("enable ", Some(true)),
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let q = rest.trim().to_string();
            if !q.is_empty()
                && !q.contains("模组")
                && !q.contains("内存")
                && !q.contains("java")
                && q.len() < 80
            {
                return IntentResult {
                    intent: Intent::ToggleMod {
                        query: q,
                        enable,
                        instance_id: None,
                    },
                    confidence: 0.85,
                    matched: prefix.trim().into(),
                    reply_zh: "正在改模组启用状态…".into(),
                };
            }
        }
    }

    let list_inst_keys = [
        "版本列表",
        "列出版本",
        "有哪些版本",
        "已装版本",
        "实例列表",
        "list instances",
        "list versions",
        "我的版本",
    ];
    for k in list_inst_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::ListInstances,
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "正在列出已安装版本…".into(),
            };
        }
    }

    // select / switch version
    for (prefix, _) in [
        ("切换版本 ", ()),
        ("选择版本 ", ()),
        ("换成版本 ", ()),
        ("用版本 ", ()),
        ("switch version ", ()),
        ("select version ", ()),
        ("switch instance ", ()),
        ("select instance ", ()),
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let q = rest.trim().to_string();
            if !q.is_empty() && q.len() < 80 {
                return IntentResult {
                    intent: Intent::SelectInstance { query: q },
                    confidence: 0.9,
                    matched: prefix.trim().into(),
                    reply_zh: "正在切换版本…".into(),
                };
            }
        }
    }
    if t.contains("切换到") && (t.contains("版本") || t.contains("instance")) {
        let q = t
            .replace("切换到", " ")
            .replace("版本", " ")
            .replace("instance", " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !q.is_empty() && q.len() < 80 {
            return IntentResult {
                intent: Intent::SelectInstance { query: q },
                confidence: 0.85,
                matched: "切换到".into(),
                reply_zh: "正在切换版本…".into(),
            };
        }
    }

        // rename version: 「重命名版本 新名字」「改名成 xxx」「把版本改名叫 xxx」
    for prefix in [
        "重命名版本 ",
        "重命名为 ",
        "改名成 ",
        "改名叫 ",
        "改名为 ",
        "rename version ",
        "rename to ",
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let new_name = rest.trim().to_string();
            if !new_name.is_empty() && new_name.len() < 64 {
                return IntentResult {
                    intent: Intent::RenameInstance {
                        query: None,
                        new_name,
                    },
                    confidence: 0.9,
                    matched: prefix.trim().into(),
                    reply_zh: "正在改版本名…".into(),
                };
            }
        }
    }
    if t.contains("改名叫") || t.contains("改名为") || t.contains("重命名成") {
        // 「把生存包改名叫硬核包」 / 「把版本改名叫 xxx」
        for sep in ["改名叫", "改名为", "重命名成"] {
            if let Some((left, right)) = t.split_once(sep) {
                let new_name = right.trim().to_string();
                if new_name.is_empty() || new_name.len() >= 64 {
                    continue;
                }
                let left = left
                    .replace("把版本", " ")
                    .replace("把", " ")
                    .replace("版本", " ")
                    .replace("将", " ");
                let q = left.split_whitespace().collect::<Vec<_>>().join(" ");
                let query = if q.is_empty() { None } else { Some(q) };
                return IntentResult {
                    intent: Intent::RenameInstance { query, new_name },
                    confidence: 0.88,
                    matched: sep.into(),
                    reply_zh: "正在改版本名…".into(),
                };
            }
        }
    }

    // duplicate version: 「复制版本」「复制当前版本」「复制版本 新名字」
    for prefix in ["复制版本 ", "复制为 ", "duplicate version ", "copy version "] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let new_name = rest.trim().to_string();
            if !new_name.is_empty() && new_name.len() < 64 {
                return IntentResult {
                    intent: Intent::DuplicateInstance {
                        query: None,
                        new_name: Some(new_name),
                    },
                    confidence: 0.9,
                    matched: prefix.trim().into(),
                    reply_zh: "正在复制版本…".into(),
                };
            }
        }
    }
    let dup_keys = [
        "复制版本",
        "复制当前版本",
        "拷贝版本",
        "克隆版本",
        "duplicate version",
        "copy version",
        "duplicate instance",
    ];
    for k in dup_keys {
        if t == k || t.contains(k) {
            return IntentResult {
                intent: Intent::DuplicateInstance {
                    query: None,
                    new_name: None,
                },
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "正在复制版本…".into(),
            };
        }
    }


    // delete mod (before delete version so 「删除模组」wins)
    for prefix in [
        "删除模组 ",
        "删掉模组 ",
        "移除模组 ",
        "卸载模组 ",
        "delete mod ",
        "remove mod ",
        "uninstall mod ",
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let q = rest.trim().to_string();
            if !q.is_empty() && q.len() < 80 {
                return IntentResult {
                    intent: Intent::DeleteMod {
                        query: q,
                        instance_id: None,
                    },
                    confidence: 0.92,
                    matched: prefix.trim().into(),
                    reply_zh: "正在删除模组…".into(),
                };
            }
        }
    }
    let del_mod_keys = ["删除模组", "删掉模组", "移除模组", "卸载模组", "delete mod", "remove mod"];
    for k in del_mod_keys {
        if t == k || (t.starts_with(k) && t.len() == k.len()) {
            return IntentResult {
                intent: Intent::DeleteMod {
                    query: String::new(),
                    instance_id: None,
                },
                confidence: 0.85,
                matched: k.into(),
                reply_zh: "请告诉我要删哪个模组，例如：删除模组 sodium".into(),
            };
        }
        if t.contains(k) {
            let idx = t.find(k).unwrap_or(0) + k.len();
            let q = t[idx..].trim().trim_start_matches(['：', ':', ' ']).to_string();
            return IntentResult {
                intent: Intent::DeleteMod {
                    query: q,
                    instance_id: None,
                },
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "正在删除模组…".into(),
            };
        }
    }
    // bare 「删除 sodium」 / 「删掉 iris」（避免误伤「删除版本」）
    for prefix in ["删除 ", "删掉 ", "移除 ", "卸载 "] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let q = rest.trim().to_string();
            if !q.is_empty()
                && !q.contains("版本")
                && !q.contains("账户")
                && !q.contains("账号")
                && !q.contains("instance")
                && !q.contains("version")
                && !q.contains("内存")
                && !q.contains("java")
                && q.len() < 64
            {
                return IntentResult {
                    intent: Intent::DeleteMod {
                        query: q,
                        instance_id: None,
                    },
                    confidence: 0.8,
                    matched: prefix.trim().into(),
                    reply_zh: "正在删除模组…".into(),
                };
            }
        }
    }

    // delete version
    for prefix in [
        "删除版本 ",
        "删掉版本 ",
        "移除版本 ",
        "卸载版本 ",
        "delete version ",
        "remove version ",
        "delete instance ",
        "remove instance ",
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let q = rest.trim().to_string();
            if !q.is_empty() && q.len() < 80 {
                return IntentResult {
                    intent: Intent::DeleteInstance { query: q },
                    confidence: 0.92,
                    matched: prefix.trim().into(),
                    reply_zh: "正在删除版本…".into(),
                };
            }
        }
    }
    let del_inst_keys = [
        "删除当前版本",
        "删掉当前版本",
        "删除这个版本",
        "删掉这个版本",
        "delete current version",
        "remove current version",
    ];
    for k in del_inst_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::DeleteInstance {
                    query: "当前".into(),
                },
                confidence: 0.92,
                matched: k.into(),
                reply_zh: "正在删除当前版本…".into(),
            };
        }
    }
    let del_inst_bare = ["删除版本", "删掉版本", "移除版本", "卸载版本", "delete version", "remove instance"];
    for k in del_inst_bare {
        if t == k || t.starts_with(&format!("{k} ")) || t.contains(k) {
            let idx = t.find(k).unwrap_or(0) + k.len();
            let q = t[idx..].trim().trim_start_matches(['：', ':', ' ']).to_string();
            let reply_zh = if q.is_empty() {
                "请告诉我要删哪个版本，例如：删除版本 1.21.1".into()
            } else {
                "正在删除版本…".into()
            };
            return IntentResult {
                intent: Intent::DeleteInstance { query: q },
                confidence: 0.9,
                matched: k.into(),
                reply_zh,
            };
        }
    }

let log_file_keys = [
        "打开日志",
        "打开最新日志",
        "最新日志",
        "日志文件",
        "打开崩溃报告",
        "崩溃报告在哪",
        "看日志文件",
        "open log",
        "open latest log",
        "latest log",
        "show log file",
    ];
    for k in log_file_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::OpenLatestLog { instance_id: None },
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "正在定位日志文件…".into(),
            };
        }
    }

    // switch account (before bare list keys that are substrings of longer phrases)
    for (prefix, _) in [
        ("切换账户 ", ()),
        ("切换账号 ", ()),
        ("选择账户 ", ()),
        ("选择账号 ", ()),
        ("换成账户 ", ()),
        ("换成账号 ", ()),
        ("用账户 ", ()),
        ("用账号 ", ()),
        ("switch account ", ()),
        ("select account ", ()),
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let q = rest.trim().to_string();
            if !q.is_empty() && q.len() < 80 {
                return IntentResult {
                    intent: Intent::SelectAccount { query: q },
                    confidence: 0.9,
                    matched: prefix.trim().into(),
                    reply_zh: "正在切换账户…".into(),
                };
            }
        }
    }
    if (t.contains("切换账户") || t.contains("切换账号") || t.contains("换成账户") || t.contains("换成账号"))
        && !t.contains("账户列表")
        && !t.contains("账号列表")
    {
        let q = t
            .replace("切换账户", " ")
            .replace("切换账号", " ")
            .replace("换成账户", " ")
            .replace("换成账号", " ")
            .replace("选择账户", " ")
            .replace("选择账号", " ")
            .replace("switch account", " ")
            .replace("select account", " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !q.is_empty() && q.len() < 80 {
            return IntentResult {
                intent: Intent::SelectAccount { query: q },
                confidence: 0.85,
                matched: "切换账户".into(),
                reply_zh: "正在切换账户…".into(),
            };
        }
    }

    let acct_keys = [
        "账户列表",
        "账号列表",
        "有哪些账户",
        "有哪些账号",
        "我的账户",
        "我的账号",
        "list accounts",
        "list account",
        "show accounts",
    ];
    for k in acct_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::ListAccounts,
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "正在列出账户…".into(),
            };
        }
    }

    let shot_keys = [
        "打开截图",
        "截图文件夹",
        "打开截图文件夹",
        "截图在哪",
        "看截图",
        "screenshots",
        "open screenshots",
        "screenshot folder",
    ];
    for k in shot_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::OpenScreenshots { instance_id: None },
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "正在定位截图文件夹…".into(),
            };
        }
    }


    let import_keys = [
        "导入整合包",
        "安装整合包",
        "导入 mrpack",
        "导入.mrpack",
        "import modpack",
        "import mrpack",
        "install mrpack",
    ];
    for k in import_keys {
        if t.contains(k) {
            let path = extract_log_path(&raw)
                .or_else(|| extract_log_path(&t))
                .or_else(|| {
                    t.split_whitespace()
                        .rev()
                        .find(|w| {
                            let wl = w.to_ascii_lowercase();
                            wl.contains('.')
                                && (w.contains('/')
                                    || w.contains('\\')
                                    || wl.ends_with("mrpack")
                                    || wl.ends_with(".zip"))
                        })
                        .map(|s| s.trim_matches(|c| c == '"' || c == '\'').to_string())
                })
                .unwrap_or_default();
            return IntentResult {
                intent: Intent::ImportMrpack { path },
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "正在导入整合包…".into(),
            };
        }
    }

    // check mods
    let mods_keys = [
        "检查模组",
        "已装模组",
        "模组冲突",
        "模组列表",
        "有多少模组",
        "看模组",
        "列出模组",
        "list mods",
        "installed mods",
        "check mods",
        "mod conflict",
    ];
    for k in mods_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::CheckMods { instance_id: None },
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "正在查看当前版本的模组…".into(),
            };
        }
    }

    // diagnose / crash
    let diagnose_keys = [
        "看上次报错",
        "上次报错",
        "查看崩溃",
        "诊断",
        "分析崩溃",
        "崩溃",
        "报错",
        "diagnose",
        "crash",
        "read crash",
        "看日志",
        "分析日志",
        "为什么崩",
        "挂了",
        "闪退",
        "启动失败",
        "打不开游戏",
        "进不去",
        "黑屏",
        "白屏",
        "卡住了",
        "启动不了",
        "游戏崩了",
        "崩了",
    ];
    for k in diagnose_keys {
        if t.contains(k) {
            let log_path = extract_log_path(&raw).or_else(|| extract_log_path(&t));
            return IntentResult {
                intent: Intent::Diagnose {
                    instance_id: None,
                    log_path,
                },
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "正在根据最近启动日志做本地规则诊断…".into(),
            };
        }
    }

    // analyze pasted log path without crash keywords
    if let Some(log_path) = extract_log_path(&raw).or_else(|| extract_log_path(&t)) {
        if t.contains("分析") || t.contains("日志") || t.contains("analyze") || t.contains("log") {
            return IntentResult {
                intent: Intent::Diagnose {
                    instance_id: None,
                    log_path: Some(log_path),
                },
                confidence: 0.88,
                matched: "log_path".into(),
                reply_zh: "正在分析你指定的日志文件…".into(),
            };
        }
    }

    // ensure java (after setup env)
    let java_keys = [
        "确保 java",
        "安装 java",
        "ensure java",
        "java 21",
        "java21",
        "缺 java",
        "没有 java",
        "下载 jdk",
        "安装 jdk",
        "修好 java",
        "修 java",
        "java 坏了",
        "java坏了",
        "找不到 java",
        "需要 java",
        "换 java",
        "java 版本",
    ];
    for k in java_keys {
        if t.contains(k) {
            let major = extract_java_major(&t);
            return IntentResult {
                intent: Intent::EnsureJava { major },
                confidence: 0.92,
                matched: k.into(),
                reply_zh: format!("将检查并准备 Java {major}。"),
            };
        }
    }

    // memory — explicit bump (shortcut applies immediately)
    let mem_keys = [
        "内存",
        "memory",
        "xmx",
        "oom",
        "堆",
        "加大内存",
        "提高内存",
        "内存不够",
        "内存不足",
        "卡成幻灯片",
        "分配内存",
        "调内存",
    ];
    for k in mem_keys {
        if t.contains(k) {
            let mb = extract_memory_mb(&t);
            return IntentResult {
                intent: Intent::BumpMemory { mb },
                confidence: 0.85,
                matched: k.into(),
                reply_zh: format!("将把启动内存设为 {mb} MB（写入 settings.memory_mb）。"),
            };
        }
    }

    // data dir (global)
    let dir_keys = [
        "数据目录",
        "打开目录",
        "data dir",
        "open data",
        "工作目录",
        "游戏目录",
        "游戏文件夹",
    ];
    for k in dir_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::ShowDataDir,
                confidence: 0.95,
                matched: k.into(),
                reply_zh: "这是 EasyMC 本地数据根目录路径。".into(),
            };
        }
    }

    // diagnostics bundle
    let diag_keys = ["诊断包", "导出日志", "复制诊断", "diagnostic", "bundle", "导出报告", "导出问题报告"];
    for k in diag_keys {
        if t.contains(k) {
            return IntentResult {
                intent: Intent::DiagnosticsBundle,
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "将打包最近日志与诊断摘要到数据目录。".into(),
            };
        }
    }

    // search mods (without install) — before install
    let search_keys = [
        "搜索模组",
        "搜模组",
        "查找模组",
        "找模组",
        "search mods",
        "search mod",
    ];
    for k in search_keys {
        if t.contains(k) {
            let q = extract_search_query(&t).or_else(|| {
                t.split(k)
                    .nth(1)
                    .map(|s| {
                        s.trim()
                            .trim_matches(|c: char| c == '"' || c == '\'' || c == '「' || c == '」')
                            .trim()
                            .to_string()
                    })
                    .filter(|s| !s.is_empty())
            });
            if let Some(q) = q {
                return IntentResult {
                    intent: Intent::SearchMods { query: q.clone() },
                    confidence: 0.92,
                    matched: format!("search:{q}"),
                    reply_zh: format!("正在搜索模组「{q}」…"),
                };
            }
            return IntentResult {
                intent: Intent::Help,
                confidence: 0.6,
                matched: k.into(),
                reply_zh: "请告诉我要搜什么模组，例如「搜模组 sodium」。".into(),
            };
        }
    }
    // also 「搜 xxx」short form when not install
    if (t.starts_with("搜 ") || t.starts_with("搜索 ")) && !t.contains("装") {
        if let Some(q) = extract_search_query(&t) {
            return IntentResult {
                intent: Intent::SearchMods { query: q.clone() },
                confidence: 0.9,
                matched: format!("search:{q}"),
                reply_zh: format!("正在搜索模组「{q}」…"),
            };
        }
    }

    // suggest mods for pack theme
    let suggest_keys = [
        "推荐模组",
        "建议模组",
        "模组推荐",
        "补什么模组",
        "还缺什么模组",
        "suggest mods",
        "recommend mods",
        "搭配模组",
        "整合包推荐",
    ];
    for k in suggest_keys {
        if t.contains(k) {
            let theme = extract_suggest_theme(&t);
            return IntentResult {
                intent: Intent::SuggestMods {
                    theme: theme.clone(),
                },
                confidence: 0.9,
                matched: k.into(),
                reply_zh: format!("正在按「{theme}」主题推荐模组…"),
            };
        }
    }

    // install mod
    if t.contains("装") || t.contains("安装") || t.contains("install") || t.contains("下载") {
        if let Some(q) = extract_mod_query(&t) {
            return IntentResult {
                intent: Intent::InstallMod { query: q.clone() },
                confidence: 0.8,
                matched: format!("install:{q}"),
                reply_zh: format!("将搜索并安装模组「{q}」到当前版本。"),
            };
        }
    }

    let help_keys = ["帮助", "help", "能做什么", "怎么用", "?"];
    for k in help_keys {
        if t == k || t.contains(k) {
            return IntentResult {
                intent: Intent::Help,
                confidence: 1.0,
                matched: k.into(),
                reply_zh: help_text(),
            };
        }
    }

    IntentResult {
        intent: Intent::Unknown { raw: raw.clone() },
        confidence: 0.0,
        matched: String::new(),
        reply_zh: format!("未识别意图：「{raw}」。\n{}", help_text()),
    }
}

pub fn help_text() -> String {
    r#"我可以帮你：
· 「查看崩溃」— 分析崩溃 / 启动失败日志（也可粘贴日志路径）
· 「搜模组 sodium」— 在 Modrinth 搜索，给你可选列表（再说「装 xxx」安装）
· 「推荐模组 科技」— 按生存 / 科技 / 魔法等主题推荐互补模组
· 「检查模组」/「已装模组」— 列出当前版本已装模组
· 「禁用 sodium」/「启用 iris」— 启用或禁用模组（改名 .jar.disabled）
· 「版本列表」— 看看已安装了哪些版本
· 「切换版本 1.21.1」— 把当前版本切到匹配的那一个
· 「重命名版本 我的生存包」— 改当前版本显示名
· 「复制版本」— 复制当前版本（模组等一起拷，不含存档）
· 「删除版本 测试包」— 删掉版本和它的游戏文件夹（找不回来）
· 「删除模组 sodium」— 从当前版本永久删掉模组文件
· 「打开日志」— 定位当前版本的 latest.log / 崩溃报告
· 「账户列表」— 看看已保存的正版 / 离线账号
· 「切换账户 Steve」— 把当前账户切到匹配的那一个
· 「打开截图」— 定位当前版本的截图文件夹
· 「打开模组文件夹」/「打开资源包」/「打开光影」/「打开存档」/「打开配置文件夹」/「打开日志文件夹」— 定位常见游戏文件夹
· 「导入整合包 /path/to/pack.mrpack」— 把本地 .mrpack 装成新版本
· 「制作整合包」— 从当前版本导出 Modrinth .mrpack 整合包
· 「一键环境」— 按当前版本准备 Java，并汇报内存与下载源
· 「优化配置」— 生成内存 / JVM 建议更改，你确认后再应用
· 「切换 BMCLAPI 加速」— 换国内镜像下载
· 「打开版本文件夹」— 定位当前版本目录
· 「修好 Java」— 检查并准备合适的 Java
· 「内存 4G」— 立刻把游戏内存调大
· 「游戏文件夹」— EasyMC 数据根目录
· 「导出报告」— 打包日志方便发给别人
· 「装 sodium」— 搜索并安装到当前版本

默认只在你电脑上跑规则诊断；配置了云端助手时才会联网。"#
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_diagnose() {
        let r = route("看上次报错");
        assert!(matches!(r.intent, Intent::Diagnose { log_path: None, .. }));
    }

    #[test]
    fn route_search_and_suggest_and_pack() {
        match route("搜模组 sodium").intent {
            Intent::SearchMods { query } => assert!(query.to_lowercase().contains("sodium")),
            other => panic!("{:?}", other),
        }
        match route("推荐模组 科技").intent {
            Intent::SuggestMods { theme } => assert_eq!(theme, "tech"),
            other => panic!("{:?}", other),
        }
        assert!(matches!(route("制作整合包").intent, Intent::BuildPack { .. }));
        assert!(matches!(route("已装模组").intent, Intent::CheckMods { .. }));
        match route("禁用 sodium").intent {
            Intent::ToggleMod { query, enable, .. } => {
                assert!(query.contains("sodium"));
                assert_eq!(enable, Some(false));
            }
            other => panic!("{:?}", other),
        }
        assert!(matches!(route("版本列表").intent, Intent::ListInstances));
        match route("切换版本 1.21.1").intent {
            Intent::SelectInstance { query } => assert!(query.contains("1.21")),
            other => panic!("{:?}", other),
        }
        assert!(matches!(
            route("打开日志").intent,
            Intent::OpenLatestLog { .. }
        ));
        assert!(matches!(route("账户列表").intent, Intent::ListAccounts));
        match route("切换账户 Steve").intent {
            Intent::SelectAccount { query } => assert!(query.to_lowercase().contains("steve")),
            other => panic!("{:?}", other),
        }
        assert!(matches!(
            route("打开截图").intent,
            Intent::OpenScreenshots { .. }
        ));
        assert!(matches!(
            route("打开模组文件夹").intent,
            Intent::OpenGameFolder {
                folder: GameFolderKind::Mods,
                ..
            }
        ));
        assert!(matches!(
            route("打开资源包").intent,
            Intent::OpenGameFolder {
                folder: GameFolderKind::ResourcePacks,
                ..
            }
        ));
        assert!(matches!(
            route("打开光影").intent,
            Intent::OpenGameFolder {
                folder: GameFolderKind::ShaderPacks,
                ..
            }
        ));
        assert!(matches!(
            route("打开存档").intent,
            Intent::OpenGameFolder {
                folder: GameFolderKind::Saves,
                ..
            }
        ));
        assert!(matches!(
            route("打开配置文件夹").intent,
            Intent::OpenGameFolder {
                folder: GameFolderKind::Config,
                ..
            }
        ));
        assert!(matches!(
            route("打开日志文件夹").intent,
            Intent::OpenGameFolder {
                folder: GameFolderKind::Logs,
                ..
            }
        ));
        assert!(matches!(
            route("打开崩溃文件夹").intent,
            Intent::OpenGameFolder {
                folder: GameFolderKind::CrashReports,
                ..
            }
        ));
        match route("重命名版本 生存包").intent {
            Intent::RenameInstance { query, new_name } => {
                assert!(query.is_none());
                assert!(new_name.contains("生存"));
            }
            other => panic!("{:?}", other),
        }
        assert!(matches!(
            route("复制版本").intent,
            Intent::DuplicateInstance { .. }
        ));
        match route("复制版本 硬核包").intent {
            Intent::DuplicateInstance { new_name, .. } => {
                assert_eq!(new_name.as_deref(), Some("硬核包"));
            }
            other => panic!("{:?}", other),
        }
        match route("删除版本 测试包").intent {
            Intent::DeleteInstance { query } => assert!(query.contains("测试")),
            other => panic!("{:?}", other),
        }
        assert!(matches!(
            route("删除当前版本").intent,
            Intent::DeleteInstance { .. }
        ));
        match route("删除模组 sodium").intent {
            Intent::DeleteMod { query, .. } => assert!(query.contains("sodium")),
            other => panic!("{:?}", other),
        }
        match route("导入整合包 /tmp/demo.mrpack").intent {
            Intent::ImportMrpack { path } => assert!(path.contains("mrpack")),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn route_diagnose_chinese_phrases() {
        for s in ["闪退了", "启动失败", "打不开游戏", "游戏崩了", "黑屏", "查看崩溃"] {
            let r = route(s);
            assert!(
                matches!(r.intent, Intent::Diagnose { .. }),
                "expected Diagnose for {s:?}, got {:?}",
                r.intent
            );
        }
    }

    #[test]
    fn route_java() {
        let r = route("确保 Java 21");
        assert_eq!(r.intent, Intent::EnsureJava { major: 21 });
    }

    #[test]
    fn route_fix_java_shortcut() {
        let r = route("修好 Java");
        assert!(matches!(r.intent, Intent::EnsureJava { .. }));
    }

    #[test]
    fn route_memory_oom_phrase() {
        let r = route("内存不够 8G");
        match r.intent {
            Intent::BumpMemory { mb } => assert_eq!(mb, 8192),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn route_sodium() {
        let r = route("装 sodium");
        match r.intent {
            Intent::InstallMod { query } => assert!(query.to_lowercase().contains("sodium")),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn route_tune_setup_pack_mirror() {
        assert!(matches!(route("优化配置").intent, Intent::TuneConfig { .. }));
        assert!(matches!(route("一键环境").intent, Intent::SetupEnv { .. }));
        assert!(matches!(route("生成整合包").intent, Intent::BuildPack { .. }));
        match route("切换 BMCLAPI 加速").intent {
            Intent::SetDownloadSource { source } => assert_eq!(source, "bmclapi"),
            other => panic!("{:?}", other),
        }
        assert!(matches!(
            route("打开版本文件夹").intent,
            Intent::OpenInstanceDir { .. }
        ));
        assert!(matches!(route("检查模组").intent, Intent::CheckMods { .. }));
    }
}
