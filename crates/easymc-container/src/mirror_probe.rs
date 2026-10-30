//! Container registry mirror candidates + HTTPS latency probe.
//!
//! Player picks (or auto-suggests fastest) among Official + China-friendly
//! public prefixes. Probe is a timed HTTPS GET to each registry's `/v2/`
//! endpoint (TLS + HTTP), **not** bare TCP connect.
//!
//! Why not TCP `:443`? On machines using Clash/sing-box fake-ip (DNS →
//! `198.18.0.0/15`), TCP connect completes against the local TUN in ~0–5ms
//! and every mirror looks identical / "0 ms". HTTPS forces real upstream RTT
//! through the proxy (or direct path) so rankings are honest.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::repair::MirrorProfile;
use crate::registries::{DEFAULT_BASE_IMAGE, DEFAULT_BASE_IMAGE_MIRROR};

/// One registry mirror the player can select.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryMirrorCandidate {
    pub id: String,
    pub label_zh: String,
    pub region_zh: String,
    pub prefer_china: bool,
    pub probe_host: String,
    /// HTTPS URL actually hit during latency probe (registry `/v2/`).
    pub probe_url: String,
    pub sample_image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryMirrorProbe {
    pub id: String,
    pub label_zh: String,
    pub region_zh: String,
    pub prefer_china: bool,
    pub ok: bool,
    pub latency_ms: Option<u32>,
    pub error_zh: Option<String>,
    pub recommended: bool,
    pub sample_image: String,
}

/// Built-in registry mirror list (DaoCloud / community CN prefixes + official).
pub fn list_registry_mirror_candidates() -> Vec<RegistryMirrorCandidate> {
    MirrorProfile::ALL
        .into_iter()
        .map(|p| RegistryMirrorCandidate {
            id: p.as_str().into(),
            label_zh: p.label_zh().into(),
            region_zh: if p.prefer_china() {
                "国内".into()
            } else {
                "全球".into()
            },
            prefer_china: p.prefer_china(),
            probe_host: p.probe_host().into(),
            probe_url: p.probe_url(),
            sample_image: p.default_image().into(),
        })
        .collect()
}

/// Probe all registry mirrors in parallel via HTTPS. Marks fastest OK.
pub async fn probe_registry_mirrors() -> Vec<RegistryMirrorProbe> {
    let candidates = list_registry_mirror_candidates();
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(4))
        .pool_max_idle_per_host(0)
        .redirect(reqwest::redirect::Policy::limited(2))
        .user_agent("EasyMC/0.1 (registry-mirror-probe)")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return candidates
                .into_iter()
                .map(|c| RegistryMirrorProbe {
                    id: c.id,
                    label_zh: c.label_zh,
                    region_zh: c.region_zh,
                    prefer_china: c.prefer_china,
                    ok: false,
                    latency_ms: None,
                    error_zh: Some(format!("无法创建探测客户端：{e}")),
                    recommended: false,
                    sample_image: c.sample_image,
                })
                .collect();
        }
    };

    let mut handles = Vec::with_capacity(candidates.len());
    for c in candidates {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            let (ok, latency_ms, error_zh) = probe_https(&client, &c.probe_url).await;
            RegistryMirrorProbe {
                id: c.id,
                label_zh: c.label_zh,
                region_zh: c.region_zh,
                prefer_china: c.prefer_china,
                ok,
                latency_ms,
                error_zh,
                recommended: false,
                sample_image: c.sample_image,
            }
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        match h.await {
            Ok(r) => results.push(r),
            Err(e) => results.push(RegistryMirrorProbe {
                id: "unknown".into(),
                label_zh: "未知".into(),
                region_zh: "—".into(),
                prefer_china: false,
                ok: false,
                latency_ms: None,
                error_zh: Some(format!("探测任务失败：{e}")),
                recommended: false,
                sample_image: String::new(),
            }),
        }
    }

    mark_fastest(&mut results);
    results
}

/// Timed HTTPS GET. Registry `/v2/` often returns 401 — that still means reachable.
async fn probe_https(
    client: &reqwest::Client,
    url: &str,
) -> (bool, Option<u32>, Option<String>) {
    let start = Instant::now();
    // Prefer HEAD (cheap); some registries reject HEAD → fall back to GET.
    let head = client.head(url).send().await;
    let resp = match head {
        Ok(r) if r.status().as_u16() != 405 && r.status().as_u16() != 501 => Ok(r),
        _ => client.get(url).send().await,
    };
    match resp {
        Ok(resp) => {
            let status = resp.status();
            // Drain a small body so we include a bit of transfer, not only headers.
            let _ = resp.bytes().await;
            let ms = elapsed_ms(start);
            // 2xx / 3xx / 401 Unauthorized are all "mirror answered".
            if status.is_success() || status.is_redirection() || status.as_u16() == 401 {
                (true, Some(ms), None)
            } else {
                (
                    false,
                    Some(ms),
                    Some(format!("HTTP {}", status.as_u16())),
                )
            }
        }
        Err(e) => (false, None, Some(short_reqwest_err(&e))),
    }
}

fn elapsed_ms(start: Instant) -> u32 {
    let ms = start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
    // Sub-millisecond success still means "reached"; never show fake 0 on OK.
    ms.max(1)
}

fn short_reqwest_err(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "超时".into()
    } else if e.is_connect() {
        "连接失败".into()
    } else {
        let s = e.to_string();
        if s.chars().count() > 80 {
            format!("{}…", s.chars().take(80).collect::<String>())
        } else {
            s
        }
    }
}

fn mark_fastest(results: &mut [RegistryMirrorProbe]) {
    let best = results
        .iter()
        .filter(|r| r.ok)
        .filter_map(|r| r.latency_ms.map(|ms| (r.id.as_str(), ms)))
        .min_by_key(|(_, ms)| *ms)
        .map(|(id, _)| id.to_string());
    if let Some(id) = best {
        for r in results.iter_mut() {
            r.recommended = r.id == id;
        }
    }
}

/// Sanity: default sample images stay consistent with registries helpers.
#[allow(dead_code)]
pub fn sample_images() -> (&'static str, &'static str) {
    (DEFAULT_BASE_IMAGE, DEFAULT_BASE_IMAGE_MIRROR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_has_china_and_official() {
        let list = list_registry_mirror_candidates();
        assert!(list.iter().any(|c| c.id == "daocloud"));
        assert!(list.iter().any(|c| c.id == "official"));
        assert!(list.iter().any(|c| c.prefer_china));
        assert!(list.iter().all(|c| c.probe_url.starts_with("https://")));
        assert!(list.iter().all(|c| c.probe_url.contains("/v2/")));
    }

    #[test]
    fn mark_fastest_picks_min() {
        let mut v = vec![
            RegistryMirrorProbe {
                id: "daocloud".into(),
                label_zh: "D".into(),
                region_zh: "国内".into(),
                prefer_china: true,
                ok: true,
                latency_ms: Some(120),
                error_zh: None,
                recommended: false,
                sample_image: String::new(),
            },
            RegistryMirrorProbe {
                id: "official".into(),
                label_zh: "O".into(),
                region_zh: "全球".into(),
                prefer_china: false,
                ok: true,
                latency_ms: Some(40),
                error_zh: None,
                recommended: false,
                sample_image: String::new(),
            },
        ];
        mark_fastest(&mut v);
        assert!(v.iter().find(|r| r.id == "official").unwrap().recommended);
    }

    #[test]
    fn elapsed_ms_never_zero() {
        let start = Instant::now();
        assert!(elapsed_ms(start) >= 1);
    }
}
