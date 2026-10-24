//! Keyword / phrase intent router — offline, zh-CN + English aliases.
//! Not an LLM. Expand the table as needed.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    Diagnose { instance_id: Option<String> },
    EnsureJava { major: u32 },
    /// Immediate write (shortcut「内存 4G」).
    BumpMemory { mb: u32 },
    /// Analyze + propose_changes card (memory + JVM); user must 应用更改.
    TuneConfig { instance_id: Option<String> },
    /// ensure_java + status summary for selected instance.
    SetupEnv { instance_id: Option<String> },
    /// Export share pack for current instance (host runs easymc-share).
    BuildPack { instance_id: Option<String> },
    SetDownloadSource { source: String },
    OpenInstanceDir { instance_id: Option<String> },
    CheckMods { instance_id: Option<String> },
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
        "生成整合包",
        "分享包",
        "导出整合包",
        "build pack",
        "buildpack",
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
                reply_zh: "正在为当前版本生成分享包…".into(),
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

    // check mods
    let mods_keys = [
        "检查模组",
        "模组冲突",
        "模组列表",
        "有多少模组",
        "看模组",
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
            return IntentResult {
                intent: Intent::Diagnose { instance_id: None },
                confidence: 0.9,
                matched: k.into(),
                reply_zh: "正在根据最近启动日志做本地规则诊断…".into(),
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
· 「查看崩溃」— 分析崩溃 / 启动失败日志
· 「一键环境」— 按当前版本准备 Java，并汇报内存与下载源
· 「优化配置」— 生成内存 / JVM 建议更改，你确认后再应用
· 「生成整合包」— 导出当前版本分享包
· 「切换 BMCLAPI 加速」— 换国内镜像下载
· 「打开版本文件夹」— 定位当前版本目录
· 「检查模组」— 看看装了多少模组
· 「修好 Java」— 检查并准备合适的 Java
· 「内存 4G」— 立刻把游戏内存调大（也会弹提示）
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
        assert!(matches!(r.intent, Intent::Diagnose { .. }));
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
