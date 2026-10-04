# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.
Entries before 0.5.0 are reconstructed from the commit history.

## Unreleased

### Changed

- Author-facing vocabulary uses full words: **operation** (was "op") and
  **option** (was "prop" / "adverb" / "colon-pair"). Compiler diagnostics,
  editor hovers, the AGENTS template, and the catalog formatters now say
  `option`. See [docs/GLOSSARY.md](docs/GLOSSARY.md).
- The `game::*` closed-enum paragraph in AGENTS.md is generated from
  `GAME_NODE_CATALOG` (`format_game_closed_enums_line`); the hand-written
  version had drifted (missing `collider` shapes, `particle_effect` presets,
  `generate` enums, and all platformer values).
- Hard-coded catalog counts were removed from prose; counts are pinned only in
  `docs_conformance.rs`.
- Editor grammar scopes `class` and `sink` as `invalid.deprecated`; `loop` is a
  hover keyword and a lexer keyword. Inside an expression or a field name
  (`$.loop`, `game::audio(:kind(loop))`) it is still an identifier.
- `ui::`, `game::`, and `loop::` catalogs share one `NodeSpec`. Closed-enum
  values are bare identifiers; a quoted string is a compile error with a fix-it.
- Executable operations live in `OPERATION_CATALOG`. The router reads each
  scrape operation's engine from that catalog. "Cannot mix" rules live in
  `COMPATIBILITY`.
- `ProcessorOp` names use the operation spelling (`text::score`, `llm::complete`,
  `tensor::infer`).
- Author `sink` modules and `app.serve` are gone from the model. Component
  methods cannot be pipelines.

### Added

- [docs/SILC-LANGUAGE.md](docs/SILC-LANGUAGE.md): the normative language
  surface, including the list of known irregularities scheduled for 0.6.0.
- [docs/GLOSSARY.md](docs/GLOSSARY.md): one meaning per term and the retired
  vocabulary.
- `examples/hotelSignupApp` README and index rows.
- `mcp::call` is a registered operation. `loop::read` accepts a nested
  operation (`loop::read(:as(x), scrape::page(:url(...)))`); the string
  `:op("scrape::page")` remains valid for one release.
- `silc docs` prints the generated catalog sections.

### Fixed

- ADR-002 / ADR-009 no longer state `@version("0.4.0")` as the required pragma.
- ARCHITECTURE.md lists the actual `sil-core` modules, the `Game` and `Loop`
  subjects, and the `sil-ide` / `sil-lsp` crates.
- README documentation table links ADR-013 and this changelog.
- Hover text for `class` and `sink` says they are rejected rather than
  "retained" or "available for specialized write paths".

## 0.5.0 — 2026-10-02

- `loop` declaration: scheduled, approval-gated, model-assisted work with a
  closed `loop::*` catalog, Go loop kernel, receipts and replay, MCP reads
  (`loop::read(:op("mcp::call"))`), a synthesized `/loops` inbox, and
  loop commands (`silc main.silc` runs once and exits) — ADR-014.
- Examples: `rfiChaseApp`, `oneThingApp`, `oneThingCliApp`, `whatToDoTodayApp`.
- README leads with VDC while keeping the language generic.

## 0.4.x — 2026-07-27 to 2026-08-24

- Compiler-synthesized runtime: author `sink`, `method serve()`, `ui::web` /
  `ui::terminal`, and `ipc` / `store` / `resource::*` pipelines removed from
  the surface — ADR-009.
- `scrape::*` namespace (ADR-006), `doc::extract` (ADR-011), MiniLM tensor
  pipeline and `silc run --input-json` (ADR-010).
- `game` declaration with the WebGPU `game::*` catalog, procedural asset
  generation (`game::generate`), FPS and platformer kernels — ADR-012, ADR-013.
- `silc assist` recursive authoring with silclm (ADR-008); silclm upgraded to
  Llama 3.2 3B.
- `sil-lsp` language server and the VS Code / Cursor extension.
- README recast as the Silc white paper; Silc declared an independent intent
  language (ADR-002).

## 0.3.0 — 2026-07-26

- Direct declarations (`contract`, `component`, `resource`, `app`, `service`,
  `processor`, `task`) replace `class … is …` (owner override of the
  subject-first benchmark).

## 0.2.0 — 2026-07-25

- Component model: `component`, `resource … for …`, `app` routes; dual-surface
  web + terminal from one tree; generic resource CRUD over SQLite.
- Portal profiles, `PortalKind`, and `is view` removed.

## 0.1.0 — 2026-07-25

- Parse → route → emit pipeline, `silc init`, pinned Bun / CPython / Go
  runtimes, shared-memory IPC ABI v1, first portal and API examples.
