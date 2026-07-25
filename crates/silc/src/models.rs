//! Silc-owned local LLM GGUF provisioning (`~/.silc/models/<id>/`).

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use sil_core::{validate_model_id, ModelCatalogEntry};

use crate::runtimes::cache_root;

/// Resolve `~/.silc/models/<id>/<filename>.gguf`, downloading once with sha256 verify.
pub fn ensure_model(model_id: &str) -> Result<PathBuf, String> {
    let entry = validate_model_id(model_id)?;
    let dest = model_path(entry)?;
    if dest.is_file() {
        verify_file_sha256(&dest, entry.sha256, entry.id)?;
        return Ok(dest);
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

fn download_streaming(
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    label: &str,
) -> Result<(), String> {
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(600))
        .call()
        .map_err(|error| format!("failed to provision model `{label}`: download {url}: {error}"))?;
    let mut reader = response.into_reader();
    let mut file = File::create(dest).map_err(|error| {
        format!(
            "failed to provision model `{label}`: create {}: {error}",
            dest.display()
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 256];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|error| format!("failed to provision model `{label}`: read body: {error}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|error| {
            format!(
                "failed to provision model `{label}`: write {}: {error}",
                dest.display()
            )
        })?;
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
        assert!(path.to_string_lossy().contains("models/llama3.2-1b"));
        assert!(path
            .to_string_lossy()
            .ends_with("Llama-3.2-1B-Instruct-Q4_K_M.gguf"));
    }
}
