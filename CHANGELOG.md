# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.

## [0.4.0] - 2026-07-27

### Breaking

- Reframed Silc as an intent-oriented, declaration-based authoring surface.
  Compiler subjects remain an internal `sil-core` architecture, not product
  identity.
- Removed author `method serve()`, `ui::web`, `ui::terminal`, `sink`,
  `ipc::*`, `store::*`, and `resource::*` pipelines from `.silc` source.
  Dual-surface serving and SQLite persistence are synthesized by the compiler.
- Resources require `resource Name for Contract` with capability declarations
  (`query list;`, `mutation create;`, …).
- Require exact `@version("0.4.0")` declarations in compiled source.

### Changed

- Pin silclm v0 to Llama 3.2 3B Instruct Q4_K_M (~2.02 GB) instead of 1B.
  Legacy alias `llama3.2-1b` still resolves to `silclm`; existing 1B cache
  artifacts are not reused as 3B weights.

### Added

- Implicit dual-surface UI from `app` routes (default ports; override via
  `SILC_HTTP_PORT` / `SILC_TERMINAL_PORT`).
- Auto-synthesized Go/SQLite sink for processor programs
  (`text::score`, `llm::complete`, `tensor::infer`).
- Closed MiniLM embedding pipeline (`tensor::tokenize` / `tensor::infer`,
  `examples/pipelineApp`).
- Experimental `silc assist "<task>"` closed-tool recursive authoring scaffold
  (`sil-rlm`, ADR-008): explores embedded AGENTS/examples/fixtures, validates
  with `check_source`, depth-1 `llm_query` via silclm. Fine-tuned
  `silclm-assist` weights are not shipped yet.
- Decision records for the 0.4.0 surface:
  [ADR-009](docs/ADR-009-compiler-synthesized-runtime.md) (synthesized runtime)
  and [ADR-010](docs/ADR-010-tensor-minilm-pipeline.md) (tensor / MiniLM);
  plus [ADR-INDEX.md](docs/ADR-INDEX.md).

## [0.3.0] - 2026-07-26

### Breaking

- Established Silc as an independent `.silc`-only intent language; `.raku` and
  `.sil` source inputs are rejected.
- Require exact `@version("0.3.0")` declarations in compiled source.
- Replaced legacy `class Name is kind` declarations with direct
  `contract`, `component`, `resource`, `app`, `service`, `processor`, `sink`,
  and `task` declarations. This is an explicit owner override of the benchmark
  no-go, whose evidence remains documented.

## [0.2.0] - 2026-07-25

### Breaking

- Removed profile-selected applications (`PortalKind` Feedback / LlmChat / Inventory).
- Removed `is view` and Contract-left-of-`ui::web` portal binding.
- Removed the separate repository-root component-source `stdlib/` directory and
  resolver. Out-of-box UI capability is compiler-owned in the primitive catalog
  and codegen templates; author components remain in application source.
- Replaced profile worker templates with unified Bun / Python / Go workers.
- Replaced the previous example suite with dual-surface component-driven apps.

### Added

- Author-defined `is component` with typed props, reactive state, slots, emits,
  handlers, queries, and render templates.
- `is resource` query/mutation methods backed by Contracts and SQLite.
- `is app` routes plus required dual-surface `serve()` (`ui::web` + `ui::terminal`).
- Expression language for props, conditionals, collection rendering, and handlers.
- Manifest v3 capabilities / actions / surfaces / processor metadata.
- Semantic release automation via Conventional Commits and `release-plz`.
- Expanded compiler-owned UI primitives and dual-surface codegen templates.

### Examples

- `examples/components.silc`
- `examples/scored_form.silc`
- `examples/chat_assistant.silc`
- `examples/shopping_app.silc`
- `examples/http_api.silc`
- `examples/data_pipeline.silc`
