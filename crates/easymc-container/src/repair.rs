//! One-click container repair + mirror profile helpers (Slice E).
//!
//! Safe local steps only: rewrite registries, start Machine, re-probe, re-pull
//! runtime image, best-effort re-provision marked container versions.

use serde::{Deserialize, Serialize};

use easymc_core::{load_instances, Paths};

use crate::detect::{probe_container_runtime, ProbeStatus, RuntimeProbe};
use crate::image::{runtime_image_ready, ImagePullResult};
use crate::preheat::{preheat_container_runtime, PreheatResult};
use crate::registries::{
    write_china_registries_config, ChinaRegistriesResult, DEFAULT_BASE_IMAGE,
    DEFAULT_BASE_IMAGE_MIRROR,
};
use crate::versions::provision_container_version;

/// Player-facing mirror profile for container image pulls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorProfile {
    /// DaoCloud public prefixes (default for CN networks).
    Daocloud,
    /// Official docker.io / upstream tags.
    Official,
}

impl MirrorProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daocloud => "daocloud",
            Self::Official => "official",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Daocloud => "国内源（DaoCloud）",
            Self::Official => "官方源",
        }
    }

    pub fn prefer_china(self) -> bool {
        matches!(self, Self::Daocloud)
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "daocloud" | "china" | "cn" | "国内" => Some(Self::Daocloud),
            "official" | "docker" | "upstream" | "官方" => Some(Self::Official),
            _ => None,
        }
    }

    pub fn from_prefer_china(prefer: bool) -> Self {
        if prefer {
            Self::Daocloud
        } else {
            Self::Official
        }
    }

    pub fn default_image(self) -> &'static str {
        match self {
            Self::Daocloud => DEFAULT_BASE_IMAGE_MIRROR,
            Self::Official => DEFAULT_BASE_IMAGE,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorProfileInfo {
    pub id: String,
    pub label_zh: String,
    pub prefer_china: bool,
    pub sample_image: String,
}

/// List mirror profiles for settings UI.
pub fn list_mirror_profiles() -> Vec<MirrorProfileInfo> {
    [MirrorProfile::Daocloud, MirrorProfile::Official]
        .into_iter()
        .map(|p| MirrorProfileInfo {
            id: p.as_str().into(),
            label_zh: p.label_zh().into(),
            prefer_china: p.prefer_china(),
            sample_image: p.default_image().into(),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairResult {
    pub ok: bool,
    pub probe: RuntimeProbe,
    pub registries: Option<ChinaRegistriesResult>,
    pub preheat: Option<PreheatResult>,
    pub image: Option<ImagePullResult>,
    pub reprovisioned: usize,
    pub messages_zh: Vec<String>,
}

/// One-click repair: mirrors → machine/socket warm → pull image → re-provision.
///
/// Never runs remote installers. Pending Podman install still surfaced via probe.
pub fn repair_container_runtime(
    paths: &Paths,
    prefer_china: bool,
    reprovision: bool,
) -> RepairResult {
    let mut messages = Vec::new();
    let mut registries = None;

    if prefer_china {
        match write_china_registries_config() {
            Ok(r) => {
                messages.push(r.message_zh.clone());
                registries = Some(r);
            }
            Err(e) => messages.push(format!("镜像加速配置失败：{e}"),),
        }
    } else {
        messages.push("已选择官方源；跳过写入国内 registries 配置".into());
    }

    let preheat = preheat_container_runtime(prefer_china, true);
    messages.extend(preheat.messages_zh.iter().cloned());
    let image = preheat.image.clone();

    let mut reprovisioned = 0usize;
    if reprovision && preheat.ok {
        if let Ok(store) = load_instances(paths) {
            for inst in store.instances.iter().filter(|i| i.container.is_some()) {
                match provision_container_version(paths, &inst.id, prefer_china) {
                    Ok(r) => {
                        reprovisioned += 1;
                        messages.push(format!("已重修版本「{}」：{}", inst.name, r.message_zh));
                    }
                    Err(e) => {
                        messages.push(format!("重修版本「{}」失败：{e}", inst.name));
                    }
                }
            }
        }
    }

    let probe = probe_container_runtime();
    let ok = matches!(probe.status, ProbeStatus::Ready)
        && (image
            .as_ref()
            .map(|i| i.pulled || i.already_present)
            .unwrap_or_else(|| runtime_image_ready(prefer_china))
            || !prefer_china);

    if ok {
        messages.push("一键修复完成：容器运行时可用".into());
    } else if matches!(probe.status, ProbeStatus::NotInstalled) {
        messages.push("还差安装 Podman，请按一键配置指引完成".into());
    } else {
        messages.push("修复未完全就绪，请查看上方提示".into());
    }

    RepairResult {
        ok,
        probe,
        registries,
        preheat: Some(preheat),
        image,
        reprovisioned,
        messages_zh: messages,
    }
}
