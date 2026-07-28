# ADR-008: Recursive silclm Assist (`silc-rlm` / `silc assist`)

- **Status:** Accepted (Phase 1 scaffold)
- **Date:** 2026-07-27
- **Updated:** 2026-07-28 (draft-first efficiency: # END stop, error retrieval, --explore)
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
silc assist "<task>" <path.silc> [--corpus <dir>] [--max-turns N] [--explore]
```

The task is the first argument; the second is the target `.silc` path (created
if missing, seeded for modify-in-place when present). Optional flags control
budgets only — RLM internals (`FINAL_VAR`, draft buffers) stay off the terminal.
Progress uses a lightweight spinner (`indicatif` + `console`) plus a durable
**action trace**: each model turn prints what it searched, which corpus slice it
read, when it drafted or repaired code, whether the compiler check passed, and
how long inference took. This is not raw chain-of-thought; silclm emits tool
decisions (or a complete program on the draft-first path), and the CLI
summarizes those decisions for the operator.

The completer is a **persistent warm worker** (`assist_complete.py`): the GGUF
loads once, then serves JSON-line requests over stdin/stdout so subsequent turns
reuse the llama.cpp KV-cache prefix. Requests may be raw `{"prompt":…}` or chat
`{"messages":[…]}` (Llama 3.2 instruct template). Responses include
`truncated: true` when generation hit `max_tokens`. Default
`SILC_LLM_N_GPU_LAYERS=-1` offloads all layers to the GPU (Metal on Apple
Silicon). Assist uses a **16_384** context window by default (overridable via
`SILC_LLM_N_CTX`) and **4096** max new tokens. Draft-first chat stops on
`# END` so the model does not invent extra components after the program.
Root history for the opt-in tool-loop is capped at **16_000 chars**.
Default wall clock is **120s**.

### Draft-first primary path

Before any tool loop, assist **auto-retrieves** a condensed rules digest
from `project/agents` / `agents` plus the top 1–2 corpus examples scored against
the task keywords, injects the current target file, and asks silclm (via the
chat template) to output **only** a complete Silc program ending with `# END`.
When **creating** a file (no existing target), the `silc init` **starter** —
the smallest known-good program — is injected as a skeleton to adapt, framed
with "keep `on_submit` as `submit();`, methods are siblings, don't invent
resources/pipelines". This is the same shape the modify path already succeeds
with, and it makes the from-scratch draft pass on attempt 1. Returning the
starter unchanged is rejected and re-prompted.

Each attempt's token budget is **scaled to the target** (`chars/3 + slack`,
clamped to `[1200, draft_max_tokens]`) so a degenerate generation can't run to
the ceiling (a form-sized task once produced 17k chars in 91s). The author loop
also honours `wall_clock_secs` between attempts. Truncated drafts that already
look complete (`@version` + `app` + `route`) are compile-checked instead of
regenerated blindly.

Failed compiles are repaired in three escalating ways, cheapest first:

1. **Deterministic autofix.** Mechanical diagnostics are repaired by assist
   itself with no model round-trip, then re-verified with `check_source` (a
   failed fix is discarded): a resource block emitted twice is de-duplicated, a
   resource whose name collides with a component is renamed (plus its call sites,
   avoiding double-plurals like `Guestss`), `seed` rows missing a stable `:id`
   are dropped, a method mistakenly nested inside another is hoisted to a
   sibling, and a missing `@version` pragma is inserted.
2. **Rule-targeted guidance.** Structural diagnostics (name collisions, seed
   `:id`, unknown props, closed enums, missing routes) map to an explicit rule
   plus the concrete edit to make. Generic corpus hits do not teach the model
   *which* declaration to change, so retrieval is skipped when a rule matches.
3. **Error-targeted corpus retrieval** across *all* examples and AGENTS (no
   model-chosen file filter), used only when no rule matches.

Because the author samples near-greedily, a repair prompt that barely changes
reproduces the rejected draft byte-for-byte. Assist detects an identical draft,
skips the redundant compile, and retries with **temperature escalated** by 0.3
(0.2 → 0.9 ceiling) plus an explicit "do not repeat your previous output" note.
That note quotes the diagnostic belonging to *that* draft, tracked separately
from the newest error of any kind — reporting a stale diagnostic guarantees the
model repairs the wrong thing.

