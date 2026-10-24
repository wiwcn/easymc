//! Structured change proposals: preview → Accept(应用更改) → Undo(撤销).
//! Maps client JE JVM knobs to EasyMC `settings.memory_mb` / instance `memory_mb` + `jvm_args`.

use serde::{Deserialize, Serialize};

use easymc_core::{
    load_instances, load_settings, save_settings, update_instance_fields, Paths, Result,
};

use crate::actions::ActionResult;

/// One display row in the proposal card (Setting | Current | Proposed | Delta).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposeItem {
    pub key: String,
    pub label_zh: String,
    pub current: String,
    pub proposed: String,
    pub delta_zh: String,
}

/// Payload written when the player accepts the proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplyPayload {
    /// Prefer instance override when set; otherwise settings.memory_mb only.
    pub instance_id: Option<String>,
    pub memory_mb: Option<u32>,
    /// Full replacement for instance.jvm_args (whitespace-separated flags).
    pub jvm_args: Option<String>,
    /// When true, clear instance.jvm_args (None).
    #[serde(default)]
    pub clear_jvm_args: bool,
}

/// Snapshot to restore on Undo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UndoSnapshot {
    pub instance_id: Option<String>,
    pub settings_memory_mb: u32,
    /// Previous instance.memory_mb; meaningful when `instance_had_memory`.
    pub instance_memory_mb: Option<u32>,
    pub instance_had_memory: bool,
    pub instance_jvm_args: Option<String>,
    pub instance_had_jvm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeProposal {
    pub id: String,
    pub title_zh: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_zh: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle_zh: Option<String>,
    pub items: Vec<ProposeItem>,
    pub apply_payload: ApplyPayload,
    pub undo_snapshot: UndoSnapshot,
}

fn fmt_mem(mb: u32) -> String {
    if mb >= 1024 && mb % 1024 == 0 {
        format!("{}G", mb / 1024)
    } else {
        format!("{mb}M")
    }
}

fn delta_mem(from: u32, to: u32) -> String {
    if to > from {
        format!("↑ +{}", fmt_mem(to - from))
    } else if to < from {
        format!("↓ −{}", fmt_mem(from - to))
    } else {
        "不变".into()
    }
}

fn jvm_has_flag(args: &str, flag: &str) -> bool {
    args.split_whitespace()
        .any(|a| a == flag || a.starts_with(&format!("{flag}=")))
}

/// Sensible client JVM extras when empty / missing G1.
pub fn suggested_client_jvm(current: &str) -> String {
    let mut parts: Vec<String> = current
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let cur = parts.join(" ");
    if !jvm_has_flag(&cur, "-XX:+UseG1GC") {
        parts.push("-XX:+UseG1GC".into());
    }
    if !jvm_has_flag(&cur, "-XX:G1HeapRegionSize") {
        parts.push("-XX:G1HeapRegionSize=16M".into());
    }
    if !jvm_has_flag(&cur, "-XX:+AlwaysPreTouch") {
        parts.push("-XX:+AlwaysPreTouch".into());
    }
    parts.join(" ")
}

fn new_id(prefix: &str) -> String {
    let stamp = chrono::Local::now().format("%Y%m%d%H%M%S%3f");
    format!("{prefix}-{stamp}")
}

fn resolve_effective_memory(
    paths: &Paths,
    instance_id: Option<&str>,
) -> (u32, Option<String>, Option<String>) {
    let settings = load_settings(paths).unwrap_or_default();
    let store = load_instances(paths).ok();
    let id = instance_id
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| store.as_ref().and_then(|s| s.active_id.clone()));
    if let (Some(id), Some(store)) = (&id, &store) {
        if let Some(inst) = store.instances.iter().find(|i| i.id == *id) {
            let mem = inst
                .memory_mb
                .filter(|m| *m >= 512)
                .unwrap_or(settings.memory_mb);
            return (mem, Some(id.clone()), inst.jvm_args.clone());
        }
    }
    (settings.memory_mb, id, None)
}

