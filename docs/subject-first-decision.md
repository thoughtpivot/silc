# Subject-first declarator decision

- **Evidence class:** Historical
- **Date:** 2026-07
- **Updated:** 2026-07-27
- **Related:** [ADR-002](ADR-002-silc-surface-syntax.md),
  [subject-first-declarators.md](subject-first-declarators.md),
  [intent-vs-subjects.md](intent-vs-subjects.md)
- **Superseded by (partial):** [ADR-002](ADR-002-silc-surface-syntax.md)
  product policy (direct declarations shipped despite benchmark no-go)

> **Historical appendix.** “Subject-first” named the 0.3.0 declarator migration
> from `class … is …` to direct keywords. From 0.4.0, author-facing docs use
> *direct declarations*; *subjects* are compiler-internal. See
> [intent-vs-subjects.md](intent-vs-subjects.md).

**Benchmark decision: no-go (July 2026).**

The measured benchmark result remains no-go. It is retained here as historical
evidence rather than rewritten to justify the subsequent product decision.

## Owner override

The repository owner overrides the benchmark gate and adopts direct
declarations for Silc 0.3.0 onward: `contract`, `component`, `resource`, `app`,
`service`, `processor`, and `task` (author `sink` existed in 0.3.0 and was
removed in 0.4.0 — see [ADR-009](ADR-009-compiler-synthesized-runtime.md)).

The benchmark measured first-attempt model familiarity with two spellings. It
did not measure the long-term language costs of retaining an OO-looking
`class … is …` surface, nor the value of one keyword mapping directly to one IR
concept. Because Silc is pre-1.0 and the migration is mechanical, those
product considerations are authoritative despite the model-success regression.
The no-go figures below remain valid evidence and should inform model training.

## Evidence

The benchmark scored 240 first-attempt agent completions: 20 `class-is` and 20
`subject-first` completions in each of six task families (`api`, `chat`, `form`,
`pipeline`, `resources`, and `ui`). Fixture baselines were excluded from the
decision metrics.

- Class-is: 81/120 first-pass green (**67.5%**)
- Subject-first: 63/120 first-pass green (**52.5%**)
- First-pass difference: **-15.0 percentage points** for subject-first; the
  migration gate required **+10 points**
- Mean repair turns: 0.00 for both variants (all samples were first attempts;
  failed samples were not repaired)
- Mean completion-token estimate: 142.7 class-is vs 135.1 subject-first
  (subject-first ratio 0.947)

Per-family first-pass results:

- API: 85% class-is, 85% subject-first
- Chat: 100% class-is, 0% subject-first
- Form: 20% class-is, 30% subject-first
- Pipeline: 0% for both
- Resources: 100% for both
- UI: 100% for both

Subject-first saved roughly 5% of completion tokens but materially reduced
first-pass success. It therefore failed the primary gate.

## Method and limitations

The samples were generated in six batched agent runs, one per task family,
using the same AGENTS contract and paired syntax guidance. Each batch produced
20 programs per variant without compiling or repairing them first. At the time
of measurement, a training-only rewrite lowered subject-first declarators to
the then-current `class … is …` parser surface. Successful trials also completed
compiler code generation (tier 3); the harness did not provision or launch
runtimes.

The trials satisfy the published sample-count rule but are not 240 independently
launched model sessions. The original no-go was intentionally conservative for
a familiarity-only metric. The owner override later adopted the migration for
language-design reasons while preserving these figures as evidence for model
training.

## Original benchmark consequence

- Do not reserve or add subject-first keywords to the product lexer/parser.
- Do not migrate examples, fixtures, AGENTS.md, docs, or banked programs.
- Keep the benchmark CLI and lowering shim so the hypothesis can be retested.
- Spend language/runtime effort on higher-value work, especially executable
  pipeline operations.

Those recommendations document the original benchmark outcome. The owner
override supersedes them for the 0.3.0 source surface (direct declarations).
The benchmark report shape and measured evidence remain; the training-only
lowering shim is retired because the product parser accepts direct declarations.
