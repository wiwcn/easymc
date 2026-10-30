//! Download mirror candidates + latency probe for player pick / auto-fastest.
//!
//! Candidates: Official (global) + BMCLAPI family (China-friendly). Probe uses a
//! short HTTPS GET against a public meta URL — not ICMP and not bare TCP — so
//! results reflect real TLS+HTTP latency. (Bare TCP `:443` is useless under
//! Clash/sing-box fake-ip: connect hits the local TUN in ~0ms.)

use std::time::{Duration, Instant};

use easymc_core::DownloadSource;
use serde::{Deserialize, Serialize};

use crate::source::BMCLAPI_BASE;
use crate::VERSION_MANIFEST_URL;

/// One selectable download / meta mirror.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadMirrorCandidate {
    pub id: String,
    pub label_zh: String,
    pub region_zh: String,
    /// Maps to [`DownloadSource`] (`official` / `bmclapi`).
    pub download_source: String,
    pub probe_url: String,
    pub host_hint: String,
}

/// Probe outcome for one candidate (sorted by caller; `recommended` marks fastest OK).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadMirrorProbe {
    pub id: String,
    pub label_zh: String,
    pub region_zh: String,
    pub download_source: String,
    pub ok: bool,
    pub latency_ms: Option<u32>,
    pub error_zh: Option<String>,
    pub recommended: bool,
}

/// Built-in download mirror list (China-friendly + official).
pub fn list_download_mirror_candidates() -> Vec<DownloadMirrorCandidate> {
    vec![
        DownloadMirrorCandidate {
            id: "bmclapi".into(),
            label_zh: "BMCLAPI 加速".into(),
            region_zh: "国内".into(),
            download_source: DownloadSource::Bmclapi.as_str().into(),
            probe_url: format!("{BMCLAPI_BASE}/mc/game/version_manifest_v2.json"),
            host_hint: "bmclapi2.bangbang93.com".into(),
        },
        DownloadMirrorCandidate {
            id: "bmclapi_alt".into(),
            label_zh: "BMCLAPI 备用".into(),
            region_zh: "国内".into(),
            // Same rewrite family — canonical downloads still use BMCLAPI_BASE.
            download_source: DownloadSource::Bmclapi.as_str().into(),
            probe_url: "https://bmclapi.bangbang93.com/mc/game/version_manifest_v2.json".into(),
            host_hint: "bmclapi.bangbang93.com".into(),
        },
        DownloadMirrorCandidate {
            id: "official".into(),
            label_zh: "官方源".into(),
            region_zh: "全球".into(),
            download_source: DownloadSource::Official.as_str().into(),
            probe_url: VERSION_MANIFEST_URL.into(),
            host_hint: "piston-meta.mojang.com".into(),
        },
    ]
}

/// Probe all download mirrors (parallel). Marks the fastest successful as recommended.
pub async fn probe_download_mirrors() -> Vec<DownloadMirrorProbe> {
    let candidates = list_download_mirror_candidates();
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(4))
        .pool_max_idle_per_host(0)
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent("EasyMC/0.1 (mirror-probe)")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return candidates
                .into_iter()
                .map(|c| DownloadMirrorProbe {
                    id: c.id,
                    label_zh: c.label_zh,
                    region_zh: c.region_zh,
                    download_source: c.download_source,
                    ok: false,
                    latency_ms: None,
                    error_zh: Some(format!("无法创建探测客户端：{e}")),
                    recommended: false,
                })
                .collect();
        }
    };

    let mut handles = Vec::with_capacity(candidates.len());
    for c in candidates {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            let (ok, latency_ms, error_zh) = probe_one(&client, &c.probe_url).await;
            DownloadMirrorProbe {
                id: c.id,
                label_zh: c.label_zh,
                region_zh: c.region_zh,
                download_source: c.download_source,
                ok,
                latency_ms,
                error_zh,
                recommended: false,
            }
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        match h.await {
            Ok(r) => results.push(r),
            Err(e) => results.push(DownloadMirrorProbe {
                id: "unknown".into(),
                label_zh: "未知".into(),
                region_zh: "—".into(),
                download_source: "official".into(),
                ok: false,
                latency_ms: None,
                error_zh: Some(format!("探测任务失败：{e}")),
                recommended: false,
            }),
        }
    }

    mark_fastest_recommended(&mut results);
    results
}

async fn probe_one(
    client: &reqwest::Client,
    url: &str,
) -> (bool, Option<u32>, Option<String>) {
    let start = Instant::now();
    match client.get(url).send().await {
        Ok(resp) => {
            let status = resp.status();
            // Include body read in timing so CDN TTFB alone can't look "instant".
            let _ = resp.bytes().await;
            let ms = elapsed_ms(start);
            if status.is_success() || status.is_redirection() {
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

fn mark_fastest_recommended(results: &mut [DownloadMirrorProbe]) {
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

/// Map a candidate id → [`DownloadSource`] (unknown → Official).
pub fn download_source_for_mirror_id(id: &str) -> DownloadSource {
    match id.trim().to_ascii_lowercase().as_str() {
        "bmclapi" | "bmcl" | "bmclapi2" | "bmclapi_alt" | "bmclapi-alt" => DownloadSource::Bmclapi,
        _ => DownloadSource::Official,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_include_china_and_official() {
        let list = list_download_mirror_candidates();
        assert!(list.iter().any(|c| c.id == "bmclapi"));
        assert!(list.iter().any(|c| c.id == "official"));
        assert!(list.iter().any(|c| c.download_source == "bmclapi"));
    }

    #[test]
    fn mirror_id_maps() {
        assert_eq!(
            download_source_for_mirror_id("bmclapi_alt"),
            DownloadSource::Bmclapi
        );
        assert_eq!(
            download_source_for_mirror_id("official"),
            DownloadSource::Official
        );
    }

    #[test]
    fn mark_fastest() {
        let mut v = vec![
            DownloadMirrorProbe {
                id: "a".into(),
                label_zh: "A".into(),
                region_zh: "国内".into(),
                download_source: "bmclapi".into(),
                ok: true,
                latency_ms: Some(200),
                error_zh: None,
                recommended: false,
            },
            DownloadMirrorProbe {
                id: "b".into(),
                label_zh: "B".into(),
                region_zh: "全球".into(),
                download_source: "official".into(),
                ok: true,
                latency_ms: Some(80),
                error_zh: None,
                recommended: false,
            },
            DownloadMirrorProbe {
                id: "c".into(),
                label_zh: "C".into(),
                region_zh: "国内".into(),
                download_source: "bmclapi".into(),
                ok: false,
                latency_ms: None,
                error_zh: Some("超时".into()),
                recommended: false,
            },
        ];
        mark_fastest_recommended(&mut v);
        assert!(v.iter().find(|r| r.id == "b").unwrap().recommended);
        assert!(!v.iter().find(|r| r.id == "a").unwrap().recommended);
    }
}
