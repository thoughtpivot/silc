//! Assist session loop (ADR-008).

use std::io::Write;
use std::time::Instant;

use crate::complete::Completer;
use crate::corpus::Corpus;
use crate::prompt::{root_bootstrap, truncate_for_history};
use crate::tools::{
    execute_tool, parse_turn, resolve_final, resolve_final_var, BudgetStats, Budgets, ParsedTurn,
    ToolOutcome, ToolState,
};

#[derive(Debug, Clone)]
pub struct AssistResult {
    pub program: String,
    pub stats: BudgetStats,
    /// True when the model finished with FINAL; false when the loop salvaged a
    /// check-passing draft after a budget ran out.
    pub finalized: bool,
}

#[derive(Debug)]
pub enum AssistError {
    Budget(String),
    Completer(String),
    Failed(String),
}

impl std::fmt::Display for AssistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Budget(m) | Self::Completer(m) | Self::Failed(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for AssistError {}

/// Run the closed-tool RLM loop until FINAL or budgets exhaust.
pub fn run_assist(
    task: &str,
    corpus: &Corpus,
    completer: &mut dyn Completer,
    budgets: &Budgets,
    mut progress: Option<&mut dyn Write>,
) -> Result<AssistResult, AssistError> {
    let started = Instant::now();
    let mut history = root_bootstrap(task, corpus);
    let mut state = ToolState::default();
    let mut recent_calls: Vec<String> = Vec::new();

    while state.stats.root_turns < budgets.max_root_turns {
        if started.elapsed().as_secs() >= budgets.wall_clock_secs {
            return salvage_draft(
                state,
                progress,
                &format!("wall clock budget exhausted ({}s)", budgets.wall_clock_secs),
            );
        }

        state.stats.root_turns += 1;
        if let Some(w) = progress.as_mut() {
            let _ = writeln!(w, "silc assist: root turn {}", state.stats.root_turns);
        }

        let response = completer
            .complete(&history)
            .map_err(AssistError::Completer)?;

        match parse_turn(&response) {
            ParsedTurn::Tool(call) => {
                if let Some(w) = progress.as_mut() {
                    let _ = writeln!(w, "silc assist: tool {}", call.name);
                }
                let outcome = execute_tool(&call, corpus, &mut state, budgets, completer)
                    .map_err(AssistError::Failed)?;
                match outcome {
                    ToolOutcome::Continue(meta) => {
                        if let Some(w) = progress.as_mut() {
                            let _ = writeln!(
                                w,
                                "silc assist: {}",
                                truncate_for_history(&meta, 160).replace('\n', " ")
                            );
                        }
                        let call_key = format!("{}:{}", call.name, call.args);
                        let repeated = recent_calls.contains(&call_key);
                        recent_calls.push(call_key);
                        if recent_calls.len() > 4 {
                            recent_calls.remove(0);
                        }

                        history.push_str("\n# Assistant\n");
                        history.push_str(&truncate_for_history(&response, 1200));
                        history.push_str("\n# Tool result\n");
                        history.push_str(&truncate_for_history(&meta, 3000));
                        if repeated {
                            history.push_str(
                                "\n# Note\nYou already made this exact tool call; the result is unchanged. Take a different action — e.g. draft_set a program adapted from what you read, then silc_check.\n",
                            );
                        }
                        let exploring_too_long = state.draft.trim().is_empty()
                            && state.stats.root_turns * 2 >= budgets.max_root_turns;
                        if state.last_check_ok {
                            history.push_str(
                                "\n# Next\nsilc_check passed. If the draft fulfills the task, reply with exactly this line and nothing else:\nFINAL_VAR(draft)\nOtherwise improve the draft with draft_set and re-run silc_check.\n",
                            );
                        } else if exploring_too_long {
                            history.push_str(
                                "\n# Next\nStop exploring. Write the COMPLETE Silc program for the task now, inside a ```silc fenced block, adapted from the example you read. Do not call any more corpus tools.\n",
                            );
                        } else {
                            history.push_str("\n# Next\nCall the next tool (or FINAL_VAR(draft) after silc_check ok).\n");
                        }
                        // Keep history bounded for 8K models.
                        history = truncate_history(&history, 24_000);
                    }
                    ToolOutcome::Finished(program) => {
                        return Ok(AssistResult {
                            program,
                            stats: state.stats,
                            finalized: true,
                        });
                    }
                }
            }
            ParsedTurn::Final(program) => match resolve_final(&program, &mut state) {
                Ok(program) => {
                    if let Some(w) = progress.as_mut() {
                        let _ = writeln!(w, "silc assist: FINAL accepted");
                    }
                    return Ok(AssistResult {
                        program,
                        stats: state.stats,
                        finalized: true,
                    });
                }
                Err(error) => {
                    if let Some(w) = progress.as_mut() {
                        let _ = writeln!(w, "silc assist: FINAL rejected");
                    }
                    history.push_str("\n# Assistant\n");
                    history.push_str(&truncate_for_history(&response, 1200));
                    history.push_str("\n# Error\n");
                    history.push_str(&error);
                    history.push_str("\n# Next\nRepair with tools, then FINAL again.\n");
                    history = truncate_history(&history, 24_000);
                }
            },
            ParsedTurn::FinalVar => match resolve_final_var(&state) {
                Ok(program) => {
                    if let Some(w) = progress.as_mut() {
                        let _ = writeln!(w, "silc assist: FINAL_VAR(draft) accepted");
                    }
                    return Ok(AssistResult {
                        program,
                        stats: state.stats,
                        finalized: true,
                    });
                }
                Err(error) => {
                    history.push_str("\n# Assistant\nFINAL_VAR(draft)\n# Error\n");
                    history.push_str(&error);
                    history.push_str(
                        "\n# Next\nUse silc_check on the draft, then FINAL_VAR(draft).\n",
                    );
                    history = truncate_history(&history, 24_000);
                }
            },
            ParsedTurn::Invalid(msg) => {
                if let Some(w) = progress.as_mut() {
                    let _ = writeln!(w, "silc assist: invalid turn ({msg})");
                }
                history.push_str("\n# Assistant\n");
                history.push_str(&truncate_for_history(&response, 800));
                history.push_str("\n# Error\n");
                history.push_str(&msg);
                history.push_str("\n# Next\nEmit a valid ```tool``` block.\n");
                history = truncate_history(&history, 24_000);
            }
        }
    }

    let reason = format!("root turn budget exhausted ({})", budgets.max_root_turns);
    salvage_draft(state, progress, &reason)
}

/// On budget exhaustion, return a check-passing draft rather than failing.
fn salvage_draft(
    state: ToolState,
    mut progress: Option<&mut dyn Write>,
    reason: &str,
) -> Result<AssistResult, AssistError> {
    if state.last_check_ok && !state.draft.trim().is_empty() {
        if let Some(w) = progress.as_mut() {
            let _ = writeln!(
                w,
                "silc assist: {reason}; returning last silc_check-passing draft"
            );
        }
        return Ok(AssistResult {
            program: state.draft,
            stats: state.stats,
            finalized: false,
        });
    }
    Err(AssistError::Budget(reason.to_string()))
}

fn truncate_history(history: &str, max_chars: usize) -> String {
    let count = history.chars().count();
    if count <= max_chars {
        return history.to_string();
    }
    let skip = count - max_chars;
    let trimmed: String = history.chars().skip(skip).collect();
    format!("…[history truncated]\n{trimmed}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complete::ScriptedCompleter;

    #[test]
    fn scripted_loop_checks_and_finalizes() {
        let corpus = Corpus::builtin();
        let source = corpus.get("fixture/scored_form.silc").unwrap().to_string();
        let escaped = serde_json::to_string(&source).unwrap();
        let mut completer = ScriptedCompleter::new([
            format!("```tool\n{{\"name\":\"corpus_list\",\"args\":{{}}}}\n```"),
            format!("```tool\n{{\"name\":\"draft_set\",\"args\":{{\"source\":{escaped}}}}}\n```"),
            format!("```tool\n{{\"name\":\"silc_check\",\"args\":{{}}}}\n```"),
            "FINAL_VAR(draft)".to_string(),
        ]);
        let budgets = Budgets::default();
        let result = run_assist(
            "make a scored form",
            &corpus,
            &mut completer,
            &budgets,
            None,
        )
        .expect("assist");
        assert!(result.program.contains("FeedbackRecord"));
        assert_eq!(result.stats.root_turns, 4);
        // draft_set auto-checks, then the explicit silc_check call.
        assert_eq!(result.stats.checks, 2);
        assert!(result.finalized);
    }

    #[test]
    fn budget_exhaustion() {
        let corpus = Corpus::builtin();
        let mut completer = ScriptedCompleter::new([
            "```tool\n{\"name\":\"corpus_list\",\"args\":{}}\n```".to_string(),
            "```tool\n{\"name\":\"corpus_list\",\"args\":{}}\n```".to_string(),
        ]);
        let budgets = Budgets {
            max_root_turns: 2,
            ..Budgets::default()
        };
        let err = run_assist("x", &corpus, &mut completer, &budgets, None).unwrap_err();
        assert!(err.to_string().contains("root turn budget"));
    }
}
