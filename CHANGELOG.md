# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.
Entries before 0.5.0 are reconstructed from the commit history.

## Unreleased — planned for 0.8.0

### Added

- Embeddings in apps and loops (THO-132, [ADR-010](docs/ADR-010-tensor-minilm-pipeline.md)
  amendment). A processor method `$e.field ==> tensor::tokenize ==> tensor::infer`
  embeds that contract field on every resource write and `loop::write`. A
  CPython embed worker runs the model; the Go loop kernel calls it for loop
  writes. Embeds never call silclm and never count as model calls. Vectors are
  kept out of `loop_events` and recomputed on replay.
- `query nearest;` on resources and `query $.hits = R.nearest($.needle, k);`
  in components: cosine ranking with sqlite-vec, best first, `k` an integer
  literal up to 10000. A blank needle returns no rows.
- Embedding models declare their dimension, and `vector_embedding` must match
  it. `minilm-l6-v2` (384, CPU) stays the default; `nomic-embed-text` (768) and
  `text-embedding-3-small` (1536) are served through an OpenAI-compatible
  `/embeddings` endpoint.

### Changed

- `llm::complete` (and `loop::ask`) may share a program with `tensor::infer`,
  and loops may embed: the LocalModel × Embedding and Loop × Embedding
  compatibility rules are removed. `text::score` × `tensor::infer` and
  `loop` × `scrape::*` stay banned.
- In app and loop programs `tensor::infer` no longer takes the processor slot
  and no longer needs `scrape::page`, `scrape::extract`, one service, or a
  `raw_content` field. Scrape-only programs keep the pipeline behaviour.
- The `:prefer(CUDA)` diagnostic names the compiler's major.minor release
  instead of a hard-coded version.

### Planned

These are the next source changes, tracked in Linear. `loop::fallback`, the
`Contract.new` type check, multi-value `:from` on `loop::ask`, and the THO-560
items below are implemented on `main`; the rest is not yet.

- Implemented: `llama-cpp-python` is pinned at 0.3.38, up from 0.3.7, which no
  longer compiles on current toolchains (`unknown type name 'uint32_t'`).
  Minimum: `llama-cpp-python` 0.3.38. The generated-app calls (`Llama(...)`,
  `create_chat_completion`, `__call__`) are unchanged, and the CI
  `<cstdint>` compiler-flag workaround is removed (THO-560).
- Implemented: model programs build on hosts that have gcc/g++ but no clang.
  The python-build-standalone CPython bakes `CC=clang` into its `sysconfig`,
  so the source build failed with "Silc install of compiler-pinned
  llama-cpp-python failed". Silc now installs a prebuilt wheel when one
  exists (Linux x86_64/aarch64, macOS arm64) and otherwise builds from source
  with an explicit `CC`/`CXX`: yours if set, else clang, else gcc/g++, else
  cc/c++. With no compiler it stops early, names the missing tool, and says
  how to install it. `SILC_LLAMA_CPP_SOURCE=1` forces the source build. CI
  gains a gcc-only job that builds a model example (THO-560).
- `@version` accepts a major.minor match, so a `0.8.x` program builds on any
  `0.8` compiler instead of requiring the exact patch version (THO-558).
- **Implemented:** `loop::fallback`: two or more `loop::read` attempts in
  order, with `:accept` and a required terminating `loop::otherwise`. The first
  attempt whose result satisfies `:accept` (false or unknown does not) binds
  `:as`; each attempt is recorded for replay, and the cost report counts every
  attempt (THO-559 Part 1,
  [ADR-019](docs/ADR-019-vran-language-into-loop.md)).
- Deferred out of 0.8.0: citations on `loop::ask`, where an answer is accepted
  only when each quote occurs in a cited source (THO-559 Part 2), and per-field
  gate approval (THO-556).
- Multi-value `:from` on `loop::ask`, so one question can read several
  bindings (THO-555, on main). `:from($release, $seen)` is the same plan as
  `:from([$release, $seen])`. The model receives one JSON object labelled by
  binding path (`release`, `seen`). That labelling also applies to existing
  `:from([$a, $b])` lists of two or more bindings, which were sent as a bare
  JSON array. A comma list on any other option is a parse error that points to
  the bracket form.
- `loop::write` and every other `Contract.new(...)` in a loop now compare each
  field value with the field's declared type, through `subset` refinements
  such as `Emb384`. A `Str` into an `Int` or vector field, a wrong-dimension
  vector, or a value of unknown type is rejected at validation with a message
  naming the field, the expected type, and the found type. Nothing coerces
  (THO-578).
- Embedding steps in `loop::let` (THO-552), nearest-neighbor `loop::find`
  (THO-553), and page-read `:into` plus `:select` on `loop::read` (THO-551 to
  THO-555).

## 0.7.0 — 2026-11-25

The only source change is that `@version("0.7.0")` is now required, and it
must match the compiler exactly. A `@version("0.6.0")` program is told to
migrate. Examples, `silc init`, and the assist corpus declare the new version.
Everything else in this release is packaging and documentation.

### Changed

- Crates are published to crates.io by release-plz through crates.io Trusted
  Publishing (OIDC) from the `ci.yml` workflow, with no API token. `sil-ide`
  and `sil-lsp` stay unpublished.
- The minimum supported Rust version is 1.86 (was 1.82). The locked `icu` and
  `idna` dependencies need it. Install with `cargo install --locked silc`.
- ADR-015 is amended: the Vran app is internal, closed source, and not paused.
  Its Loop trigger runs continuously and is not Silc's `loop` declaration.
- README, ARCHITECTURE, the training README, and SILC-LANGUAGE section 12 now
  describe 0.7.0 and list what is planned for 0.8.0. The `silc` crate README
  describes Silc as an open-source loop language and compiler.

### Added

- [ADR-019](docs/ADR-019-vran-language-into-loop.md) folds the Vran 0.1 draft
  grammar into `loop::*`. The mapping in
  [docs/vran-0.1-loop-grammar.md](docs/vran-0.1-loop-grammar.md) is confirmed
  by Dan on 24 November 2026. Each construct is Kept, Changed, Planned 0.8.0,
  or Dropped. Planned for 0.8.0: `fallback`, `cite`, and multi-value `from`.

## 0.6.0 — 2026-10-04

First release published to crates.io.

### Added

- `files "<dir>";` on an `app` plus `ui::file_browser`. The synthesized worker
  serves `GET /files/list` and `GET /files/download`: files stream as
  attachments, folders (and `zip=1`) stream a zip from the pinned CPython.
  Paths stay inside the share. Terminal lists URLs instead of transferring
  bytes ([ADR-018](docs/ADR-018-files-capability.md)).
- Synthesized MCP server on `POST /mcp` for programs with an `app` (or a loop
  web surface). Tools come from resource queries/mutations and schedule/manual
  loops (`talks_list`, `talk_today_run`, …). Auth is `SILC_MCP_TOKEN`. The plan
  hash covers the tool list; `silc build` prints it.
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
