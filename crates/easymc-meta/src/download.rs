use std::path::Path;

use futures_util::StreamExt;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use easymc_core::{EasyMcError, Result};

pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<()> {
    if dest.exists() {
        if let Some(sha) = expected_sha1 {
            if verify_sha1(dest, sha)? {
                if let Some(sz) = expected_size {
                    on_progress(sz, Some(sz));
                }
                return Ok(());
            }
        } else if expected_size.is_some_and(|s| dest.metadata().map(|m| m.len() == s).unwrap_or(false))
        {
            return Ok(());
        }
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| EasyMcError::Http(e.to_string()))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(e.to_string()))?;

    let total = expected_size.or_else(|| resp.content_length());
    let tmp = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();
    let mut hasher = Sha1::new();
    let mut written = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| EasyMcError::Http(e.to_string()))?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
        on_progress(written, total);
    }
    file.flush().await?;
    drop(file);

    if let Some(sha) = expected_sha1 {
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(sha) {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(EasyMcError::msg(format!(
                "sha1 mismatch for {}: expected {sha}, got {got}",
                dest.display()
            )));
        }
    }

    tokio::fs::rename(&tmp, dest).await?;
    Ok(())
}

pub fn verify_sha1(path: &Path, expected: &str) -> Result<bool> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha1::new();
    hasher.update(&data);
    let got = hex::encode(hasher.finalize());
    Ok(got.eq_ignore_ascii_case(expected))
}
