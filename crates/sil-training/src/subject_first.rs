//! Historical subject-first declarator benchmark harness.
//!
//! The report schema and paired variants remain available for reproducing the
//! migration evidence. Product validation uses the Silc 0.4.0 parser directly.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::check::{check_source, extract_program};
use crate::prompts::{format_prompt, load_tasks};
use crate::schema::TaskSeed;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyntaxVariant {
    /// Historical pre-0.3.0 `class X is resource|component|app|…` surface.
    ClassIs,
    /// Current Silc 0.4.0 direct-declaration / intent surface
    /// (historical harness label: “subject-first”).
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
                r#"Use the historical pre-0.3.0 declarators for this benchmark variant:
- `class Name { … }` for contracts
- `class Name is component { … }`
- `class Name is resource { … }`
- `class Name is app { … }`
- `class Name is service|processor|sink { … }` for modules
This syntax is intentionally rejected by the current compiler."#
            }
            Self::SubjectFirst => {
                r#"Use current Silc 0.4.0 direct declarations (intent surface):
- `contract Name { … }` instead of bare `class Name`
- `component Name { … }` instead of `class Name is component`
- `resource Name for Contract { query list; mutation create; … }` (capability-only)
- `app Name { route … }` only — no `method serve()`, `ui::web`, or `ui::terminal`
- `service Name` / `processor Name` / `task Name` for workflows
Do **not** author `sink`, `ipc::*`, `store::*`, or `resource::*` pipelines.
Keep `has`, `method` (workflow), `route`, `==>`, and UI primitives (`ui::chat`, …)."#
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
    pub category: String,
    pub variant: SyntaxVariant,
    pub prompt_tokens_est: usize,
    pub completion_tokens_est: usize,
    pub first_pass_ok: bool,
    pub repair_turns: u32,
    pub error: Option<String>,
    pub baseline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchSummary {
    pub category: Option<String>,
    pub variant: SyntaxVariant,
    pub trials: usize,
    pub first_pass_ok: usize,
    pub first_pass_rate: f64,
    pub mean_repair_turns: f64,
    pub mean_prompt_tokens_est: f64,
    pub mean_completion_tokens_est: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchDecision {
    InsufficientData,
    Go,
    NoGo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    pub decision: BenchDecision,
    pub decision_reasons: Vec<String>,
    pub go_no_go_criteria: Vec<String>,
    pub notes: Vec<String>,
    pub prompts: Vec<VariantPrompt>,
    pub trials: Vec<BenchTrial>,
    pub summaries: Vec<BenchSummary>,
    pub category_summaries: Vec<BenchSummary>,
}

/// Rough token estimate: whitespace-separated words (good enough for relative compare).
pub fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace().count().max(1)
}

pub fn build_variant_prompts(agents_md: &str, tasks: &[TaskSeed]) -> Vec<VariantPrompt> {
    let mut out = Vec::new();
    for task in tasks {
        for variant in [SyntaxVariant::ClassIs, SyntaxVariant::SubjectFirst] {
            let task_body = format!(
                "{}\n\n# Syntax variant\n\n{}",
                task.description,
                variant.syntax_guidance()
            );
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

#[derive(Debug, Deserialize, Serialize)]
pub struct TrialInput {
    pub task_id: String,
    pub variant: SyntaxVariant,
    pub completion: String,
    /// Number of repair turns after the first attempt (0 = first-pass green).
    #[serde(default)]
    pub repair_turns: u32,
}

pub fn read_trial_jsonl(path: &Path) -> Result<Vec<TrialInput>, String> {
    let file = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut trials = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| format!("read {}: {e}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        let trial = serde_json::from_str(&line)
            .map_err(|e| format!("parse {} line {}: {e}", path.display(), index + 1))?;
        trials.push(trial);
    }
    Ok(trials)
}

pub fn score_trials(
    inputs: &[TrialInput],
    tasks: &[TaskSeed],
    prompts: &[VariantPrompt],
) -> Vec<BenchTrial> {
    inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let source = extract_program(&input.completion);
            let emit_root = std::env::temp_dir().join(format!(
                "silc-subject-first-bench-{}-{index}",
                std::process::id()
            ));
            let check = check_source(&source, Some(&emit_root));
            let _ = fs::remove_dir_all(&emit_root);
            let (first_pass_ok, error) = match check {
                Ok(_) => (input.repair_turns == 0, None),
                Err(e) => (false, Some(e)),
            };
            let category = tasks
                .iter()
                .find(|task| task.id == input.task_id)
                .map(|task| task.category.clone())
                .unwrap_or_else(|| "unknown".into());
            let prompt_tokens_est = prompts
                .iter()
                .find(|prompt| prompt.task_id == input.task_id && prompt.variant == input.variant)
                .map(|prompt| prompt.prompt_tokens_est)
                .unwrap_or_default();
            BenchTrial {
                task_id: input.task_id.clone(),
                category,
                variant: input.variant,
                prompt_tokens_est,
                completion_tokens_est: estimate_tokens(&input.completion),
                first_pass_ok,
                repair_turns: input.repair_turns,
                error,
                baseline: false,
            }
        })
        .collect()
}

pub fn summarize(trials: &[BenchTrial]) -> Vec<BenchSummary> {
    summarize_for_category(trials, None)
}

fn summarize_for_category(trials: &[BenchTrial], category: Option<&str>) -> Vec<BenchSummary> {
    let mut out = Vec::new();
    for variant in [SyntaxVariant::ClassIs, SyntaxVariant::SubjectFirst] {
        let subset: Vec<_> = trials
            .iter()
            .filter(|t| !t.baseline && t.variant == variant)
            .filter(|t| category.is_none_or(|category| t.category == category))
            .collect();
        if subset.is_empty() {
            continue;
        }
        let n = subset.len() as f64;
        let first_pass_ok = subset.iter().filter(|t| t.first_pass_ok).count();
        let mean_repair = subset.iter().map(|t| t.repair_turns as f64).sum::<f64>() / n;
        let mean_prompt = subset
            .iter()
            .map(|t| t.prompt_tokens_est as f64)
            .sum::<f64>()
            / n;
        let mean_completion = subset
            .iter()
            .map(|t| t.completion_tokens_est as f64)
            .sum::<f64>()
            / n;
        out.push(BenchSummary {
            category: category.map(str::to_owned),
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

pub fn summarize_categories(trials: &[BenchTrial], tasks: &[TaskSeed]) -> Vec<BenchSummary> {
    let mut categories: Vec<&str> = tasks.iter().map(|task| task.category.as_str()).collect();
    categories.sort_unstable();
    categories.dedup();
    categories
        .into_iter()
        .flat_map(|category| summarize_for_category(trials, Some(category)))
        .collect()
}

pub fn decide(
    summaries: &[BenchSummary],
    category_summaries: &[BenchSummary],
    tasks: &[TaskSeed],
) -> (BenchDecision, Vec<String>) {
    let mut categories: Vec<&str> = tasks.iter().map(|task| task.category.as_str()).collect();
    categories.sort_unstable();
    categories.dedup();
    let underpowered: Vec<String> = categories
        .iter()
        .filter_map(|category| {
            let class_n = category_summaries
                .iter()
                .find(|s| {
                    s.category.as_deref() == Some(category) && s.variant == SyntaxVariant::ClassIs
                })
                .map(|s| s.trials)
                .unwrap_or_default();
            let subject_n = category_summaries
                .iter()
                .find(|s| {
                    s.category.as_deref() == Some(category)
                        && s.variant == SyntaxVariant::SubjectFirst
                })
                .map(|s| s.trials)
                .unwrap_or_default();
            (class_n < 20 || subject_n < 20).then(|| {
                format!("{category}: class-is={class_n}, subject-first={subject_n} (need 20 each)")
            })
        })
        .collect();
    if !underpowered.is_empty() {
        return (BenchDecision::InsufficientData, underpowered);
    }

    let class = summaries
        .iter()
        .find(|s| s.variant == SyntaxVariant::ClassIs)
        .expect("powered benchmark has class-is summary");
    let subject = summaries
        .iter()
        .find(|s| s.variant == SyntaxVariant::SubjectFirst)
        .expect("powered benchmark has subject-first summary");
    let first_pass_gain = subject.first_pass_rate - class.first_pass_rate;
    let repair_ok = subject.mean_repair_turns <= class.mean_repair_turns;
    let token_ratio = if class.mean_completion_tokens_est == 0.0 {
        1.0
    } else {
        subject.mean_completion_tokens_est / class.mean_completion_tokens_est
    };
    let tokens_ok = token_ratio <= 1.15 || first_pass_gain > 0.15;
    let rate_ok = first_pass_gain >= 0.10;
    let reasons = vec![
        format!(
            "first-pass gain: {:.1} points (need ≥10.0)",
            first_pass_gain * 100.0
        ),
        format!(
            "mean repair turns: subject-first={:.2}, class-is={:.2}",
            subject.mean_repair_turns, class.mean_repair_turns
        ),
        format!(
            "completion token ratio: {:.3} (limit 1.15 unless gain >15 points)",
            token_ratio
        ),
    ];
    if rate_ok && repair_ok && tokens_ok {
        (BenchDecision::Go, reasons)
    } else {
        (BenchDecision::NoGo, reasons)
    }
}

pub fn go_no_go_criteria() -> Vec<String> {
    vec![
        "Migrate only if subject-first first-pass `silc build` success rate beats class-is by ≥10 absolute percentage points on the same task set.".into(),
        "Mean repair turns to green must not increase.".into(),
        "Mean completion tokens must not increase by >15% unless first-pass gain exceeds 15 points.".into(),
        "Decision deferred until both variants have ≥20 scored trials per task family.".into(),
    ]
}

/// Record current fixture baselines under both historical report labels.
pub fn baseline_fixture_trials() -> Vec<BenchTrial> {
    const FIXTURES: &[(&str, &str, &str)] = &[
        (
            "shopping_crud",
            "resources",
            include_str!("../../silc/tests/fixtures/shopping_app.silc"),
        ),
        (
            "scored_form",
            "form",
            include_str!("../../silc/tests/fixtures/scored_form.silc"),
        ),
        (
            "data_pipeline",
            "pipeline",
            include_str!("../../silc/tests/fixtures/data_pipeline_runnable.silc"),
        ),
    ];
    let mut trials = Vec::new();
    for (task_id, category, source) in FIXTURES {
        let result = check_source(source, None);
        let ok = result.is_ok();
        let error = result.err();
        trials.push(BenchTrial {
            task_id: (*task_id).into(),
            category: (*category).into(),
            variant: SyntaxVariant::ClassIs,
            prompt_tokens_est: 0,
            completion_tokens_est: estimate_tokens(source),
            first_pass_ok: ok,
            repair_turns: 0,
            error,
            baseline: true,
        });
        let subject_result = check_source(source, None);
        let subject_ok = subject_result.is_ok();
        trials.push(BenchTrial {
            task_id: (*task_id).into(),
            category: (*category).into(),
            variant: SyntaxVariant::SubjectFirst,
            prompt_tokens_est: 0,
            completion_tokens_est: estimate_tokens(source),
            first_pass_ok: subject_ok,
            repair_turns: 0,
            error: subject_result.err(),
            baseline: true,
        });
    }
    trials
}

pub fn run_benchmark(
    agents_path: &Path,
    tasks_dir: &Path,
    trials_paths: &[std::path::PathBuf],
    out_path: &Path,
) -> Result<BenchReport, String> {
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
    for path in trials_paths {
        let inputs = read_trial_jsonl(path)?;
        trials.extend(score_trials(&inputs, &tasks, &prompts));
    }
    let summaries = summarize(&trials);
    let category_summaries = summarize_categories(&trials, &tasks);
    let (decision, decision_reasons) = decide(&summaries, &category_summaries, &tasks);
    let report = BenchReport {
        decision,
        decision_reasons,
        go_no_go_criteria: go_no_go_criteria(),
        notes: vec![
            "Baseline records current fixtures under both historical report labels and is excluded from decision metrics.".into(),
            "Agent TrialInput rows determine go/no-go metrics.".into(),
            "Silc 0.4.0 validates both variants directly; legacy class-is inputs receive migration diagnostics.".into(),
        ],
        prompts,
        trials,
        summaries,
        category_summaries,
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
    fn baseline_fixtures_compile_under_current_parser() {
        let trials = baseline_fixture_trials();
        assert!(
            trials.iter().all(|t| t.first_pass_ok),
            "current fixtures must compile for both historical report labels: {trials:#?}"
        );
    }

    #[test]
    fn current_subject_first_source_checks_directly() {
        let source = r#"@version("0.4.0")
contract Note { has Str $.text; }
component Page {
    method render() { ui::page(ui::heading(:text("Hi"))) }
}
app Notes {
    route "/" => Page;
}
"#;
        assert!(check_source(source, None).is_ok());
    }

    #[test]
    fn decision_requires_twenty_trials_per_category() {
        let tasks = vec![TaskSeed {
            id: "t1".into(),
            category: "ui".into(),
            description: "Build a tiny app".into(),
            tags: vec![],
        }];
        let trials = vec![];
        let summaries = summarize(&trials);
        let category_summaries = summarize_categories(&trials, &tasks);
        let (decision, _) = decide(&summaries, &category_summaries, &tasks);
        assert_eq!(decision, BenchDecision::InsufficientData);
    }
}
