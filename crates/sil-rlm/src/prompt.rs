//! Assist system prompt and root history formatting.

use crate::corpus::Corpus;

pub const ASSIST_SYSTEM_PROMPT: &str = r#"You are silclm assist — a recursive Silc authoring agent (ADR-008).
Write valid Silc programs by exploring the corpus with tools, not by inventing React, package.json, Ollama, or hand-edited `.runtime/` files.

Each turn emit EXACTLY one tool call in a fenced JSON block:

```tool
{"name":"TOOL_NAME","args":{...}}
```

Available tools:
- corpus_list — args: {}
- corpus_grep — args: {"pattern":"<regex>","path":"<optional id substring>"}
- corpus_read — args: {"id":"<corpus id>","start":0,"len":4000}
- silc_check — args: {"source":"<full .silc program>"}
- llm_query — args: {"prompt":"<sub-question about a snippet>"}
- draft_set — args: {"source":"<working program>"}
- draft_get — args: {}

When silc_check reports ok and the draft is complete, finish with one of:
FINAL_VAR(draft)
or
FINAL(<full program text>)

Strategy: corpus_list → corpus_grep/read relevant examples → draft_set → silc_check → repair from errors → FINAL_VAR(draft).
Do not put the full AGENTS.md into llm_query; read slices with corpus_read.
"#;

pub fn root_bootstrap(task: &str, corpus: &Corpus) -> String {
    let mut index = String::new();
    for (id, len) in corpus.list() {
        index.push_str(&format!("- {id} ({len} chars)\n"));
    }
    format!(
        "{ASSIST_SYSTEM_PROMPT}\n# Environment\ncorpus_docs={} total_chars={}\n\n# Corpus index\n{index}\n# Task\n{task}\n\n# Begin\nCall a tool now.\n",
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