/// Build a tune / OOM recovery proposal (memory + client JVM flags).
pub fn build_tune_proposal(
    paths: &Paths,
    instance_id: Option<&str>,
    suggested_mb: Option<u32>,
    reason_zh: &str,
) -> ChangeProposal {
    let settings = load_settings(paths).unwrap_or_default();
    let (current_mem, resolved_id, current_jvm) = resolve_effective_memory(paths, instance_id);
    let target_mb = suggested_mb
        .unwrap_or_else(|| {
            if current_mem < 4096 {
                4096
            } else if current_mem < 6144 {
                6144
            } else {
                current_mem.max(4096)
            }
        })
        .clamp(1024, 32768);

    let cur_jvm = current_jvm.clone().unwrap_or_default();
    let proposed_jvm = suggested_client_jvm(&cur_jvm);
    let jvm_changing = proposed_jvm != cur_jvm.trim();

    let xms_cur = current_mem.min(1024);
    let xms_new = target_mb.min(1024);

    let mut items = vec![
        ProposeItem {
            key: "xms".into(),
            label_zh: "-Xms".into(),
            current: fmt_mem(xms_cur),
            proposed: fmt_mem(xms_new),
            delta_zh: if xms_new == xms_cur {
                "不变".into()
            } else {
                delta_mem(xms_cur, xms_new)
            },
        },
        ProposeItem {
            key: "xmx".into(),
            label_zh: "-Xmx / 内存".into(),
            current: fmt_mem(current_mem),
            proposed: fmt_mem(target_mb),
            delta_zh: delta_mem(current_mem, target_mb),
        },
    ];

    let g1_cur = jvm_has_flag(&cur_jvm, "-XX:+UseG1GC");
    let g1_new = jvm_has_flag(&proposed_jvm, "-XX:+UseG1GC");
    items.push(ProposeItem {
        key: "use_g1gc".into(),
        label_zh: "-XX:+UseG1GC".into(),
        current: if g1_cur { "true" } else { "false" }.into(),
        proposed: if g1_new { "true" } else { "false" }.into(),
        delta_zh: if g1_cur == g1_new {
            "不变".into()
        } else if g1_new {
            "已启用".into()
        } else {
            "关闭".into()
        },
    });

    let region_cur = cur_jvm
        .split_whitespace()
        .find(|a| a.starts_with("-XX:G1HeapRegionSize"))
        .map(|a| a.trim_start_matches("-XX:G1HeapRegionSize=").to_string())
        .unwrap_or_else(|| "（未设）".into());
    let region_new = proposed_jvm
        .split_whitespace()
        .find(|a| a.starts_with("-XX:G1HeapRegionSize"))
        .map(|a| a.trim_start_matches("-XX:G1HeapRegionSize=").to_string())
        .unwrap_or_else(|| "（未设）".into());
    items.push(ProposeItem {
        key: "g1_region".into(),
        label_zh: "-XX:G1HeapRegionSize".into(),
        current: region_cur.clone(),
        proposed: region_new.clone(),
        delta_zh: if region_cur == region_new {
            "不变".into()
        } else {
            "优化".into()
        },
    });

    let apt_cur = jvm_has_flag(&cur_jvm, "-XX:+AlwaysPreTouch");
    let apt_new = jvm_has_flag(&proposed_jvm, "-XX:+AlwaysPreTouch");
    items.push(ProposeItem {
        key: "always_pretouch".into(),
        label_zh: "-XX:+AlwaysPreTouch".into(),
        current: if apt_cur { "true" } else { "false" }.into(),
        proposed: if apt_new { "true" } else { "false" }.into(),
        delta_zh: if apt_cur == apt_new {
            "不变".into()
        } else if apt_new {
            "已启用".into()
        } else {
            "关闭".into()
        },
    });

    let (inst_had_mem, inst_mem, inst_had_jvm, inst_jvm) = if let Some(ref id) = resolved_id {
        let store = load_instances(paths).ok();
        let inst = store
            .as_ref()
            .and_then(|s| s.instances.iter().find(|i| i.id == *id));
        (
            inst.and_then(|i| i.memory_mb).is_some(),
            inst.and_then(|i| i.memory_mb),
            inst.and_then(|i| i.jvm_args.as_ref()).is_some(),
            inst.and_then(|i| i.jvm_args.clone()),
        )
    } else {
        (false, None, false, None)
    };

    // JVM flags live on the instance; without an instance we still propose memory via settings.
    let jvm_for_apply = if jvm_changing && resolved_id.is_some() {
        Some(proposed_jvm)
    } else {
        None
    };

    ChangeProposal {
        id: new_id("tune"),
        title_zh: "建议更改".into(),
        tag_zh: Some("启动参数".into()),
        subtitle_zh: Some(reason_zh.to_string()),
        items,
        apply_payload: ApplyPayload {
            instance_id: resolved_id.clone(),
            memory_mb: Some(target_mb),
            jvm_args: jvm_for_apply,
            clear_jvm_args: false,
        },
        undo_snapshot: UndoSnapshot {
            instance_id: resolved_id,
            settings_memory_mb: settings.memory_mb,
            instance_memory_mb: inst_mem,
            instance_had_memory: inst_had_mem,
            instance_jvm_args: inst_jvm,
            instance_had_jvm: inst_had_jvm,
        },
    }
}

