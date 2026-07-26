//! Provider-neutral silclm training harness.
//!
//! Builds prompts from AGENTS.md + task seeds, and banks compiler-validated
//! `.silc` candidates for a future fine-tuned silclm. Does not call any LLM.

pub mod bank;
pub mod check;
pub mod prompts;
pub mod schema;
pub mod subject_first;

pub use bank::{bank_candidates, BankStats};
pub use check::{check_source, CheckResult};
pub use prompts::{build_prompt_records, load_tasks, write_prompt_jsonl, PromptRecord, TaskSeed};
pub use schema::{AcceptedRecord, CandidateRecord, RejectedRecord};
pub use subject_first::{run_benchmark, BenchReport};
