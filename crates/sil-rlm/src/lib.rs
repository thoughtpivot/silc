//! Closed-tool recursive language model scaffold for `silc assist` (ADR-008).
//!
//! Keeps long Silc authoring knowledge (AGENTS, examples, fixtures) in an
//! external corpus. The root model issues structured tool calls; `silc_check`
//! validates drafts via `sil-training::check_source`. Depth-1 recursion uses
//! `llm_query`. This crate does not change the `.silc` language surface.

pub mod complete;
pub mod corpus;
pub mod prompt;
pub mod session;
pub mod tools;

pub use complete::{Completer, ScriptedCompleter};
pub use corpus::Corpus;
pub use session::{run_assist, AssistError, AssistResult};
pub use tools::{Budgets, BudgetStats};
