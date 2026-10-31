//! Pull / ensure Temurin JRE base image via China-friendly registries (Slice C).

use serde::{Deserialize, Serialize};

use crate::detect::{detect_podman, probe_container_runtime, ProbeStatus};
use crate::podman_util::{podman_bin, podman_cmd};
use crate::registries::{prefer_china_image_ref, DEFAULT_BASE_IMAGE, DEFAULT_BASE_IMAGE_MIRROR};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePullResult {
    pub image: String,
    pub pulled: bool,
    pub already_present: bool,
    pub message_zh: String,
}

/// Resolve which image ref to use (DaoCloud prefix when prefer_china).
pub fn resolve_runtime_image(prefer_china: bool) -> String {
    if prefer_china {
        DEFAULT_BASE_IMAGE_MIRROR.to_string()
    } else {
        prefer_china_image_ref(DEFAULT_BASE_IMAGE, false)
    }
}

fn image_present(bin: &str, image: &str) -> bool {
    podman_cmd(bin, &["image", "exists", image]).is_ok()
}

/// Ensure the runtime image is local — pull via Podman (registries.conf mirrors apply).
pub fn ensure_runtime_image(prefer_china: bool) -> Result<ImagePullResult, String> {
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
    let image = resolve_runtime_image(prefer_china);
    if image_present(&bin, &image) {
        return Ok(ImagePullResult {
            image,
            pulled: false,
            already_present: true,
            message_zh: "运行环境基础文件已在本地".into(),
        });
    }
    // Fall back: if China mirror fails, try public docker.io tag once.
    match podman_cmd(&bin, &["pull", &image]) {
        Ok(_) => Ok(ImagePullResult {
            message_zh: format!("已下载运行环境基础文件 {image}"),
            image,
            pulled: true,
            already_present: false,
        }),
        Err(e) if prefer_china => {
            let fallback = DEFAULT_BASE_IMAGE.to_string();
            podman_cmd(&bin, &["pull", &fallback]).map_err(|e2| {
                format!("下载基础文件失败（国内线路：{e}；官方线路：{e2}）")
            })?;
            Ok(ImagePullResult {
                image: fallback,
                pulled: true,
                already_present: false,
                message_zh: "国内线路失败，已改用官方线路下载 Java 运行环境".into(),
            })
        }
        Err(e) => Err(format!("下载基础文件失败：{e}")),
    }
}

/// Best-effort pull without requiring Ready (used during preheat after machine start).
pub fn try_pull_runtime_image(prefer_china: bool) -> ImagePullResult {
    match ensure_runtime_image(prefer_china) {
        Ok(r) => r,
        Err(msg) => ImagePullResult {
            image: resolve_runtime_image(prefer_china),
            pulled: false,
            already_present: false,
            message_zh: msg,
        },
    }
}

/// True when local podman reports the image.
pub fn runtime_image_ready(prefer_china: bool) -> bool {
    let Some(pod) = detect_podman() else {
        return false;
    };
    if !pod.ready {
        return false;
    }
    image_present(&pod.path, &resolve_runtime_image(prefer_china))
}
