//! Crash / failure pattern rules — keyword + light regex, offline, zh-CN tips.

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: FindingSeverity,
    pub title: String,
    pub summary: String,
    pub fixes: Vec<String>,
    /// Optional local action id the UI/CLI can offer (ensure_java, bump_memory, …).
    pub action: Option<String>,
    pub evidence: Option<String>,
}

struct Rule {
    id: &'static str,
    severity: FindingSeverity,
    /// Any of these substrings (case-insensitive) must match.
    needles: &'static [&'static str],
    /// Optional regex; if set, must also match.
    re: Option<&'static str>,
    title: &'static str,
    summary: &'static str,
    fixes: &'static [&'static str],
    action: Option<&'static str>,
}

fn rules() -> &'static [Rule] {
    &[
        Rule {
            id: "oom",
            severity: FindingSeverity::Error,
            needles: &[
                "outofmemoryerror",
                "java.lang.OutOfMemoryError",
                "out of memory",
                "GC overhead limit exceeded",
                "Java heap space",
                "内存不足",
                "内存溢出",
                "堆空间",
            ],
            re: None,
            title: "内存不足（OutOfMemory）",
            summary: "JVM 堆内存耗尽，常见于模组较多或分配过小。",
            fixes: &[
                "在设置或启动参数中提高 -Xmx（例如 4096M / 6144M）",
                "关闭占用高的光影/高清材质，或减少同时加载的模组",
                "Agent 可执行：提高建议内存",
            ],
            action: Some("bump_memory"),
        },
        Rule {
            id: "missing_java",
            severity: FindingSeverity::Error,
            needles: &[
                "unsupportedclassversionerror",
                "no java",
                "unable to locate java",
                "java not found",
                "Invalid maximum heap size",
                "错误的主版本号",
                "class file version",
                "找不到 java",
                "未找到 java",
                "没有可用的 java",
                "Unsupported class file major version",
            ],
            re: None,
            title: "Java 缺失或版本不对",
            summary: "找不到合适的 Java，或 class 文件版本高于当前 JVM。",
            fixes: &[
                "点助手「修好 Java」，自动准备合适版本",
                "1.21.x 一般需要 Java 21；1.17–1.20 多用 Java 17",
                "仍不行可到设置高级项查看数据目录里的 runtimes",
            ],
            action: Some("ensure_java"),
        },
        Rule {
            id: "mixin_conflict",
            severity: FindingSeverity::Error,
            needles: &[
                "mixin apply",
                "mixin prepare",
                "@mixin",
                "Mixin apply for mod",
                "critical injection failure",
                "MixinTransformerError",
                "MixinConstraintException",
                "模组冲突",
                "mod conflict",
                "incompatible with",
                "Cannot apply mixin",
            ],
            re: None,
            title: "Mixin / 模组冲突",
            summary: "两个模组改了同一处代码，或模组与加载器版本不匹配。",
            fixes: &[
                "对照崩溃栈里的 modid，暂时移出冲突模组再启动",
                "确认模组支持当前 MC + Fabric/Quilt/Forge/NeoForge 版本",
                "查看是否需要更新到更新的模组构建",
            ],
            action: None,
        },
        Rule {
            id: "wrong_loader",
            severity: FindingSeverity::Error,
            needles: &[
                "modresolutionexception",
                "incompatible mods found",
                "missing or unsupported mandatory dependencies",
                "requires fabric",
                "requires forge",
                "requires neoforge",
                "not a valid mod file",
                "Mod Loading has halted",
                "Failed to find main class",
                "缺少依赖",
                "依赖缺失",
                "加载器不匹配",
            ],
            re: None,
            title: "加载器或模组不匹配",
            summary: "实例加载器与模组要求不一致，或依赖缺失。",
            fixes: &[
                "确认实例是 Fabric / Quilt / Forge / NeoForge 中正确的一种",
                "到「商店」按当前加载器筛选后重装模组",
                "检查 mods/ 下是否混入了其他加载器的 jar",
            ],
            action: None,
        },
        Rule {
            id: "opengl_glfw",
            severity: FindingSeverity::Error,
            needles: &[
                "glfwerror",
                "failed to create the glfw window",
                "no opengl context",
                "opengl 3.2",
                "Could not find a valid Vulkan",
                "Failed to create window",
                "GLFW error",
                "Failed to create Display",
                "Pixel format not accelerated",
            ],
            re: None,
            title: "图形 / OpenGL / GLFW 问题",
            summary: "无法创建游戏窗口，常见于无显示器、驱动过旧或远程会话。",
            fixes: &[
                "在有 GPU/显示器的桌面环境启动（需要 DISPLAY）",
                "更新显卡驱动；Linux 确认安装了 libgl1 / mesa",
                "头无服务器无法出窗口属预期，请在本机 GUI 再试",
            ],
            action: None,
        },
        Rule {
            id: "openal",
            severity: FindingSeverity::Warn,
            needles: &[
                "failed to open openal",
                "error starting soundsystem",
                "openal",
                "Could not locate OpenAL",
            ],
            re: None,
            title: "音频（OpenAL）不可用",
            summary: "找不到声卡/OpenAL，游戏通常仍可运行但无声音。",
            fixes: &[
                "安装/启用声卡驱动与 OpenAL 库（如 libopenal1）",
                "可忽略此警告继续游玩",
            ],
            action: None,
        },
        Rule {
            id: "auth_session",
            severity: FindingSeverity::Warn,
            needles: &[
                "invalid session",
                "Failed to login",
                "401 Unauthorized",
                "Not authenticated with Minecraft.net",
                "multiplayer.disconnect.unverified_username",
            ],
            re: None,
            title: "正版验证 / 会话问题",
            summary: "离线账户无法进正版验证服；或微软会话过期。",
            fixes: &[
                "单人/离线服可用离线账户；正版多人服需 Microsoft 登录（尚未接通）",
                "检查系统时间是否准确",
            ],
            action: None,
        },
        Rule {
            id: "assets_missing",
            severity: FindingSeverity::Warn,
            needles: &[
                "hash check failed",
                "failed to download",
                "Couldn't load assets",
                "Unable to locate assets",
            ],
            re: Some(r"(?i)(assets|assetIndex|objects/[0-9a-f]{2}/)"),
            title: "游戏资源可能不完整",
            summary: "资源索引或 objects 缺失，标题画面纹理/音效可能异常。",
            fixes: &[
                "重新安装版本并勾选「下载游戏资源」（不要用 --no-assets）",
                "删除损坏的 assets/objects 后重试",
            ],
            action: None,
        },
        Rule {
            id: "duplicate_module",
            severity: FindingSeverity::Error,
            needles: &[
                "duplicate key",
                "ResolutionException",
                "export package",
                "Modules minecraft and",
            ],
            re: None,
            title: "模块路径 / 类路径冲突",
            summary: "Forge/NeoForge 启动时 classpath 含重复 jar 或错误的原版客户端 jar。",
            fixes: &[
                "更新 EasyMC 到含 classpath 去重的版本后重装加载器",
                "删除 versions/ 下损坏的 forge/neoforge 配置后重新 install-forge/neoforge",
            ],
            action: None,
        },
        Rule {
            id: "exit_nonzero",
            severity: FindingSeverity::Warn,
            needles: &["进程已退出", "exit code", "Exit code"],
            re: Some(r"(?i)(exit.?code[=:\s]+[1-9]|非正常退出|进程已退出:.*[^0])"),
            title: "进程异常退出",
            summary: "游戏进程已结束且可能非正常退出，请结合上方其它诊断项。",
            fixes: &["查看完整日志末尾的 Exception 堆栈", "导出诊断包发给自己存档"],
            action: Some("diagnostics"),
        },
    ]
}

