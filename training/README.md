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

## Product note

**silclm** is Silc's owned local model identity. Chat apps already default to
it (`llm::complete` / omit `:model`). This bank is the path to a fine-tuned
silclm for in-app chat, Silc assist flows, and cheap local authoring help.
See [docs/ADR-005-local-llm-complete.md](../docs/ADR-005-local-llm-complete.md).