Some intents need a structure the model does not infer. When a task asks to
store/save/persist data and the target has no `resource`, the prompt injects the
**required pattern** — one top-level `resource` block plus a
`Guests.create(Guest.new(:field($.state)))` call in the submit handler — and
explains that a `processor` computes and discards. Without it, "make submit
actually store the data" reliably failed; with it, it lands on attempt 1.

The most common first-draft failures — the shared-namespace collision, seeds
without `:id`, contracts holding methods, and methods nested in `render()` — are
also stated as rules in the author system prompt, so the usual run passes on
attempt 1. When draft-first exhausts its attempts without `--explore`, the
closest rejected draft is saved next to the target as `<file>.rejected` so the
run is still inspectable.

The closed-tool RLM explore loop is **opt-in** via `--explore`. Without it,
draft-first failure exits with the last compiler diagnostic instead of burning
minutes on empty greps. When explore is enabled, `corpus_grep` with a full
`.silc` path that returns no matches auto-widens to the entire corpus, and two
consecutive no-match greps block further grepping.

Tool/protocol prompts use `<tool>…</tool>` and `<silc>…</silc>` **sentinels**,
never markdown fences. Invalid-turn escalation abandons the tool loop after
three consecutive bad replies.

This is a compiler/CLI product, same class as `silc init` and `sil-training`.
It is **not** an in-app `ui::chat` mode. In-app chat stays ADR-005
`llm::complete`.

The `silc` binary parses subcommands with **clap** (Rust’s Cobra equivalent).

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

The root model emits **one** structured tool call per turn inside a sentinel
block (fence-free):

```text
<tool>
{"name":"corpus_list","args":{}}
</tool>
```

Invalid tool calls return a short error and consume a turn. `FINAL` may also
appear as a bare line `FINAL(...)` or `FINAL_VAR(draft)` after a successful
check. Bare Silc programs and `<silc>…</silc>` blocks are accepted as implicit
`draft_set`.

### Budgets (hard defaults)

| Budget | Default |
| --- | --- |
| `max_depth` | 1 |
| `max_draft_attempts` | 5 (draft-first path; explore loop is opt-in) |
| `draft_max_tokens` | 4096 |
| draft temperature | 0.2, +0.3 per identical repeat, 0.9 ceiling |
| `allow_explore` | false (enable with `--explore`) |
| `max_root_turns` | 24 |
| `max_silc_check` | 16 |
| `max_llm_query` | 24 |
| `max_read_chars` | 4000 per `corpus_read` |
| `wall_clock_secs` | 120 |

Root prompt = assist system prompt + user task + **environment metadata only**
(corpus size, tool list). Full `AGENTS.md` is corpus id `agents`, not preloaded.
A nearest project `AGENTS.md` (walk up from the target path, then cwd) loads as
`project/agents` when present.

### Model identity

Phase 1 uses catalog id **`silclm`** with an assist system prompt (same pinned
GGUF as chat). Future distilled weights reserve catalog id **`silclm-assist`**;
no fine-tuned GGUF ships in this slice.

### Corpus

Read-only bundle at assist time:

- `crates/silc/templates/AGENTS.md` → id `agents`
- `examples/*/main.silc` (+ sibling `AGENTS.md` when present)
- Selected `crates/silc/tests/fixtures/*.silc`
- Nearest project `AGENTS.md` → id `project/agents` (auto-discovered)
- Existing target `.silc` (when modifying) → id `target` and seed draft

No RAG index in Phase 1. `--corpus <dir>` adds extra files.

Tiny draft fragments (< 200 chars) are rejected by `draft_set` so the model
cannot burn turns finalizing placeholders.

When modifying an existing file, the seeded program is recorded as the session
seed. `FINAL` / `FINAL_VAR(draft)` and budget salvage all reject a program that
is byte-identical to that seed, and the CLI leaves the file untouched — assist
never reports success for an unedited program.

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
  "agents_md_version": "0.4.0",
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
