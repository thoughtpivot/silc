# Silc example apps

Each directory under `examples/` is a **standalone Silc 0.5.0 project** —
the same shape `silc init` creates for end users.

## Layout

```text
examples/<appName>/
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

Every directory under `examples/` is tracked; the conformance test in
`crates/silc/tests/docs_conformance.rs` checks each one's `AGENTS.md` block
and `@version`, so a new example needs no registration beyond its files and a
row in the table below.

## Current apps

| App | Purpose | Web | Terminal |
| --- | --- | --- | --- |
| [`chatApp/`](chatApp/) | Multi-session local chat via **silclm** | 18090 | 18091 |
| [`inventoryApp/`](inventoryApp/) | Inventory CRUD + browse/admin + grounded silclm assistant | 18096 | 18097 |
| [`hotelSignupApp/`](hotelSignupApp/) | Two-route sign-up form + ledger; direct `Guests.create(...)` handler style | 18088 | 18023 |
| [`scraperApp/`](scraperApp/) | URL + depth form; site crawl via `scrape::*`; results table | 18110 | 18111 |
| [`pipelineApp/`](pipelineApp/) | One-shot scrape → MiniLM/ONNX → SQLite pipeline | — | — |
| [`blogApp/`](blogApp/) | Seeded blog: home filters + grounded search + admin modal CRUD | 18120 | 18121 |
| [`dataExtractorApp/`](dataExtractorApp/) | File upload + `doc::extract` → documents ledger | 18130 | 18131 |
| [`arenaGameApp/`](arenaGameApp/) | WebGPU game kernel (Godot/Unity/Unreal synthesis on Babylon) | 18140 | — |
| [`platformGameApp/`](platformGameApp/) | WebGPU platformer (side-scroll camera, arrows+jump controls) | 18140 | — |
| [`rfiChaseApp/`](rfiChaseApp/) | Weekday `loop`: overdue RFIs → silclm draft → PM approval in `/loops` → keyed reminder | 18088 | — |
| [`oneThingApp/`](oneThingApp/) | Daily `loop`: four Moz MCP reads → silclm brief → Dan's one sentence (needs `MOZ_MCP_TOKEN`) | 18088 | — |
| [`oneThingCliApp/`](oneThingCliApp/) | Loop **command**: the same reads and brief, run once by `silc main.silc`; the sentence is printed to stdout and the process exits. No UI, no `/loops` (needs `MOZ_MCP_TOKEN`) | — | — |
| [`whatToDoTodayApp/`](whatToDoTodayApp/) | Scheduled daily `loop` with no authored UI: same Moz MCP reads → silclm brief → 3 to 5 item to-do list; the synthesized `/loops` inbox (web, OpenTUI, or `/api`) is the whole interface (needs `MOZ_MCP_TOKEN`) | 18088 | 18023 |

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
8. Game programs declare `game { game::scene(...) }` only —
   web/WebGPU surface, no terminal ([ADR-012](../docs/ADR-012-webgpu-game-subject.md)).
9. Loop commands (`oneThingCliApp`) have no `app` and only `loop::manual`
   triggers. `silc main.silc` runs each loop once, narrates progress on stderr,
   prints the notices on stdout, and exits; nothing is served
   ([ADR-014](../docs/ADR-014-loop-subject.md)).

## Build / run

```bash
cargo install --path crates/silc --force   # once, from the compiler repo

cd examples/chatApp
silc build main.silc
silc main.silc              # web by default
silc main.silc --terminal   # also attach OpenTUI (+ telnet fallback)

cd ../oneThingCliApp
silc main.silc              # a loop command: runs once, prints the result, exits
```
