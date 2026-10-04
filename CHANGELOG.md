# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.
Entries before 0.5.0 are reconstructed from the commit history.

## Unreleased

### Changed

- Silc 0.6.0. `@version("0.5.0")` sources are rejected. When a 0.5.0 program still writes a kernel node as `game::`, the diagnostic lists exactly those renames (`game::mesh` → `scene::mesh`).
- The real-time root is `scene`. Kernel nodes are `scene::`; gameplay nodes stay `game::`. `game Name` remains a one-release alias of `scene Name` (ADR-016).
- `task` is removed. The compiler routes by operation.
- `{$binding.path}` placeholders use one checker in `loop::` and `ui::` string options.
- README title is generic. VDC is a domain document ([docs/domains/vdc.md](docs/domains/vdc.md)), not the language identity (ADR-015, ADR-016).
- Examples live under `examples/core/` and `examples/domains/aec/`. The three Moz-specific loop apps are one generic MCP loop example.

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
  (`$.loop`, `scene::audio(:kind(loop))`) it is still an identifier.
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

- `ui::embed` — dual-surface URL viewport (`:src` required, `:title?`). Web
  lowers to a sandboxed iframe; terminal lowers to a title/URL/“Open in a
  browser” card (THO-119, [ADR-017](docs/ADR-017-ui-embed.md)).
- `examples/core/embedLoopApp`: one program with `ui::embed`, blank-Str `loop::ask` (“no more items”), and a live loop kernel on `app.db` (THO-119/120/121).
- [docs/SILC-LANGUAGE.md](docs/SILC-LANGUAGE.md): the normative language
  surface, including the list of known irregularities scheduled for 0.6.0.
- [docs/GLOSSARY.md](docs/GLOSSARY.md): one meaning per term and the retired
  vocabulary.
- `examples/core/hotelSignupApp` README and index rows.
- `mcp::call` is a registered operation. `loop::read` accepts a nested
  operation (`loop::read(:as(x), scrape::page(:url(...)))`); the string
  `:op("scrape::page")` remains valid for one release.
- `silc docs` prints the generated catalog sections.

### Fixed

- THO-120: `loop::ask` contract checking accepts blank and whitespace-only strings as valid `Str`. Non-string JSON values still fail, and the error names the actual JSON type (number, object, array, bool, null).
- THO-121: exiting `silc` tears down the worker tree (Go loop kernel, Bun, CPython)
  so children are not reparented to pid 1. Workers join a dedicated process group and
  receive `PR_SET_PDEATHSIG` on Linux. A second loop kernel refuses an `app.db` that
  already has a live kernel via an exclusive flock on `app.db.kernel.lock`; stale
  locks from dead processes do not block restart.
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
  generation (`scene::generate`), FPS and platformer kernels — ADR-012, ADR-013.
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
