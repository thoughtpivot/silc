# silclm training foundation

This directory holds **seed tasks** and docs for Silc's provider-neutral dataset
pipeline. The goal is a future fine-tuned **silclm** that understands Silc
authoring — without baking any one cloud LLM into the compiler.

## What lands here

| Path | Tracked? | Role |
| --- | --- | --- |
| `tasks/*.json` | yes | Reviewed natural-language task seeds |
| `README.md` | yes | Workflow + interchange contracts |
| `out/` | **no** (gitignored) | Generated prompts, candidates, banks |

Bulk generated data is never source-controlled.

## Tools

```bash
# 1) Build model-ready prompts from AGENTS.md + task seeds
cargo run -p sil-training -- prompts \
  --agents crates/silc/templates/AGENTS.md \
  --tasks training/tasks \
  --out training/out/prompts.jsonl

# 2) Any generator writes candidates.jsonl (schema below)

# 3) Bank only programs that pass Silc parse/validate/route/emit
cargo run -p sil-training -- bank \
  --candidates training/out/candidates.jsonl \
  --accepted training/out/accepted.jsonl \
  --rejected training/out/rejected.jsonl
```

`bank` uses the compiler crates directly (no `silc build`, no runtime
provisioning). Pass `--no-emit` to stop after classify (faster; tier 2).

## Interchange schemas

### Prompt record (`prompts.jsonl`)

```json
{
  "id": "prompt-scored_form",
  "task_id": "scored_form",
  "category": "form",
  "task": "…",
  "agents_md_version": "0.2.0",
  "prompt": "You are silclm…",
  "prompt_sha256": "…",
  "target_model": "silclm"
}
```

### Candidate record (any provider → `candidates.jsonl`)

```json
{
  "prompt_id": "prompt-scored_form",
  "prompt": "optional full prompt text",
  "task": "optional task text",
  "completion": "```silc\n@version(\"0.2.0\")\n…\n```",
  "model": "optional-generator-id",
  "category": "form"
}
```

### Accepted / rejected

Accepted rows include `program`, `program_sha256`, `execution_mode`,
`validation_tier`, and `target_model: "silclm"`. Rejected rows keep the raw
`completion` plus structured `stage`/`error` for future repair training.

## Subject-first declarator benchmark

Evaluate `class X is resource` vs hypothetical `resource X` **without migrating
syntax**. See [docs/subject-first-declarators.md](../docs/subject-first-declarators.md).

```bash
cargo run -p sil-training -- subject-first-bench \
  --agents crates/silc/templates/AGENTS.md \
  --tasks training/tasks \
  --out training/out/subject_first_bench.json
```

## Product note

**silclm** is Silc's owned local model identity. Chat apps already default to
it (`llm::complete` / omit `:model`). This bank is the path to a fine-tuned
silclm for in-app chat and cheap local authoring help.
See [docs/ADR-005-local-llm-complete.md](../docs/ADR-005-local-llm-complete.md).

## Future: RLM trajectories (ADR-008)

[`silc assist`](../docs/ADR-008-recursive-silclm-assist.md) explores AGENTS +
examples via a closed-tool recursive loop and validates drafts with
`check_source`. Phase 2 will record trajectory JSONL (root / tool / sub_llm
turns) and filter to bank-accepted programs for a future `silclm-assist`
distillation. Until then, continue banking final completions with `bank` as
above.