/// Scan log text; return unique findings (first evidence line kept).
pub fn scan_log(text: &str) -> Vec<Finding> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    for rule in rules() {
        let needle_hit = rule
            .needles
            .iter()
            .any(|n| lower.contains(&n.to_ascii_lowercase()));
        if !needle_hit {
            continue;
        }
        if let Some(pat) = rule.re {
            let Ok(re) = Regex::new(pat) else { continue };
            if !re.is_match(text) {
                continue;
            }
        }
        let evidence = text
            .lines()
            .find(|l| {
                let ll = l.to_ascii_lowercase();
                rule.needles.iter().any(|n| ll.contains(&n.to_ascii_lowercase()))
            })
            .map(|s| {
                let t = s.trim();
                if t.len() > 240 {
                    format!("{}…", &t[..237])
                } else {
                    t.to_string()
                }
            });
        out.push(Finding {
            id: rule.id.into(),
            severity: rule.severity,
            title: rule.title.into(),
            summary: rule.summary.into(),
            fixes: rule.fixes.iter().map(|s| (*s).to_string()).collect(),
            action: rule.action.map(|s| s.to_string()),
            evidence,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_oom() {
        let f = scan_log("java.lang.OutOfMemoryError: Java heap space");
        assert!(f.iter().any(|x| x.id == "oom"));
    }

    #[test]
    fn detects_oom_zh() {
        let f = scan_log("启动失败：内存不足 / 堆空间耗尽");
        assert!(f.iter().any(|x| x.id == "oom"));
    }

    #[test]
    fn detects_mixin_conflict() {
        let f = scan_log("@Mixin apply failed: MixinTransformerError critical injection failure");
        assert!(f.iter().any(|x| x.id == "mixin_conflict"));
    }

    #[test]
    fn detects_missing_java_zh() {
        let f = scan_log("错误：找不到 Java / Unsupported class file major version 65");
        assert!(f.iter().any(|x| x.id == "missing_java"));
    }

    #[test]
    fn detects_openal() {
        let f = scan_log("Failed to open OpenAL device");
        assert!(f.iter().any(|x| x.id == "openal"));
    }
}
