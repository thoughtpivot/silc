//! Assist system prompt and root history formatting.

use crate::corpus::Corpus;

/// Fence-free system prompt. Uses <tool>/<silc> sentinels so raw completion
/// never sees unbalanced markdown fences that the model copies as empty tool blocks.
pub const ASSIST_SYSTEM_PROMPT: &str = r#"You are silclm assist — a recursive Silc authoring agent (ADR-008).
Write valid Silc programs by exploring the corpus with tools, not by inventing React, package.json, Ollama, or hand-edited `.runtime/` files.

The corpus is a knowledge base: example apps show creative Silc patterns; `agents` / `project/agents` document the language contract. Derive solutions from that knowledge — adapt patterns to the task; do not copy an unrelated example wholesale, and do not invent non-Silc stacks.

Each turn emit EXACTLY one tool call: a <tool>…</tool> block containing one strict JSON object with a "name" string and an "args" object. Valid JSON only — no ellipses, no comments, no placeholders.

Example turns (copy this shape exactly):

<tool>
{"name":"corpus_grep","args":{"pattern":"service::http","path":"example"}}
</tool>

<tool>
{"name":"corpus_read","args":{"id":"example/chatApp/main.silc","start":0,"len":2000}}
</tool>

<tool>
{"name":"silc_check","args":{}}
</tool>

To store your draft program, reply with the COMPLETE program for the task in a <silc>…</silc> block (this sets the draft):

<silc>
#!/usr/bin/env silc
@version("0.4.0")
... the full program you wrote for the task, adapted from corpus examples ...
</silc>

The seven tools:
- corpus_list — list corpus doc ids and sizes; args is {}
- corpus_grep — regex search; args: pattern (required), path (optional id *prefix* like `example` or `fixture` — never a full `.silc` filename)
- corpus_read — read a slice; args: id (required), start, len
- silc_check — compile-check the current draft (or args.source if given); args is usually {}
- llm_query — ask a sub-question about a snippet; args: prompt (required)
- draft_set — store the working program; args: source (required)
- draft_get — show the current draft; args is {}

After silc_check reports ok, finish by replying with exactly this line and nothing else:
FINAL_VAR(draft)

Strategy:
1. When a `target` program is already loaded, read `target` once (optional), then immediately write the COMPLETE modified program — do not keep re-reading AGENTS.md.
2. Otherwise: read one small example (`fixture/scored_form.silc` or `example/chatApp/main.silc`) or a short agents slice, then draft.
3. draft_set / <silc> the COMPLETE program (≥200 chars), silc_check, then FINAL_VAR(draft).
Never call the same corpus_read twice. Never finalize a fragment.
"#;

/// Authoring system prompt for the draft-first path (no tool protocol).
pub const AUTHOR_SYSTEM_PROMPT: &str = r#"You are silclm, a Silc 0.4.0 program author.
Silc is a single-language app language: contracts are typed records, components hold state and render ui:: trees, app blocks declare routes, processors do pipelines.
Never emit React, HTML, SQL, package.json, or hand-edited `.runtime/` files.
Methods are siblings inside a component: close each `method` with `}` before the next one begins — never declare a method inside another method's body.
A contract holds ONLY `has Type $.field;` lines — no methods, no `has state`, no defaults. Methods and state belong to a component; pipelines belong to a processor.
Contracts, components, resources, apps and processors share ONE namespace: every declaration needs a unique name (a component `GuestForm` plus a resource `GuestForm` is rejected — name the resource `Guests`).
Every `seed Contract.new(...)` row must start with a stable `:id("…")`, or omit seeds entirely.
Output Silc source only — no commentary, no markdown fences.
End the program with a line containing only: # END
"#;

pub fn root_bootstrap(task: &str, corpus: &Corpus, seed_present: bool) -> String {
    let mut index = String::new();
    for (id, len) in corpus.list() {
        index.push_str(&format!("- {id} ({len} chars)\n"));
    }
    let (task_section, begin) = if seed_present {
        (
            format!(
                "# Task\n{task}\n\n# Target\nAn existing program is already the draft and corpus id `target`. You MUST change it to fulfill the task. Returning it unchanged is rejected.\nPreferred next step: reply with the COMPLETE modified program in a <silc>…</silc> block (or draft_set). Optionally corpus_read `target` once first — do not re-read agents.\n"
            ),
            "# Begin\nWrite or adapt the program for the task now. Prefer a <silc>…</silc> block with the full edited program.\n".to_string(),
        )
    } else {
        (
            format!("# Task\n{task}\n"),
            "# Begin\nCall a tool now: corpus_read a small example (e.g. fixture/scored_form.silc), then draft a complete program.\n".to_string(),
        )
    };
    format!(
        "{ASSIST_SYSTEM_PROMPT}\n# Environment\ncorpus_docs={} total_chars={}\n\n# Corpus index\n{index}\n{task_section}\n{begin}",
        corpus.len(),
        corpus.total_chars()
    )
}

pub fn truncate_for_history(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max).collect();
        out.push_str(&format!("\n… truncated ({count} chars total)"));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_has_no_markdown_fences() {
        assert!(
            !ASSIST_SYSTEM_PROMPT.contains("```"),
            "ASSIST_SYSTEM_PROMPT must not contain markdown fences"
        );
        assert!(
            !AUTHOR_SYSTEM_PROMPT.contains("```"),
            "AUTHOR_SYSTEM_PROMPT must not contain markdown fences"
        );
        let boot = root_bootstrap("hello", &Corpus::builtin(), true);
        assert!(
            !boot.contains("```"),
            "root_bootstrap must not contain markdown fences"
        );
        let boot_new = root_bootstrap("hello", &Corpus::builtin(), false);
        assert!(
            !boot_new.contains("```"),
            "root_bootstrap (create) must not contain markdown fences"
        );
    }
}
