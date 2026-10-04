# Silc example apps

Each example is a **standalone Silc 0.6.0 project** — the same shape `silc init` creates for end users.

Core programs live in `examples/core/`. Domain programs live in `examples/domains/<domain>/`. Architecture, engineering, and construction examples are under `examples/domains/aec/`.

## Layout

```text
examples/core/<appName>/
examples/domains/aec/<appName>/
  main.silc      # authored program (only .silc source that matters)
  AGENTS.md      # compiler AGENTS template + app-specific notes
  README.md      # how to build/run this app
  .gitignore     # ignores .runtime/ and .silc/
  .runtime/      # compiler-owned (never commit, never hand-edit)
  .silc/         # runtime lock (never commit, never hand-edit)
```

## AGENTS.md sync rule

The shared block between `<!-- BEGIN SILC_AGENTS_TEMPLATE -->` and
`<!-- END SILC_AGENTS_TEMPLATE -->` **must** match
[`crates/silc/templates/AGENTS.md`](../crates/silc/templates/AGENTS.md)
byte-for-byte. App-specific notes go **after** the end marker only.

Every example directory is tracked; the conformance test in
`crates/silc/tests/docs_conformance.rs` checks each one's `AGENTS.md` block
and `@version`, so a new example needs no registration beyond its files and a
row in the table below.

## Core

| App | Purpose | Web | Terminal |
| --- | --- | --- | --- |
| [`chatApp/`](core/chatApp/) | Multi-session local chat via **silclm** | 18090 | 18091 |
| [`inventoryApp/`](core/inventoryApp/) | Inventory CRUD + browse/admin + grounded silclm assistant | 18096 | 18097 |
| [`hotelSignupApp/`](core/hotelSignupApp/) | Two-route sign-up form + ledger; direct `Guests.create(...)` handler style | 18088 | 18023 |
| [`scraperApp/`](core/scraperApp/) | URL form; site crawl at depth 2 via `scrape::*`; results table | 18110 | 18111 |
| [`pipelineApp/`](core/pipelineApp/) | One-shot scrape → MiniLM/ONNX → SQLite pipeline | — | — |
| [`blogApp/`](core/blogApp/) | Seeded blog: home filters + grounded search + admin modal CRUD | 18120 | 18121 |
| [`dataExtractorApp/`](core/dataExtractorApp/) | File upload + `doc::extract` → documents ledger | 18130 | 18131 |
| [`arenaGameApp/`](core/arenaGameApp/) | WebGPU scene kernel plus a gameplay layer (Babylon adapter) | 18140 | — |
| [`platformGameApp/`](core/platformGameApp/) | WebGPU platformer (sprites, collectibles, patrols, level end) | 18140 | — |
| [`mcpLoopApp/`](core/mcpLoopApp/) | Generic MCP loop command: one `mcp::call`, a silclm brief, one keyed note. README also shows the inbox-only and app modes | — | — |
| [`embedLoopApp/`](core/embedLoopApp/) | `ui::embed` + blank-Str `loop::ask` + live loop kernel on `app.db` (THO-119/120/121) | 18150 | 18151 |

## Domains

### AEC

| App | Purpose | Web | Terminal |
| --- | --- | --- | --- |
| [`rfiChaseApp/`](domains/aec/rfiChaseApp/) | Weekday `loop`: overdue RFIs → silclm draft → PM approval in `/loops` → keyed reminder | 18088 | — |
| [`vdcWalkthrough/`](domains/aec/vdcWalkthrough/) | Project-environment scene: `scene::` kernel, `game::pawn` viewer | 18140 | — |

## Conventions

1. Author only `.silc` (and project docs). Never patch `.runtime/`.
2. Every UI `app` synthesizes **both** web and terminal surfaces (OpenTUI primary;
   TCP telnet fallback). Do not write `method serve()`, `ui::web`, or
   `ui::terminal` in source ([ADR-009](../docs/ADR-009-compiler-synthesized-runtime.md)).
   Examples may set `SILC_HTTP_PORT` / `SILC_TERMINAL_PORT` for the ports above;
   defaults are 18088 / 18023.
3. Prefer the default model: call `llm::complete()` with no `:model` (resolves to **silclm**).
4. Chat that must reason over live data uses `ui::chat(:context($.items), …)`; give the assistant an identity with `:persona("You are …, built on silclm.")`.
5. Rebuild with the current `silc` after compiler upgrades — generated workers refresh automatically.
6. Future training corpora will come from these apps; they are not a dataset yet.
7. Pipeline-only programs (`pipelineApp`) run with
   `silc run main.silc --input-json '{"url":"…"}'`
   ([ADR-010](../docs/ADR-010-tensor-minilm-pipeline.md)).
8. Scene programs declare `scene Name { scene::scene(...) }`.
   `game Name` is a one-release alias of that root.
   Gameplay nodes stay `game::`. Web/WebGPU only, no terminal
   ([ADR-012](../docs/ADR-012-webgpu-game-subject.md),
   [ADR-016](../docs/ADR-016-generic-kernel-domain-layers.md)).
9. Loop commands (`mcpLoopApp`) have no `app` and only `loop::manual`
   triggers. `silc main.silc` runs each loop once, narrates progress on stderr,
   prints the notices on stdout, and exits; nothing is served
   ([ADR-014](../docs/ADR-014-loop-subject.md)).

## Build / run

```bash
cargo install --path crates/silc --force   # once, from the compiler repo

cd examples/core/chatApp
silc build main.silc
silc main.silc              # web by default
silc main.silc --terminal   # also attach OpenTUI (+ telnet fallback)

cd ../mcpLoopApp
silc main.silc              # a loop command: runs once, prints the result, exits
```
