//! Stable JSONL interchange for silclm training data.

use serde::{Deserialize, Serialize};

/// Seed task committed under `training/tasks/`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskSeed {
    pub id: String,
    pub category: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Model-ready prompt record emitted by `sil-training prompts`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptRecord {
    pub id: String,
    pub task_id: String,
    pub category: String,
    pub task: String,
    pub agents_md_version: String,
    pub prompt: String,
    pub prompt_sha256: String,
    pub target_model: String,
}

/// Candidate completion from any generator/provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateRecord {
    pub prompt_id: String,
    /// Full prompt text (optional if prompt_id is enough for join).
    #[serde(default)]
    pub prompt: String,
    /// Natural-language task (optional metadata).
    #[serde(default)]
    pub task: String,
    /// Raw model output (may include fences).
    pub completion: String,
    /// Optional generator model id (not necessarily silclm).
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcceptedRecord {
    pub id: String,
    pub prompt_id: String,
    pub task: String,
    pub prompt: String,
    pub program: String,
    pub program_sha256: String,
    pub compiler_version: String,
    pub execution_mode: String,
    pub validation_tier: u8,
    pub target_model: String,
    #[serde(default)]
    pub generator_model: Option<String>,
    #[serde(default)]
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RejectedRecord {
    pub id: String,
    pub prompt_id: String,
    pub task: String,
    pub prompt: String,
    pub completion: String,
    pub error: String,
    pub stage: String,
    pub compiler_version: String,
    #[serde(default)]
    pub generator_model: Option<String>,
}
