//! Silc-owned local model provisioning (`~/.silc/models/<id>/`).

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use sil_core::{
    validate_embedding_model_id, validate_model_id, EmbeddingModelCatalogEntry, ModelArtifact,
    ModelCatalogEntry, LEGACY_MODEL_ID,
};

use crate::runtimes::cache_root;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingModelBundle {
    pub directory: PathBuf,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
}

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
    println!(
        "silc: downloading model `{}` (~{:.0} MB)…",
        entry.id,
        entry.approx_bytes as f64 / 1_000_000.0
    );
    provision_artifact(&dest, entry.url, entry.sha256, None, entry.id)?;
    println!("silc: model ready at {}", dest.display());
    Ok(dest)
}

/// Ensure the complete ONNX embedding bundle is available and verified.
pub fn ensure_embedding_model(model_id: &str) -> Result<EmbeddingModelBundle, String> {
    let entry = validate_embedding_model_id(model_id)?;
    let bundle = embedding_model_paths(entry)?;
    for artifact in entry.artifacts {
        let dest = bundle.directory.join(artifact.filename);
        if dest.is_file() {
            verify_artifact(&dest, artifact.sha256, Some(artifact.size_bytes), entry.id)?;
        } else {
            println!(
                "silc: downloading model `{}` artifact `{}` ({:.1} MB)…",
                entry.id,
                artifact.filename,
                artifact.size_bytes as f64 / 1_000_000.0
            );
            provision_embedding_artifact(&dest, artifact, entry.id)?;
        }
    }
    println!(
        "silc: embedding model ready at {}",
        bundle.directory.display()
    );
    Ok(bundle)
}

pub fn model_path(entry: &ModelCatalogEntry) -> Result<PathBuf, String> {
    Ok(cache_root()?
        .join("models")
        .join(entry.id)
        .join(entry.filename))
}

pub fn embedding_model_paths(
    entry: &EmbeddingModelCatalogEntry,
) -> Result<EmbeddingModelBundle, String> {
    Ok(embedding_model_paths_under(&cache_root()?, entry))
}

fn embedding_model_paths_under(
    root: &Path,
    entry: &EmbeddingModelCatalogEntry,
) -> EmbeddingModelBundle {
    let directory = root.join("models").join(entry.id);
    EmbeddingModelBundle {
        model_path: directory.join("model.onnx"),
        tokenizer_path: directory.join("tokenizer.json"),
        directory,
    }
}

fn legacy_model_path(entry: &ModelCatalogEntry) -> Result<PathBuf, String> {
    Ok(cache_root()?
        .join("models")
        .join(LEGACY_MODEL_ID)
        .join(entry.filename))
}

fn provision_embedding_artifact(
    dest: &Path,
    artifact: &ModelArtifact,
    label: &str,
) -> Result<(), String> {
    provision_artifact(
        dest,
        artifact.url,
        artifact.sha256,
        Some(artifact.size_bytes),
        label,
    )
}

fn provision_artifact(
    dest: &Path,
    url: &str,
    expected_sha256: &str,
    expected_size: Option<u64>,
    label: &str,
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to provision model `{label}`: create {}: {error}",
                parent.display()
            )
        })?;
    }
    let filename = dest
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid model artifact path {}", dest.display()))?;
    let partial = dest.with_file_name(format!("{filename}.partial"));
    let _ = fs::remove_file(&partial);
    let result =
        download_streaming(url, &partial, expected_sha256, expected_size, label).and_then(|()| {
            fs::rename(&partial, dest).map_err(|error| {
                format!(
                    "failed to provision model `{label}`: rename {} -> {}: {error}",
                    partial.display(),
                    dest.display()
                )
            })
        });
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

fn download_streaming(
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    expected_size: Option<u64>,
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
        .and_then(|v| v.parse::<u64>().ok())
        .or(expected_size);
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
    if let Some(expected) = expected_size {
        if size != expected {
            return Err(format!(
                "failed to provision model `{label}`: size mismatch for {url}: expected {expected} bytes, got {size}"
            ));
        }
    }
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
    verify_artifact(path, expected, None, label)
}

fn verify_artifact(
    path: &Path,
    expected_sha256: &str,
    expected_size: Option<u64>,
    label: &str,
) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| {
        format!(
            "failed to verify model `{label}`: open {}: {error}",
            path.display()
        )
    })?;
    if let Some(expected) = expected_size {
        let actual = file
            .metadata()
            .map_err(|error| {
                format!(
                    "failed to verify model `{label}`: stat {}: {error}",
                    path.display()
                )
            })?
            .len();
        if actual != expected {
            return Err(format!(
                "failed to verify model `{label}`: size mismatch for {}: expected {expected} bytes, got {actual} (delete and re-run silc)",
                path.display()
            ));
        }
    }
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
    if actual.eq_ignore_ascii_case(expected_sha256) {
        Ok(())
    } else {
        Err(format!(
            "failed to verify model `{label}`: sha256 mismatch for {}: expected {expected_sha256}, got {actual} (delete and re-run silc)",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sil_core::{DEFAULT_MODEL_ID, MINILM_MODEL_ID};

    #[test]
    fn default_model_resolves_under_cache() {
        let entry = validate_model_id(DEFAULT_MODEL_ID).unwrap();
        let path = model_path(entry).unwrap();
        assert!(path.to_string_lossy().contains("models/silclm"));
        assert!(path
            .to_string_lossy()
            .ends_with("Llama-3.2-3B-Instruct-Q4_K_M.gguf"));
    }

    #[test]
    fn embedding_bundle_paths_are_stable() {
        let entry = validate_embedding_model_id(MINILM_MODEL_ID).unwrap();
        let bundle = embedding_model_paths_under(Path::new("/tmp/silc-test-cache"), entry);
        assert_eq!(
            bundle.directory,
            Path::new("/tmp/silc-test-cache/models/minilm-l6-v2")
        );
        assert_eq!(bundle.model_path, bundle.directory.join("model.onnx"));
        assert_eq!(
            bundle.tokenizer_path,
            bundle.directory.join("tokenizer.json")
        );
    }

    #[test]
    fn cached_artifact_checksum_mismatch_is_rejected() {
        let path =
            std::env::temp_dir().join(format!("silc-model-checksum-{}", uuid::Uuid::new_v4()));
        fs::write(&path, b"not a model").unwrap();
        let error = verify_artifact(&path, &"0".repeat(64), Some(11), MINILM_MODEL_ID).unwrap_err();
        let _ = fs::remove_file(&path);
        assert!(error.contains("sha256 mismatch"));
        assert!(error.contains("delete and re-run silc"));
    }
}
