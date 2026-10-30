//! Container registry mirror candidates + TCP/HTTPS latency probe.
//!
//! Player picks (or auto-suggests fastest) among Official + China-friendly
//! public prefixes. Probe is a timed TCP connect to `:443` (no image pull).

use std::io;
use std::net::{TcpStream, ToSocketAddrs};
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
            sample_image: p.default_image().into(),
        })
        .collect()
}

/// Probe all registry mirrors (sequential; TCP connect is cheap). Marks fastest OK.
pub fn probe_registry_mirrors() -> Vec<RegistryMirrorProbe> {
    let timeout = Duration::from_secs(3);
    let mut results: Vec<RegistryMirrorProbe> = list_registry_mirror_candidates()
        .into_iter()
        .map(|c| {
            let (ok, latency_ms, error_zh) = probe_tcp_host(&c.probe_host, 443, timeout);
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
        })
        .collect();
    mark_fastest(&mut results);
    results
}

fn probe_tcp_host(host: &str, port: u16, timeout: Duration) -> (bool, Option<u32>, Option<String>) {
    let start = Instant::now();
    let addr = format!("{host}:{port}");
    let addrs = match addr.to_socket_addrs() {
        Ok(a) => a.collect::<Vec<_>>(),
        Err(e) => return (false, None, Some(format!("DNS 失败：{e}"))),
    };
    if addrs.is_empty() {
        return (false, None, Some("DNS 无结果".into()));
    }
    let mut last_err = None;
    for a in addrs {
        match TcpStream::connect_timeout(&a, timeout) {
            Ok(_stream) => {
                let ms = start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
                return (true, Some(ms), None);
            }
            Err(e) => last_err = Some(e),
        }
    }
    (
        false,
        None,
        Some(format!("连接失败：{}", io_err_zh(last_err.as_ref()))),
    )
}

fn io_err_zh(e: Option<&io::Error>) -> String {
    match e {
        Some(err) if err.kind() == io::ErrorKind::TimedOut => "超时".into(),
        Some(err) => err.to_string(),
        None => "未知错误".into(),
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
}
