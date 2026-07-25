//! Compiler-owned local LLM model catalog for `llm::complete`.

/// Default catalog id when `:model` / `model_ref` is omitted.
pub const DEFAULT_MODEL_ID: &str = "llama3.2-1b";

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

/// v1 catalog — only Llama 3.2 1B Instruct Q4_K_M ships in this slice.
pub const MODEL_CATALOG: &[ModelCatalogEntry] = &[ModelCatalogEntry {
    id: "llama3.2-1b",
    filename: "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
    url: "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
    // bartowski Llama-3.2-1B-Instruct-Q4_K_M.gguf (807694464 bytes)
    sha256: "6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83",
    approx_bytes: 807_694_464,
}];

pub fn lookup_model(id: &str) -> Option<&'static ModelCatalogEntry> {
    let trimmed = id.trim().trim_matches('"').trim_matches('\'');
    MODEL_CATALOG.iter().find(|e| e.id == trimmed)
}

pub fn validate_model_id(id: &str) -> Result<&'static ModelCatalogEntry, String> {
    lookup_model(id).ok_or_else(|| {
        let known: Vec<&str> = MODEL_CATALOG.iter().map(|e| e.id).collect();
        format!(
            "unknown model `{}` (catalog: {})",
            id.trim().trim_matches('"').trim_matches('\''),
            known.join(", ")
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
    }

    #[test]
    fn rejects_unknown_model() {
        assert!(validate_model_id("not-a-real-model").is_err());
    }
}
