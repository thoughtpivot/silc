# Silc example apps

Each directory under `examples/` is a **standalone Silc 0.4.0 project** —
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

Tracked examples today: `chatApp`, `inventoryApp`, `scraperApp`,
`pipelineApp`, and `blogApp`.

## Current apps

| App | Purpose | Web | Terminal |
| --- | --- | --- | --- |
| [`chatApp/`](chatApp/) | Multi-session local chat via **silclm** | 18090 | 18091 |
| [`inventoryApp/`](inventoryApp/) | Inventory CRUD + browse/admin + grounded silclm assistant | 18096 | 18097 |
| [`scraperApp/`](scraperApp/) | URL + depth form; site crawl via `scrape::*`; results table | 18110 | 18111 |
| [`pipelineApp/`](pipelineApp/) | One-shot scrape → MiniLM/ONNX → SQLite pipeline | — | — |
| [`blogApp/`](blogApp/) | Seeded blog: home filters + grounded search + admin modal CRUD | 18120 | 18121 |

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

## Build / run

```bash
cargo install --path crates/silc --force   # once, from the compiler repo

cd examples/chatApp
silc build main.silc
silc main.silc   # OpenTUI attaches in a real TTY; web at the app port
```
