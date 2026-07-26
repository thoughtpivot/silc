//! Silc-owned local LLM GGUF provisioning (`~/.silc/models/<id>/`).

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use sil_core::{validate_model_id, ModelCatalogEntry, LEGACY_MODEL_ID};

use crate::runtimes::cache_root;

/// Resolve `~/.silc/models/<id>/<filename>.gguf`, downloading once with sha256 verify.
pub fn ensure_model(model_id: &str) -> Result<PathBuf, String> {
    let entry = validate_model_id(model_id)?;
    let dest = model_path(entry)?;
    if dest.is_file() {
        verify_file_sha256(&dest, entry.sha256, entry.id)?;
        return Ok(dest);
    }
    // One-release migration: reuse a compatible artifact stored under the
    // legacy catalog directory. Older 1B weights do not match the 3B filename
    // or digest and therefore cannot be substituted for the current silclm.
    if let Ok(legacy) = legacy_model_path(entry) {
        if legacy.is_file() {
            verify_file_sha256(&legacy, entry.sha256, entry.id)?;
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "failed to provision model `{}`: create {}: {error}",
                        entry.id,
                        parent.display()
                    )
                })?;
            }
            fs::copy(&legacy, &dest).map_err(|error| {
                format!(
                    "failed to provision model `{}`: migrate {} -> {}: {error}",
                    entry.id,
                    legacy.display(),
                    dest.display()
                )
            })?;
            println!(
                "silc: migrated legacy model cache {} -> {}",
                legacy.display(),
                dest.display()
            );
            return Ok(dest);
        }
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to provision model `{}`: create {}: {error}",
                entry.id,
                parent.display()
            )
        })?;
    }
    let partial = dest.with_extension("gguf.partial");
    let _ = fs::remove_file(&partial);
    println!(
        "silc: downloading model `{}` (~{:.0} MB)…",
        entry.id,
        entry.approx_bytes as f64 / 1_000_000.0
    );
    download_streaming(entry.url, &partial, entry.sha256, entry.id)?;
    fs::rename(&partial, &dest).map_err(|error| {
        format!(
            "failed to provision model `{}`: rename {} -> {}: {error}",
            entry.id,
            partial.display(),
            dest.display()
        )
    })?;
    println!("silc: model ready at {}", dest.display());
    Ok(dest)
}

pub fn model_path(entry: &ModelCatalogEntry) -> Result<PathBuf, String> {
    Ok(cache_root()?
        .join("models")
        .join(entry.id)
        .join(entry.filename))
}

fn legacy_model_path(entry: &ModelCatalogEntry) -> Result<PathBuf, String> {
    Ok(cache_root()?
        .join("models")
        .join(LEGACY_MODEL_ID)
        .join(entry.filename))
}

fn download_streaming(
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    label: &str,
) -> Result<(), String> {
    // No overall request timeout: multi-GB artifacts on slow links can take
    // an hour. Only stalls (connect / per-read) abort the download.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout_read(std::time::Duration::from_secs(120))
        .build();
    let response = agent
        .get(url)
        .call()
        .map_err(|error| format!("failed to provision model `{label}`: download {url}: {error}"))?;
    let total_bytes = response
        .header("content-length")
        .and_then(|v| v.parse::<u64>().ok());
    let mut reader = response.into_reader();
    let mut file = File::create(dest).map_err(|error| {
        format!(
            "failed to provision model `{label}`: create {}: {error}",
            dest.display()
        )
    })?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    const PROGRESS_STEP: u64 = 100 * 1024 * 1024;
    let mut next_report = PROGRESS_STEP;
    let mut buf = [0u8; 1024 * 256];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|error| format!("failed to provision model `{label}`: read body: {error}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
        file.write_all(&buf[..n]).map_err(|error| {
            format!(
                "failed to provision model `{label}`: write {}: {error}",
                dest.display()
            )
        })?;
        if size >= next_report {
            match total_bytes {
                Some(total) if total > 0 => println!(
                    "silc: downloading `{label}`… {:.0} / {:.0} MB ({:.0}%)",
                    size as f64 / 1_000_000.0,
                    total as f64 / 1_000_000.0,
                    size as f64 * 100.0 / total as f64
                ),
                _ => println!(
                    "silc: downloading `{label}`… {:.0} MB",
                    size as f64 / 1_000_000.0
                ),
            }
            next_report += PROGRESS_STEP;
        }
    }
    file.flush().map_err(|error| {
        format!(
            "failed to provision model `{label}`: flush {}: {error}",
            dest.display()
        )
    })?;
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        let _ = fs::remove_file(dest);
        return Err(format!(
            "failed to provision model `{label}`: sha256 mismatch for {url}: expected {expected_sha256}, got {actual}"
        ));
    }
    Ok(())
}

fn verify_file_sha256(path: &Path, expected: &str, label: &str) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| {
        format!(
            "failed to verify model `{label}`: open {}: {error}",
            path.display()
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 256];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|error| format!("failed to verify model `{label}`: read: {error}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "failed to verify model `{label}`: sha256 mismatch for {}: expected {expected}, got {actual} (delete and re-run silc)",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sil_core::DEFAULT_MODEL_ID;

    #[test]
    fn default_model_resolves_under_cache() {
        let entry = validate_model_id(DEFAULT_MODEL_ID).unwrap();
        let path = model_path(entry).unwrap();
        assert!(path.to_string_lossy().contains("models/silclm"));
        assert!(path
            .to_string_lossy()
            .ends_with("Llama-3.2-3B-Instruct-Q4_K_M.gguf"));
    }
}
