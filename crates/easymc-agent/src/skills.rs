//! Lightweight skill injection (dsh-style: pick relevant skills by keywords; do not dump all).
//! Skill files live in repo `skills/*.md`. Runtime loads a few matches into the system prompt.

use std::path::PathBuf;

use crate::llm::system_prompt_zh;

#[derive(Clone, Copy)]
struct SkillIndex {
    file: &'static str,
    /// Lowercase keywords; any hit selects the skill.
    keywords: &'static [&'static str],
}

const SKILL_INDEX: &[SkillIndex] = &[
    SkillIndex {
        file: "agent-see-act-verify.md",
        keywords: &[
            "崩溃", "闪退", "卡死", "打不开", "启动不了", "diagnose", "crash", "日志", "分析",
            "修好", "看崩", "error", "exception",
        ],
    },
    SkillIndex {
        file: "agent-delete-version-mod.md",
        keywords: &["删除版本", "删版本", "删除模组", "删模组", "卸载模组", "delete_instance", "delete_mod"],
    },
    SkillIndex {
        file: "agent-mod-toggle-mrpack.md",
        keywords: &[
            "禁用", "启用", "整合包", "mrpack", "toggle", "导入整合", "制作整合",
        ],
    },
    SkillIndex {
        file: "agent-game-folders.md",
        keywords: &[
            "模组文件夹", "资源包", "光影", "存档", "配置文件夹", "日志文件夹", "崩溃文件夹",
            "打开模组", "open_game_folder",
        ],
    },
    SkillIndex {
        file: "agent-select-log-accounts.md",
        keywords: &["切换版本", "版本列表", "打开日志", "账户", "切换账户"],
    },
    SkillIndex {
        file: "agent-rename-duplicate-version.md",
        keywords: &["重命名", "复制版本", "副本"],
    },
    SkillIndex {
        file: "agent-account-screenshots.md",
        keywords: &["截图", "screenshots", "账户列表"],
    },
];

fn skill_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("skills"));
        dirs.push(cwd.join("easymc").join("skills"));
    }
    // Crate-relative when running from workspace target/
    let manifest_skills = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("skills");
    dirs.push(manifest_skills);
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("skills"));
            dirs.push(parent.join("resources").join("skills"));
        }
    }
    dirs
}

fn read_skill_body(file: &str) -> Option<String> {
    for dir in skill_search_dirs() {
        let path = dir.join(file);
        if path.is_file() {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                return Some(strip_frontmatter(&raw));
            }
        }
    }
    // Built-in fallback for the see→act→verify skill (always available).
    if file == "agent-see-act-verify.md" {
        return Some(BUILTIN_SEE_ACT_VERIFY.into());
    }
    None
}

fn strip_frontmatter(raw: &str) -> String {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            return rest[end + 4..].trim().to_string();
        }
    }
    t.to_string()
}

/// Pick up to `limit` skill bodies whose keywords appear in `user_text`.
pub fn select_skills_for_prompt(user_text: &str, limit: usize) -> Vec<(String, String)> {
    let lower = user_text.to_lowercase();
    let mut out = Vec::new();
    for sk in SKILL_INDEX {
        if out.len() >= limit {
            break;
        }
        let hit = sk.keywords.iter().any(|k| {
            let kk = k.to_lowercase();
            lower.contains(&kk) || user_text.contains(*k)
        });
        if !hit {
            continue;
        }
        if let Some(body) = read_skill_body(sk.file) {
            let clipped = if body.chars().count() > 1200 {
                body.chars().take(1200).collect::<String>() + "…"
            } else {
                body
            };
            out.push((sk.file.to_string(), clipped));
        }
    }
    out
}

/// System prompt + 0..=3 matched skills.
pub fn skill_augmented_system_prompt(user_text: &str) -> String {
    let mut prompt = system_prompt_zh().to_string();
    let skills = select_skills_for_prompt(user_text, 3);
    if skills.is_empty() {
        return prompt;
    }
    prompt.push_str("\n\n---\n以下是与本次请求相关的本地 skill（按需遵循，勿逐字朗读给玩家）：\n");
    for (name, body) in skills {
        prompt.push_str(&format!("\n### {name}\n{body}\n"));
    }
    prompt
}

const BUILTIN_SEE_ACT_VERIFY: &str = r#"# Agent：see → act → verify（崩溃排查）

## 何时用
启动失败、闪退、卡在标题画面、日志报错、模组冲突嫌疑。

## 步骤
1. **See（先看）**：调用 `analyze_crash_log` / `diagnose`；需要时再 `open_latest_log`、`list_installed_mods`。
2. **Act（再做）**：根据结果提出可逆建议（`propose_changes` / 优化配置）；危险操作（删模组、删版本、禁用）只说明，等玩家确认后再走本地规则。
3. **Verify（再验）**：改完后再次诊断或让玩家重启后再「查看崩溃」，对照前后摘要。

## 注意
- 玩家向中文；一次只推进一步，别一次删很多模组。
- 不要提 MCBBS；不要编造不存在的命令。
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_crash_skill() {
        let hits = select_skills_for_prompt("游戏崩溃了打不开", 3);
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|(n, _)| n.contains("see-act-verify")));
    }

    #[test]
    fn no_dump_on_unrelated() {
        // Still may match nothing useful
        let hits = select_skills_for_prompt("今天天气怎么样", 3);
        assert!(hits.len() <= 3);
    }
}
