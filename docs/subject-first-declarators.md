# Subject-first declarators

- **Evidence class:** Harness / reproducibility
- **Date:** 2026-07
- **Updated:** 2026-07-27
- **Related:** [subject-first-decision.md](subject-first-decision.md),
  [ADR-002](ADR-002-silc-surface-syntax.md),
  [intent-vs-subjects.md](intent-vs-subjects.md)

> **Historical vocabulary.** “Subject-first” named the 0.3.0 declarator
> migration. From 0.4.0, author-facing docs use *direct declarations*;
> *subjects* are compiler-internal. See
> [intent-vs-subjects.md](intent-vs-subjects.md).

Silc ships direct declarations: `resource X`, `component X`, `app X`,
`contract X`, `service X`, `processor X`, and `task X`. Author `sink` was
present in 0.3.0 and removed in 0.4.0
([ADR-009](ADR-009-compiler-synthesized-runtime.md)).

The benchmark remains reproducible as historical migration evidence. Its
measured no-go result was subsequently superseded by an explicit repository
owner override; see
[subject-first-decision.md](subject-first-decision.md).

## Harness

```bash
cargo run -p sil-training -- subject-first-bench \
  --agents crates/silc/templates/AGENTS.md \
  --tasks training/tasks \
  --out training/out/subject_first_bench.json
```

The report includes:

- Paired prompts (`class-is` vs `subject-first` guidance) for each task seed
- Current fixture baselines retained under both historical report labels
- Token estimates, first-pass success, repair turns (when agent trials are scored)
- Per-task-family summaries and a programmatic `insufficient_data`, `go`, or
  `no_go` decision

Score live agent runs by supplying one or more JSONL files:

```bash
cargo run -p sil-training -- subject-first-bench \
  --agents crates/silc/templates/AGENTS.md \
  --tasks training/tasks \
  --trials training/out/subject-first-ui.jsonl \
  --trials training/out/subject-first-api.jsonl \
  --out training/out/subject_first_bench.json
```

Each line is a `TrialInput`:

```json
{"task_id":"components","variant":"subject-first","completion":"component Page { … }","repair_turns":0}
```

Both variants are sent directly to the current parser. Subject-first /
direct-declaration programs can pass; historical class-is programs receive an
actionable migration diagnostic.

## Go / no-go

Migrate only when:

1. Subject-first first-pass `silc build` success beats class-is by **≥10 absolute
   percentage points** on the same task set.
2. Mean repair turns to green do **not** increase.
3. Mean completion tokens do not rise by **>15%** unless first-pass gain exceeds
   15 points.
4. Both variants have **≥20 scored trials** per task family.

These criteria explain the historical benchmark decision; they no longer gate
the shipped declaration-based surface.

## Decision

The July 2026 benchmark met the sample floor and returned **no-go**. The result
is preserved, while the owner override adopts direct declarations from 0.3.0
onward. Product policy lives in [ADR-002](ADR-002-silc-surface-syntax.md).
