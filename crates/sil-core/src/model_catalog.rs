//! Compiler-owned local model catalogs for `llm::complete` and `tensor::infer`.
//!
//! **silclm** is Silc's owned local LLM identity. v0 ships pinned Llama 3.2
//! 3B Instruct Q4_K_M weights under that catalog id; future fine-tunes keep
//! the same authoring surface.
//!
//! **minilm-l6-v2** is the closed Silc 0.4.0 embedding model for
//! `tensor::infer` (384-d, CPU / ONNX). Its model and tokenizer artifacts are
//! pinned to one upstream sentence-transformers commit.

/// Default catalog id when `:model` / `model_ref` is omitted for LLM.
pub const DEFAULT_MODEL_ID: &str = "silclm";

/// Legacy catalog id accepted for one release (resolves to `silclm`).
pub const LEGACY_MODEL_ID: &str = "llama3.2-1b";

/// Default llama.cpp context window for generated silclm workers.
pub const DEFAULT_LLM_N_CTX: u32 = 8192;

/// Closed Silc 0.4.0 embedding model id for `tensor::infer`.
pub const MINILM_MODEL_ID: &str = "minilm-l6-v2";

/// Default embedding catalog id when `:model` is omitted on `tensor::infer`.
pub const DEFAULT_EMBEDDING_MODEL_ID: &str = MINILM_MODEL_ID;

/// Closed embedding output dimension for minilm-l6-v2.
pub const MINILM_EMBEDDING_DIM: u32 = 384;

/// Default contract field fed into `tensor::tokenize` / `tensor::infer`.
pub const DEFAULT_TENSOR_INPUT_FIELD: &str = "raw_content";

/// Default contract field written by `tensor::infer`.
pub const DEFAULT_TENSOR_OUTPUT_FIELD: &str = "vector_embedding";

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
    filename: "Llama-3.2-3B-Instruct-Q4_K_M.gguf",
    url: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
    // bartowski Llama-3.2-3B-Instruct-Q4_K_M.gguf (2019377696 bytes)
    sha256: "6c1a2b41161032677be168d354123594c0e6e67d2b9227c84f296ad037c728ff",
    approx_bytes: 2_019_377_696,
}];

/// One immutable artifact in a compiler-owned model bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelArtifact {
    pub filename: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size_bytes: u64,
}

/// Compiler-owned embedding model bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddingModelCatalogEntry {
    pub id: &'static str,
    pub dimension: u32,
    pub cpu_only: bool,
    pub upstream_license: &'static str,
    pub artifacts: &'static [ModelArtifact],
}

pub const MINILM_ARTIFACTS: &[ModelArtifact] = &[
    ModelArtifact {
        filename: "model.onnx",
        url: "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/bc57282bc374d33e0d6c4de27f12dc1c2a87f37a/onnx/model.onnx",
        sha256: "6fd5d72fe4589f189f8ebc006442dbb529bb7ce38f8082112682524616046452",
        size_bytes: 90_405_214,
    },
    ModelArtifact {
        filename: "tokenizer.json",
        url: "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/bc57282bc374d33e0d6c4de27f12dc1c2a87f37a/tokenizer.json",
        sha256: "be50c3628f2bf5bb5e3a7f17b1f74611b2561a3a27eeab05e5aa30f411572037",
        size_bytes: 466_247,
    },
];

/// Closed Silc 0.4.0 embedding catalog.
pub const EMBEDDING_MODEL_CATALOG: &[EmbeddingModelCatalogEntry] = &[EmbeddingModelCatalogEntry {
    id: MINILM_MODEL_ID,
    dimension: MINILM_EMBEDDING_DIM,
    cpu_only: true,
    upstream_license: "Apache-2.0",
    artifacts: MINILM_ARTIFACTS,
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

fn normalize_embedding_model_id(id: &str) -> &str {
    id.trim().trim_matches('"').trim_matches('\'')
}

pub fn lookup_embedding_model(id: &str) -> Option<&'static EmbeddingModelCatalogEntry> {
    let normalized = normalize_embedding_model_id(id);
    EMBEDDING_MODEL_CATALOG.iter().find(|e| e.id == normalized)
}

pub fn validate_embedding_model_id(
    id: &str,
) -> Result<&'static EmbeddingModelCatalogEntry, String> {
    lookup_embedding_model(id).ok_or_else(|| {
        let known: Vec<&str> = EMBEDDING_MODEL_CATALOG.iter().map(|e| e.id).collect();
        format!(
            "unknown embedding model `{}` (catalog: {})",
            normalize_embedding_model_id(id),
            known.join(", ")
        )
    })
}

pub fn is_known_embedding_model_id(id: &str) -> bool {
    lookup_embedding_model(id).is_some()
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

    #[test]
    fn minilm_is_closed_embedding_catalog() {
        let entry = validate_embedding_model_id(MINILM_MODEL_ID).unwrap();
        assert_eq!(entry.id, "minilm-l6-v2");
        assert_eq!(entry.dimension, 384);
        assert!(entry.cpu_only);
        assert_eq!(entry.upstream_license, "Apache-2.0");
        assert_eq!(entry.artifacts, MINILM_ARTIFACTS);
        assert_eq!(entry.artifacts.len(), 2);
        assert_eq!(entry.artifacts[0].filename, "model.onnx");
        assert_eq!(entry.artifacts[0].size_bytes, 90_405_214);
        assert_eq!(entry.artifacts[1].filename, "tokenizer.json");
        assert_eq!(entry.artifacts[1].size_bytes, 466_247);
        assert!(validate_embedding_model_id("not-an-embedding").is_err());
    }
}
