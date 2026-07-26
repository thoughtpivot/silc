# Subject-first declarators — evaluation only

Silc 0.2.0 keeps `class X is resource|component|app|…`. A subject-first surface
(`resource X`, `component X`, `app X`, `contract X`, …) may reduce false OO
implications and tokens, but it is a breaking change.

**This workstream does not migrate syntax.** It only provides a reproducible
benchmark so a later 0.x decision can be evidence-based.

## Harness

```bash
cargo run -p sil-training -- subject-first-bench \
  --agents crates/silc/templates/AGENTS.md \
  --tasks training/tasks \
  --out training/out/subject_first_bench.json
```

The report includes:

- Paired prompts (`class-is` vs `subject-first` guidance) for each task seed
- Baseline trials: current fixtures must compile; subject-first sketches are
  recorded as expected compile failures until the grammar exists
- Token estimates, first-pass success, repair turns (when agent trials are scored)

Score live agent runs by appending `TrialInput` rows (see
`sil_training::subject_first`) and re-summarizing.

## Go / no-go

Migrate only when:

1. Subject-first first-pass `silc build` success beats class-is by **≥10 absolute
   percentage points** on the same task set.
2. Mean repair turns to green do **not** increase.
3. Mean completion tokens do not rise by **>15%** unless first-pass gain exceeds
   15 points.
4. Both variants have **≥20 scored trials** per task family.

Until those numbers exist, keep `class … is …`.
