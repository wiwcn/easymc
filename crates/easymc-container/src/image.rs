//! Pull / ensure Temurin JRE base image via China-friendly registries (Slice C).

use serde::{Deserialize, Serialize};

use crate::detect::{detect_podman, probe_container_runtime, ProbeStatus};
use crate::podman_util::{podman_bin, podman_cmd};
use crate::registries::{prefer_image_ref_for_profile, DEFAULT_BASE_IMAGE};
use crate::repair::MirrorProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePullResult {
    pub image: String,
    pub pulled: bool,
    pub already_present: bool,
    pub message_zh: String,
}

/// Resolve which image ref to use (DaoCloud prefix when prefer_china).
pub fn resolve_runtime_image(prefer_china: bool) -> String {
    resolve_runtime_image_for_profile(MirrorProfile::from_prefer_china(prefer_china))
}

pub fn resolve_runtime_image_for_profile(profile: MirrorProfile) -> String {
    prefer_image_ref_for_profile(DEFAULT_BASE_IMAGE, profile)
}

fn image_present(bin: &str, image: &str) -> bool {
    podman_cmd(bin, &["image", "exists", image]).is_ok()
}

/// Ensure the runtime image is local — pull via Podman (registries.conf mirrors apply).
pub fn ensure_runtime_image(prefer_china: bool) -> Result<ImagePullResult, String> {
    ensure_runtime_image_for_profile(MirrorProfile::from_prefer_china(prefer_china))
}

pub fn ensure_runtime_image_for_profile(profile: MirrorProfile) -> Result<ImagePullResult, String> {
    let probe = probe_container_runtime();
    if matches!(probe.status, ProbeStatus::Unsupported) {
        return Err(probe.message_zh);
    }
    if !matches!(probe.status, ProbeStatus::Ready) {
        return Err(format!(
            "隔离环境未就绪：{}。请先完成一键配置或启动运行环境。",
            probe.message_zh
        ));
    }
    let bin = podman_bin()?;
    let image = resolve_runtime_image_for_profile(profile);
    if image_present(&bin, &image) {
        return Ok(ImagePullResult {
            image,
            pulled: false,
            already_present: true,
            message_zh: "运行环境基础文件已在本地".into(),
        });
    }
    match podman_cmd(&bin, &["pull", &image]) {
        Ok(_) => Ok(ImagePullResult {
            message_zh: format!("已下载运行环境基础文件 {image}"),
            image,
            pulled: true,
            already_present: false,
        }),
        Err(e) if profile.prefer_china() => {
            let fallback = DEFAULT_BASE_IMAGE.to_string();
            podman_cmd(&bin, &["pull", &fallback]).map_err(|e2| {
                format!("下载基础文件失败（{}：{e}；官方线路：{e2}）", profile.label_zh())
            })?;
            Ok(ImagePullResult {
                image: fallback,
                pulled: true,
                already_present: false,
                message_zh: format!("{}失败，已改用官方线路下载 Java 运行环境", profile.label_zh()),
            })
        }
        Err(e) => Err(format!("下载基础文件失败：{e}")),
    }
}

/// Best-effort pull without requiring Ready (used during preheat after machine start).
pub fn try_pull_runtime_image(prefer_china: bool) -> ImagePullResult {
    try_pull_runtime_image_for_profile(MirrorProfile::from_prefer_china(prefer_china))
}

pub fn try_pull_runtime_image_for_profile(profile: MirrorProfile) -> ImagePullResult {
    match ensure_runtime_image_for_profile(profile) {
        Ok(r) => r,
        Err(msg) => ImagePullResult {
            image: resolve_runtime_image_for_profile(profile),
            pulled: false,
            already_present: false,
            message_zh: msg,
        },
    }
}

/// True when local podman reports the image.
pub fn runtime_image_ready(prefer_china: bool) -> bool {
    runtime_image_ready_for_profile(MirrorProfile::from_prefer_china(prefer_china))
}

pub fn runtime_image_ready_for_profile(profile: MirrorProfile) -> bool {
    let Some(pod) = detect_podman() else {
        return false;
    };
    if !pod.ready {
        return false;
    }
    image_present(&pod.path, &resolve_runtime_image_for_profile(profile))
}
