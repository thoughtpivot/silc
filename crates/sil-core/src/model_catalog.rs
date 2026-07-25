//! Compiler-owned local LLM model catalog for `llm::complete`.
//!
//! **silclm** is Silc's owned local model identity. v0 ships pinned Llama 3.2
//! 1B Instruct Q4_K_M weights under that catalog id; future fine-tunes keep
//! the same authoring surface.

/// Default catalog id when `:model` / `model_ref` is omitted.
pub const DEFAULT_MODEL_ID: &str = "silclm";

/// Legacy catalog id accepted for one release (resolves to `silclm`).
pub const LEGACY_MODEL_ID: &str = "llama3.2-1b";

/// Default llama.cpp context window for generated silclm workers.
pub const DEFAULT_LLM_N_CTX: u32 = 8192;

/// One Silc-owned GGUF artifact authors may name via `:model(...)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelCatalogEntry {
    pub id: &'static str,
    pub filename: &'static str,
    /// Hugging Face resolve URL for the pinned GGUF.
    pub url: &'static str,
    pub sha256: &'static str,
    /// Approximate download size for docs/UX (bytes).
    pub approx_bytes: u64,
}

/// v1 catalog — silclm (Llama-based GGUF) ships in this slice.
pub const MODEL_CATALOG: &[ModelCatalogEntry] = &[ModelCatalogEntry {
    id: "silclm",
    filename: "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
    url: "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
    // bartowski Llama-3.2-1B-Instruct-Q4_K_M.gguf (807694464 bytes)
    sha256: "6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83",
    approx_bytes: 807_694_464,
}];

fn normalize_model_id(id: &str) -> &str {
    let trimmed = id.trim().trim_matches('"').trim_matches('\'');
    if trimmed == LEGACY_MODEL_ID {
        DEFAULT_MODEL_ID
    } else {
        trimmed
    }
}

pub fn lookup_model(id: &str) -> Option<&'static ModelCatalogEntry> {
    let normalized = normalize_model_id(id);
    MODEL_CATALOG.iter().find(|e| e.id == normalized)
}

pub fn validate_model_id(id: &str) -> Result<&'static ModelCatalogEntry, String> {
    lookup_model(id).ok_or_else(|| {
        let known: Vec<&str> = MODEL_CATALOG.iter().map(|e| e.id).collect();
        format!(
            "unknown model `{}` (catalog: {}; legacy alias `{}` also accepted)",
            id.trim().trim_matches('"').trim_matches('\''),
            known.join(", "),
            LEGACY_MODEL_ID
        )
    })
}

pub fn is_known_model_id(id: &str) -> bool {
    lookup_model(id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_is_in_catalog() {
        assert!(lookup_model(DEFAULT_MODEL_ID).is_some());
        assert_eq!(DEFAULT_MODEL_ID, "silclm");
    }

    #[test]
    fn legacy_alias_resolves_to_silclm() {
        let entry = validate_model_id(LEGACY_MODEL_ID).unwrap();
        assert_eq!(entry.id, "silclm");
    }

    #[test]
    fn rejects_unknown_model() {
        assert!(validate_model_id("not-a-real-model").is_err());
    }
}