/// Apply accepted proposal via settings / instance update APIs.
pub fn apply_proposal(paths: &Paths, proposal: &ChangeProposal) -> Result<ActionResult> {
    let p = &proposal.apply_payload;
    let mut notes = Vec::new();

    if let Some(mb) = p.memory_mb {
        let mb = mb.clamp(1024, 32768);
        let mut s = load_settings(paths)?;
        let old = s.memory_mb;
        s.memory_mb = mb;
        save_settings(paths, &s)?;
        notes.push(format!("全局内存 {old} → {mb} MB"));

        if let Some(id) = p.instance_id.as_deref() {
            update_instance_fields(paths, id, |inst| {
                inst.memory_mb = Some(mb);
            })?;
            notes.push(format!("版本内存覆盖 → {mb} MB"));
        }
    }

    if let Some(id) = p.instance_id.as_deref() {
        if p.clear_jvm_args {
            update_instance_fields(paths, id, |inst| {
                inst.jvm_args = None;
            })?;
            notes.push("已清除版本 JVM 参数".into());
        } else if let Some(args) = &p.jvm_args {
            let trimmed = args.trim().to_string();
            update_instance_fields(paths, id, |inst| {
                inst.jvm_args = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.clone())
                };
            })?;
            notes.push("已写入版本 JVM 参数（G1 等）".into());
        }
    }

    Ok(ActionResult {
        ok: true,
        action: "apply_changes".into(),
        message_zh: format!("已应用更改，下次启动生效。\n{}", notes.join("；")),
        path: None,
        value: Some(serde_json::to_value(proposal).unwrap_or_default()),
    })
}

pub fn undo_proposal(paths: &Paths, proposal: &ChangeProposal) -> Result<ActionResult> {
    let u = &proposal.undo_snapshot;

    let mut s = load_settings(paths)?;
    s.memory_mb = u.settings_memory_mb;
    save_settings(paths, &s)?;

    if let Some(id) = u.instance_id.as_deref() {
        update_instance_fields(paths, id, |inst| {
            if u.instance_had_memory {
                inst.memory_mb = u.instance_memory_mb;
            } else {
                inst.memory_mb = None;
            }
            if u.instance_had_jvm {
                inst.jvm_args = u.instance_jvm_args.clone();
            } else {
                inst.jvm_args = None;
            }
        })?;
    }

    Ok(ActionResult {
        ok: true,
        action: "undo_changes".into(),
        message_zh: "已撤销更改，恢复为应用前的设置。".into(),
        path: None,
        value: Some(serde_json::json!({ "proposal_id": proposal.id })),
    })
}

pub fn proposal_to_action(proposal: ChangeProposal, message_zh: impl Into<String>) -> ActionResult {
    ActionResult {
        ok: true,
        action: "propose_changes".into(),
        message_zh: message_zh.into(),
        path: None,
        value: Some(serde_json::to_value(&proposal).unwrap_or_default()),
    }
}

