//! Subject-first declarator benchmark harness (evaluate only — no syntax migration).
//!
//! Compares prompt variants that instruct agents to use today's `class X is resource`
//! surface versus hypothetical subject-first declarators (`resource X`, …).

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::check::{check_source, extract_program};
use crate::prompts::{format_prompt, load_tasks};
use crate::schema::TaskSeed;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyntaxVariant {
    /// Current Silc 0.2.0: `class X is resource|component|app|…`.
    ClassIs,
    /// Hypothetical: `resource X` / `component X` / `app X` / `contract X` / ….
    SubjectFirst,
}

impl SyntaxVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClassIs => "class-is",
            Self::SubjectFirst => "subject-first",
        }
    }

    pub fn syntax_guidance(self) -> &'static str {
        match self {
            Self::ClassIs => {
                r#"Use current Silc 0.2.0 declarators only:
- `class Name { … }` for contracts
- `class Name is component { … }`
- `class Name is resource { … }`
- `class Name is app { … }`
- `class Name is service|processor|sink { … }` for modules
Do not invent subject-first keywords."#
            }
            Self::SubjectFirst => {
                r#"Use subject-first declarators (experimental prompt variant — not yet in the compiler):
- `contract Name { … }` instead of bare `class Name`
- `component Name { … }` instead of `class Name is component`
- `resource Name { … }` instead of `class Name is resource`
- `app Name { … }` instead of `class Name is app`
- `service Name` / `processor Name` / `sink Name` instead of `class Name is service|processor|sink`
Keep `has`, `method`, `query`, `mutation`, `route`, `==>`, and `ui::*` as today."#
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantPrompt {
    pub id: String,
    pub task_id: String,
    pub variant: SyntaxVariant,
    pub prompt: String,
    pub prompt_tokens_est: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchTrial {
    pub task_id: String,
    pub variant: SyntaxVariant,
    pub prompt_tokens_est: usize,
    pub completion_tokens_est: usize,
    pub first_pass_ok: bool,
    pub repair_turns: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchSummary {
    pub variant: SyntaxVariant,
    pub trials: usize,
    pub first_pass_ok: usize,
    pub first_pass_rate: f64,
    pub mean_repair_turns: f64,
    pub mean_prompt_tokens_est: f64,
    pub mean_completion_tokens_est: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    pub go_no_go_criteria: Vec<String>,
    pub notes: Vec<String>,
    pub prompts: Vec<VariantPrompt>,
    pub trials: Vec<BenchTrial>,
    pub summaries: Vec<BenchSummary>,
}

/// Rough token estimate: whitespace-separated words (good enough for relative compare).
pub fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace().count().max(1)
}

pub fn build_variant_prompts(agents_md: &str, tasks: &[TaskSeed]) -> Vec<VariantPrompt> {
    let mut out = Vec::new();
    for task in tasks {
        for variant in [SyntaxVariant::ClassIs, SyntaxVariant::SubjectFirst] {
            let task_body = format!("{}\n\n# Syntax variant\n\n{}", task.description, variant.syntax_guidance());
            let prompt = format_prompt(agents_md, &task_body);
            out.push(VariantPrompt {
                id: format!("prompt-{}-{}", task.id, variant.as_str()),
                task_id: task.id.clone(),
                variant,
                prompt_tokens_est: estimate_tokens(&prompt),
                prompt,
            });
        }
    }
    out
}

#[derive(Debug, Deserialize)]
pub struct TrialInput {
    pub task_id: String,
    pub variant: SyntaxVariant,
    pub completion: String,
    /// Number of repair turns after the first attempt (0 = first-pass green).
    #[serde(default)]
    pub repair_turns: u32,
}

pub fn score_trials(inputs: &[TrialInput]) -> Vec<BenchTrial> {
    inputs
        .iter()
        .map(|input| {
            let program = extract_program(&input.completion);
            let check = check_source(&program, None);
            let (first_pass_ok, error) = match check {
                Ok(_) => (input.repair_turns == 0, None),
                Err(e) => (false, Some(e)),
            };
            // If repairs were needed but final completion is green, first_pass_ok stays false.
            let final_ok = check_source(&program, None).is_ok();
            let first_pass_ok = first_pass_ok && final_ok;
            BenchTrial {
                task_id: input.task_id.clone(),
                variant: input.variant,
                prompt_tokens_est: 0,
                completion_tokens_est: estimate_tokens(&input.completion),
                first_pass_ok,
                repair_turns: input.repair_turns,
                error,
            }
        })
        .collect()
}

pub fn summarize(trials: &[BenchTrial]) -> Vec<BenchSummary> {
    let mut out = Vec::new();
    for variant in [SyntaxVariant::ClassIs, SyntaxVariant::SubjectFirst] {
        let subset: Vec<_> = trials.iter().filter(|t| t.variant == variant).collect();
        if subset.is_empty() {
            continue;
        }
        let n = subset.len() as f64;
        let first_pass_ok = subset.iter().filter(|t| t.first_pass_ok).count();
        let mean_repair = subset.iter().map(|t| t.repair_turns as f64).sum::<f64>() / n;
        let mean_prompt = subset.iter().map(|t| t.prompt_tokens_est as f64).sum::<f64>() / n;
        let mean_completion = subset
            .iter()
            .map(|t| t.completion_tokens_est as f64)
            .sum::<f64>()
            / n;
        out.push(BenchSummary {
            variant,
            trials: subset.len(),
            first_pass_ok,
            first_pass_rate: first_pass_ok as f64 / n,
            mean_repair_turns: mean_repair,
            mean_prompt_tokens_est: mean_prompt,
            mean_completion_tokens_est: mean_completion,
        });
    }
    out
}

pub fn go_no_go_criteria() -> Vec<String> {
    vec![
        "Migrate only if subject-first first-pass `silc build` success rate beats class-is by ≥10 absolute percentage points on the same task set.".into(),
        "Mean repair turns to green must not increase.".into(),
        "Mean completion tokens must not increase by >15% unless first-pass gain exceeds 15 points.".into(),
        "Decision deferred until both variants have ≥20 scored trials per task family.".into(),
    ]
}

/// Record baseline: current syntax compiles for fixture programs; subject-first does not parse yet.
pub fn baseline_fixture_trials() -> Vec<BenchTrial> {
    const FIXTURES: &[(&str, &str)] = &[
        (
            "shopping_crud",
            include_str!("../../silc/tests/fixtures/shopping_app.silc"),
        ),
        (
            "scored_form",
            include_str!("../../silc/tests/fixtures/scored_form.silc"),
        ),
        (
            "data_pipeline",
            include_str!("../../silc/tests/fixtures/data_pipeline.silc"),
        ),
    ];
    let mut trials = Vec::new();
    for (task_id, source) in FIXTURES {
        let ok = check_source(source, None).is_ok();
        trials.push(BenchTrial {
            task_id: (*task_id).into(),
            variant: SyntaxVariant::ClassIs,
            prompt_tokens_est: 0,
            completion_tokens_est: estimate_tokens(source),
            first_pass_ok: ok,
            repair_turns: 0,
            error: if ok {
                None
            } else {
                Some("baseline fixture failed check".into())
            },
        });
        // Subject-first reference rewrite is not executable yet — record as known-fail placeholder.
        let rewritten = rewrite_to_subject_first_sketch(source);
        trials.push(BenchTrial {
            task_id: (*task_id).into(),
            variant: SyntaxVariant::SubjectFirst,
            prompt_tokens_est: 0,
            completion_tokens_est: estimate_tokens(&rewritten),
            first_pass_ok: false,
            repair_turns: 0,
            error: Some(
                "subject-first syntax is not implemented; baseline records compile failure by design"
                    .into(),
            ),
        });
    }
    trials
}

/// Best-effort sketch rewrite for token comparison only (not a real migration).
fn rewrite_to_subject_first_sketch(source: &str) -> String {
    source
        .replace("class ", "/*class→subject*/ ")
        .replace(" is component", "")
        .replace(" is resource", "")
        .replace(" is app", "")
        .replace(" is service", "")
        .replace(" is processor", "")
        .replace(" is sink", "")
}

pub fn run_benchmark(agents_path: &Path, tasks_dir: &Path, out_path: &Path) -> Result<BenchReport, String> {
    let agents_md = fs::read_to_string(agents_path)
        .map_err(|e| format!("read {}: {e}", agents_path.display()))?;
    let tasks = load_tasks(tasks_dir)?;
    let prompts = build_variant_prompts(&agents_md, &tasks);
    let mut trials = baseline_fixture_trials();
    // Attach prompt token estimates where task ids match.
    for trial in &mut trials {
        if let Some(p) = prompts
            .iter()
            .find(|p| p.task_id == trial.task_id && p.variant == trial.variant)
        {
            trial.prompt_tokens_est = p.prompt_tokens_est;
        }
    }
    let summaries = summarize(&trials);
    let report = BenchReport {
        go_no_go_criteria: go_no_go_criteria(),
        notes: vec![
            "Baseline recorded for class-is fixtures that compile today.".into(),
            "Subject-first trials are placeholders until an agent run fills TrialInput JSONL.".into(),
            "No syntax migration in this workstream.".into(),
        ],
        prompts,
        trials,
        summaries,
    };
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    let mut file =
        fs::File::create(out_path).map_err(|e| format!("create {}: {e}", out_path.display()))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("write {}: {e}", out_path.display()))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_paired_prompts() {
        let tasks = vec![TaskSeed {
            id: "t1".into(),
            category: "ui".into(),
            description: "Build a tiny dual-surface app.".into(),
            tags: vec![],
        }];
        let prompts = build_variant_prompts("# agents", &tasks);
        assert_eq!(prompts.len(), 2);
        assert!(prompts[0].prompt.contains("class Name is component"));
        assert!(prompts[1].prompt.contains("component Name"));
    }

    #[test]
    fn baseline_class_is_fixtures_compile() {
        let trials = baseline_fixture_trials();
        let class_ok = trials
            .iter()
            .filter(|t| t.variant == SyntaxVariant::ClassIs)
            .all(|t| t.first_pass_ok);
        assert!(class_ok, "class-is fixtures must compile");
        assert!(trials
            .iter()
            .filter(|t| t.variant == SyntaxVariant::SubjectFirst)
            .all(|t| !t.first_pass_ok));
    }
}
