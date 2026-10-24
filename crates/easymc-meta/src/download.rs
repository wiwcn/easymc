use std::path::Path;

use futures_util::StreamExt;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use easymc_core::{DownloadSource, EasyMcError, Result};

use crate::source::{is_remapped, rewrite_url};

/// Outcome of a fetch: skipped (already on disk) or newly written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchResult {
    Skipped,
    Downloaded,
}

pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<FetchResult> {
    if dest.exists() {
        if let Some(sha) = expected_sha1 {
            if verify_sha1(dest, sha)? {
                if let Some(sz) = expected_size {
                    on_progress(sz, Some(sz));
                }
                return Ok(FetchResult::Skipped);
            }
            // Corrupt / outdated — remove and re-fetch.
            let _ = tokio::fs::remove_file(dest).await;
        } else if expected_size.is_some_and(|s| dest.metadata().map(|m| m.len() == s).unwrap_or(false))
        {
            return Ok(FetchResult::Skipped);
        }
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let tmp = dest.with_extension("part");
    // Drop any leftover partial from a prior failed run.
    let _ = tokio::fs::remove_file(&tmp).await;

    let result = download_to_part(
        client,
        url,
        dest,
        &tmp,
        expected_sha1,
        expected_size,
        &mut on_progress,
    )
    .await;

    if result.is_err() {
        let _ = tokio::fs::remove_file(&tmp).await;
    }
    result
}

async fn download_to_part(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    tmp: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    on_progress: &mut impl FnMut(u64, Option<u64>),
) -> Result<FetchResult> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("请求失败 {url}: {e}")))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("HTTP 错误 {url}: {e}")))?;

    let total = expected_size.or_else(|| resp.content_length());
    let mut file = tokio::fs::File::create(tmp)
        .await
        .map_err(|e| EasyMcError::msg(format!("无法写入临时文件 {}: {e}", tmp.display())))?;
    let mut stream = resp.bytes_stream();
    let mut hasher = Sha1::new();
    let mut written = 0u64;
    let mut last_emit = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| EasyMcError::Http(format!("下载中断 {url}: {e}")))?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
        // Throttle: every ~256 KiB or at completion.
        let step = 256 * 1024;
        if written == total.unwrap_or(u64::MAX)
            || written.saturating_sub(last_emit) >= step
            || last_emit == 0
        {
            on_progress(written, total);
            last_emit = written;
        }
    }
    file.flush().await?;
    drop(file);
    on_progress(written, total);

    if let Some(expected) = expected_size {
        if written != expected {
            return Err(EasyMcError::msg(format!(
                "大小不符 {}: 期望 {expected} 字节，实际 {written}（网络可能中断，已丢弃不完整文件）",
                dest.display()
            )));
        }
    }

    if let Some(sha) = expected_sha1 {
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(sha) {
            return Err(EasyMcError::msg(format!(
                "校验失败 {}: 期望 sha1={sha}，实际 {got}（已丢弃损坏文件）",
                dest.display()
            )));
        }
    }

    tokio::fs::rename(tmp, dest).await.map_err(|e| {
        EasyMcError::msg(format!(
            "无法落盘 {} ← {}: {e}",
            dest.display(),
            tmp.display()
        ))
    })?;
    Ok(FetchResult::Downloaded)
}

/// Download using the chosen source, with one Official fallback if the mirror fails.
pub async fn download_with_source(
    client: &reqwest::Client,
    official_url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    source: DownloadSource,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<FetchResult> {
    let primary = rewrite_url(source, official_url);
    match download_file(
        client,
        &primary,
        dest,
        expected_sha1,
        expected_size,
        &mut on_progress,
    )
    .await
    {
        Ok(r) => Ok(r),
        Err(first)
            if source != DownloadSource::Official && is_remapped(source, official_url) =>
        {
            tracing::warn!(
                "mirror download failed ({primary}): {first}; retrying official"
            );
            download_file(
                client,
                official_url,
                dest,
                expected_sha1,
                expected_size,
                &mut on_progress,
            )
            .await
            .map_err(|e| {
                EasyMcError::Http(format!(
                    "镜像与官方均失败：镜像={first}；官方={e}"
                ))
            })
        }
        Err(e) => Err(e),
    }
}

/// GET with mirror remap + one Official fallback (for JSON/meta endpoints).
pub async fn get_with_source(
    client: &reqwest::Client,
    official_url: &str,
    source: DownloadSource,
) -> Result<reqwest::Response> {
    let primary = rewrite_url(source, official_url);
    let try_one = |url: String| async move {
        client
            .get(&url)
            .send()
            .await
            .map_err(|e| EasyMcError::Http(format!("请求失败 {url}: {e}")))?
            .error_for_status()
            .map_err(|e| EasyMcError::Http(format!("HTTP 错误 {url}: {e}")))
    };
    match try_one(primary.clone()).await {
        Ok(r) => Ok(r),
        Err(first)
            if source != DownloadSource::Official && is_remapped(source, official_url) =>
        {
            tracing::warn!("mirror GET failed ({primary}): {first}; retrying official");
            try_one(official_url.to_string()).await.map_err(|e| {
                EasyMcError::Http(format!(
                    "镜像与官方均失败：镜像={first}；官方={e}"
                ))
            })
        }
        Err(e) => Err(e),
    }
}

pub fn verify_sha1(path: &Path, expected: &str) -> Result<bool> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha1::new();
    hasher.update(&data);
    let got = hex::encode(hasher.finalize());
    Ok(got.eq_ignore_ascii_case(expected))
}