pub fn parse_proposal(value: &serde_json::Value) -> Option<ChangeProposal> {
    serde_json::from_value(value.clone()).ok()
}

/// Heuristic Java major for an MC version string (client JE).
pub fn java_major_for_mc(version: &str) -> u32 {
    let v = version.trim().trim_start_matches('v');
    let mut parts = v.split('.');
    let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    if major >= 2 || (major == 1 && minor >= 21) {
        21
    } else if major == 1 && minor >= 17 {
        17
    } else {
        8
    }
}

/// Suggested memory after OOM / low-RAM finding.
pub fn oom_suggested_mb(current_mb: u32) -> u32 {
    let bump = if current_mb < 4096 {
        4096
    } else if current_mb < 6144 {
        6144
    } else {
        current_mb.saturating_add(2048).min(16384)
    };
    bump.clamp(1024, 32768)
}

#[cfg(test)]
mod tests {
    use super::*;
    use easymc_core::{create_named_instance, McVersion, Paths};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_paths() -> Paths {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("easymc-propose-{n}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Paths::from_root(root)
    }

    #[test]
    fn suggested_jvm_adds_g1() {
        let s = suggested_client_jvm("");
        assert!(s.contains("-XX:+UseG1GC"));
        assert!(s.contains("-XX:+AlwaysPreTouch"));
        assert_eq!(suggested_client_jvm(&s), s);
    }

    #[test]
    fn java_major_heuristic() {
        assert_eq!(java_major_for_mc("1.21.1"), 21);
        assert_eq!(java_major_for_mc("1.20.4"), 17);
        assert_eq!(java_major_for_mc("1.16.5"), 8);
    }

    #[test]
    fn propose_apply_undo_roundtrip() {
        let paths = tmp_paths();
        let mut s = load_settings(&paths).unwrap();
        s.memory_mb = 2048;
        save_settings(&paths, &s).unwrap();

        let inst = create_named_instance(&paths, "Test", McVersion("1.21.1".into()), false).unwrap();

        let prop = build_tune_proposal(&paths, Some(&inst.id), Some(4096), "测试优化");
        assert_eq!(prop.apply_payload.memory_mb, Some(4096));
        assert!(prop.items.iter().any(|i| i.key == "xmx"));
        assert_eq!(prop.apply_payload.instance_id.as_deref(), Some(inst.id.as_str()));

        let applied = apply_proposal(&paths, &prop).unwrap();
        assert!(applied.ok);
        assert_eq!(applied.action, "apply_changes");

        let store = load_instances(&paths).unwrap();
        let inst2 = store.instances.iter().find(|i| i.id == inst.id).unwrap();
        assert_eq!(inst2.memory_mb, Some(4096));
        assert!(inst2
            .jvm_args
            .as_deref()
            .unwrap_or("")
            .contains("UseG1GC"));
        assert_eq!(load_settings(&paths).unwrap().memory_mb, 4096);

        let undone = undo_proposal(&paths, &prop).unwrap();
        assert!(undone.ok);
        let store = load_instances(&paths).unwrap();
        let inst3 = store.instances.iter().find(|i| i.id == inst.id).unwrap();
        assert!(inst3.memory_mb.is_none());
        assert!(inst3.jvm_args.is_none());
        assert_eq!(load_settings(&paths).unwrap().memory_mb, 2048);

        let _ = std::fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn proposal_serializes_action_shape() {
        let paths = tmp_paths();
        let prop = build_tune_proposal(&paths, None, Some(4096), "内存偏小");
        let ar = proposal_to_action(prop, "已生成优化建议，请确认后应用。");
        assert_eq!(ar.action, "propose_changes");
        let v = ar.value.unwrap();
        assert!(v.get("items").is_some());
        assert!(v.get("apply_payload").is_some());
        assert!(v.get("undo_snapshot").is_some());
        let _ = std::fs::remove_dir_all(&paths.root);
    }
}
