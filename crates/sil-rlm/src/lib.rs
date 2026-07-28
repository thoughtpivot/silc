//! Closed-tool recursive language model scaffold for `silc assist` (ADR-008).
//!
//! Keeps long Silc authoring knowledge (AGENTS, examples, fixtures) in an
//! external corpus. The root model issues structured tool calls; `silc_check`
//! validates drafts via `sil-training::check_source`. Depth-1 recursion uses
//! `llm_query`. This crate does not change the `.silc` language surface.

pub mod author;
pub mod complete;
pub mod corpus;
pub mod progress;
pub mod prompt;
pub mod session;
pub mod tools;

pub use complete::{ChatReply, ChatRequest, Completer, ScriptedCompleter};
pub use corpus::{find_agents_md, Corpus};
pub use progress::{
    draft_preview, truncate_one_line, ActionKind, ProgressEvent, ProgressReporter, NullProgress,
};
pub use session::{run_assist, AssistError, AssistResult, AssistSeed, HISTORY_CAP};
pub use tools::{BudgetStats, Budgets, MIN_DRAFT_CHARS};
