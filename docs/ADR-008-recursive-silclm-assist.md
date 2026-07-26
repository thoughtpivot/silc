# ADR-008: Recursive silclm Assist (`silc-rlm` / `silc assist`)

- **Status:** Accepted (Phase 1 scaffold)
- **Date:** 2026-07-27
- **Updated:** 2026-07-27
- **Related:** [ADR-005](ADR-005-local-llm-complete.md),
  [ADR-002](ADR-002-silc-surface-syntax.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ADR-INDEX.md](ADR-INDEX.md),
  training harness under `training/`
- **Canonical:** [`crates/sil-rlm/`](../crates/sil-rlm/),
  [`crates/silc/src/assist.rs`](../crates/silc/src/assist.rs)

## Context

[ADR-005](ADR-005-local-llm-complete.md) shipped local `llm::complete` via
**silclm** (now Llama 3.2 3B Instruct GGUF, 8K context) and deferred both a
fine-tuned GGUF and a `silc assist` authoring CLI. In-app chat remains a
grounded product surface; authoring help is a separate problem.

Silc authoring knowledge lives in a large contract:
[`AGENTS.md`](../crates/silc/templates/AGENTS.md), `examples/*/main.silc`,
compiler fixtures, and banked programs from `sil-training`. Stuffing that
corpus into an 8K window fails: context rot, truncated guidance, and a small
model that invents React/Ollama instead of valid `.silc`.

**Recursive Language Models** (MIT; Ovid’s Shesha/Ananta) treat a long prompt
as part of an external environment. The model writes code (or tool calls) to
peek, search, chunk, and recursively call itself on snippets — keeping only
short metadata in the root window. Silc’s advantage over generic repo explorers
is a hard oracle: `sil-training`’s `check_source` (parse → validate → route).

## Decision

### Product surface

Phase 1 ships an experimental CLI:

```text
silc assist "<task>" [--out path.silc] [--corpus <dir>] [--max-turns N]
```

This is a compiler/CLI product, same class as `silc init` and `sil-training`.
It is **not** an in-app `ui::chat` mode. In-app chat stays ADR-005
`llm::complete`.

### Out of the language

No `rlm::*` ops, no `is agent` / `is assist` modules, and no `.silc` syntax
changes. Authors never name the RLM scaffold in programs.

### Closed-tool RLM (not open Python)

Phase 1 uses a **Rust-hosted closed tool loop**. Tools are the symbolic
environment; recursion is `llm_query`. An open Python REPL (Shesha-style) is
deferred: safer for local runs and more reliable for a small root model.

| Tool | Purpose | Result into root history |
| --- | --- | --- |
| `corpus_list()` | Enumerate doc/example ids + sizes | Short index |
| `corpus_grep(pattern, path?)` | Search over corpus | Truncated matches |
| `corpus_read(id, start, len)` | Slice read (cap 4000 chars) | Truncated slice + meta |
| `silc_check(source)` | `check_source` tier 2 | `{ok, stage, error, mode}` — no AST dump |
| `llm_query(prompt)` | Depth-1 leaf completion (same silclm) | Truncated answer (~2k) |
| `draft_set(source)` / `draft_get()` | Working buffer for the candidate | Meta / short preview |
| `FINAL` / `FINAL_VAR(draft)` | Terminate | Emit `.silc` |

The root model emits **one** structured tool call per turn inside a fenced
JSON block:

````text
```tool
{"name":"corpus_list","args":{}}
```
````

Invalid tool calls return a short error and consume a turn. `FINAL` may also
appear as a bare line `FINAL(...)` or `FINAL_VAR(draft)` after a successful
check.

### Budgets (hard defaults)

| Budget | Default |
| --- | --- |
| `max_depth` | 1 |
| `max_root_turns` | 12 |
| `max_silc_check` | 8 |
| `max_llm_query` | 16 |
| `max_read_chars` | 4000 per `corpus_read` |
| `wall_clock_secs` | 900 |

Root prompt = assist system prompt + user task + **environment metadata only**
(corpus size, tool list). Full `AGENTS.md` is corpus id `agents`, not preloaded.

### Model identity

Phase 1 uses catalog id **`silclm`** with an assist system prompt (same pinned
GGUF as chat). Future distilled weights reserve catalog id **`silclm-assist`**;
no fine-tuned GGUF ships in this slice.

### Corpus

Read-only bundle at assist time:

- `crates/silc/templates/AGENTS.md` → id `agents`
- `examples/*/main.silc` (+ sibling `AGENTS.md` when present)
- Selected `crates/silc/tests/fixtures/*.silc`

No RAG index in Phase 1. `--corpus <dir>` adds extra files.

### Validator

`silc_check` calls `sil_training::check_source` at validation tier 2 (parse →
validate → route; no emit / no runtime provision). Only programs that pass may
be returned via `FINAL` / `FINAL_VAR(draft)`.

### Training schema (Phase 2 — specify now)

Future trajectory JSONL (filter: `accepted` and successful `silc_check`):

```json
{
  "id": "traj-…",
  "task_id": "…",
  "task": "…",
  "agents_md_version": "0.3.0",
  "target_model": "silclm-assist",
  "turns": [
    {"role": "root", "content": "…"},
    {"role": "tool", "name": "corpus_grep", "args": {}, "result_meta": "…"},
    {"role": "sub_llm", "prompt_meta": "…", "completion_meta": "…"}
  ],
  "final_program": "…",
  "accepted": true,
  "check_stage": null,
  "budget_stats": {"root_turns": 5, "checks": 2, "llm_queries": 3}
}
```

SFT recipe (later): distill **root turns** from teacher RLM trajectories that
end in bank-accepted programs (same insight as RLM-Qwen3-8B).

### Phased rollout

| Phase | Ship | Explicit non-goal that phase |
| --- | --- | --- |
| **1** | This ADR + `silc assist` closed-tool loop + corpus + `silc_check` + depth-1 | Fine-tune, open Python REPL, streaming |
| **2** | Trajectory JSONL writer + `sil-training trajectories filter` | Weight training |
| **3** | Distill `silclm-assist` GGUF + catalog entry | Changing `.silc` surface |

## Consequences

- Authoring help can stay tiny: knowledge lives in the corpus + compiler check,
  not only in weights.
- `llm::complete` app chat and `silc assist` stay separate scaffolds on the same
  (or later sibling) catalog ids.
- ADR-005’s deferred “`silc assist` authoring CLI” is superseded by this ADR’s
  Phase 1 scaffold; fine-tuned weights remain Phase 3.
- Boundary crate `sil-rlm` owns the tool loop; `silc` CLI stays thin orchestration
  (runtimes, model ensure, I/O).

## Non-goals

- Open Python / arbitrary code-exec sandbox in Phase 1
- Mixing assist into `llm::complete` or `ui::chat` personas
- Author-selectable engines or GGUF paths in `.silc`
- Shipping fine-tuned weights or `silclm-assist` catalog pinning in this slice
- MCP / IDE agent protocol
- Streaming tokens
