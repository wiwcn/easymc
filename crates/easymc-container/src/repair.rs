//! One-click container repair + mirror profile helpers (Slice E + mirror probe).
//!
//! Safe local steps only: rewrite registries, start Machine, re-probe, re-pull
//! runtime image, best-effort re-provision marked container versions.

use serde::{Deserialize, Serialize};

use easymc_core::{load_instances, Paths};

use crate::detect::{probe_container_runtime, ProbeStatus, RuntimeProbe};
use crate::image::{runtime_image_ready, ImagePullResult};
use crate::preheat::{preheat_container_runtime, PreheatResult};
use crate::registries::{
    prefer_image_ref_for_profile, write_registries_config_for_profile, ChinaRegistriesResult,
    DEFAULT_BASE_IMAGE, DEFAULT_BASE_IMAGE_MIRROR,
};
use crate::versions::provision_container_version;

/// Player-facing mirror profile for container image pulls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorProfile {
    /// DaoCloud public prefixes (default for CN networks).
    Daocloud,
    /// Community CN proxy prefix `dockerproxy.net`.
    Dockerproxy,
    /// Community CN proxy prefix `docker.1ms.run`.
    Onems,
    /// Official docker.io / upstream tags.
    Official,
}

impl MirrorProfile {
    pub const ALL: [MirrorProfile; 4] = [
        MirrorProfile::Daocloud,
        MirrorProfile::Dockerproxy,
        MirrorProfile::Onems,
        MirrorProfile::Official,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daocloud => "daocloud",
            Self::Dockerproxy => "dockerproxy",
            Self::Onems => "onems",
            Self::Official => "official",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Daocloud => "国内源（DaoCloud）",
            Self::Dockerproxy => "国内源（DockerProxy）",
            Self::Onems => "国内源（1ms）",
            Self::Official => "官方源",
        }
    }

    pub fn prefer_china(self) -> bool {
        !matches!(self, Self::Official)
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "daocloud" | "china" | "cn" | "国内" => Some(Self::Daocloud),
            "dockerproxy" | "dockerproxy.net" | "proxy" => Some(Self::Dockerproxy),
            "onems" | "1ms" | "docker.1ms.run" => Some(Self::Onems),
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
            Self::Official => DEFAULT_BASE_IMAGE,
            _ => DEFAULT_BASE_IMAGE_MIRROR,
        }
    }

    /// Host used for HTTPS latency probe (and registries.conf mirror host).
    pub fn probe_host(self) -> &'static str {
        match self {
            Self::Daocloud => "docker.m.daocloud.io",
            Self::Dockerproxy => "dockerproxy.net",
            Self::Onems => "docker.1ms.run",
            Self::Official => "registry-1.docker.io",
        }
    }

    /// HTTPS URL for real latency probe (TLS + HTTP, not bare TCP).
    /// Registry `/v2/` returns 200 or 401 — both count as reachable.
    pub fn probe_url(self) -> String {
        format!("https://{}/v2/", self.probe_host())
    }

    /// docker.io mirror hostname written into registries.conf / image refs.
    pub fn docker_io_mirror_host(self) -> Option<&'static str> {
        match self {
            Self::Daocloud => Some("docker.m.daocloud.io"),
            Self::Dockerproxy => Some("dockerproxy.net"),
            Self::Onems => Some("docker.1ms.run"),
            Self::Official => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorProfileInfo {
    pub id: String,
    pub label_zh: String,
    pub prefer_china: bool,
    pub sample_image: String,
    pub probe_host: String,
}

/// List mirror profiles for settings / onboarding UI.
pub fn list_mirror_profiles() -> Vec<MirrorProfileInfo> {
    MirrorProfile::ALL
        .into_iter()
        .map(|p| MirrorProfileInfo {
            id: p.as_str().into(),
            label_zh: p.label_zh().into(),
            prefer_china: p.prefer_china(),
            sample_image: prefer_image_ref_for_profile(DEFAULT_BASE_IMAGE, p),
            probe_host: p.probe_host().into(),
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
    repair_container_runtime_with_profile(
        paths,
        MirrorProfile::from_prefer_china(prefer_china),
        reprovision,
    )
}

/// Repair using an explicit mirror profile (DaoCloud / DockerProxy / 1ms / Official).
pub fn repair_container_runtime_with_profile(
    paths: &Paths,
    profile: MirrorProfile,
    reprovision: bool,
) -> RepairResult {
    let prefer_china = profile.prefer_china();
    let mut messages = Vec::new();
    let mut registries = None;

    if prefer_china {
        match write_registries_config_for_profile(profile) {
            Ok(r) => {
                messages.push(r.message_zh.clone());
                registries = Some(r);
            }
            Err(e) => messages.push(format!("镜像加速配置失败：{e}")),
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
