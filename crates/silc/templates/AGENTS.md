# Silc project guidance for AI tools

This directory is a **Silc** project. Silc (said like “silk”) is an AI-native
language and compiler. Edit Silc source only. Do not hand-edit `.runtime/` or
`.silc/runtimes.lock.json`.

## Engines are owned by Silc

Silc provisions pinned **Bun**, **CPython**, and **Go** into a global cache
(`~/.silc/runtimes/`). Projects get a compiler-owned lock file at
`.silc/runtimes.lock.json`. Users and AI tools never install, choose, or
configure those engines. There is no runtime configuration surface.

`.runtime/` holds only this app’s generated workers, IPC slots, SQLite data,
UI bundles, and logs — not copies of Bun/CPython/Go.

## Authoritative docs

- Surface syntax: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-002-silc-surface-syntax.md
- Declarative UI: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-003-declarative-ui.md
- Runtime strengths: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-004-runtime-strengths.md
- Architecture: https://github.com/thoughtpivot/silc/blob/main/docs/ARCHITECTURE.md
- Runtime / IPC: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-001-runtime-and-ipc.md
- IPC ABI v1: https://github.com/thoughtpivot/silc/blob/main/docs/SILC-IPC-ABI-v1.md
- Local LLM completions: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-005-local-llm-complete.md
- Examples: https://github.com/thoughtpivot/silc/tree/main/examples

## Workflow

```bash
silc init myapp   # scaffold + provision Silc-owned Bun/CPython/Go + lock file
cd myapp
silc main.silc    # build stubs or run if the program is runnable v1
silc build main.silc   # compile only
```

## Declarative UI (do not write HTML/CSS/React/Tailwind/OpenTUI)

Express UI intent in Silc only:

- `ui::web(:port, :route)` — browser UI using a compiler profile template
- `ui::web(:view(Name), :port, :route)` — browser UI lowered from a named view
- `class Name is view { method render() { … } }` — semantic component tree
- `ui::terminal(:port)` — loopback TCP/telnet UI on Bun

### View catalog (use only these)

`ui::page`, `ui::app_bar`, `ui::side_panel`, `ui::nav_item`, `ui::toolbar`,
`ui::stack`, `ui::row`, `ui::grid`, `ui::card`, `ui::heading`, `ui::text`,
`ui::form`, `ui::text_input`, `ui::textarea`, `ui::radio_group`, `ui::button`,
`ui::chat`, `ui::chat_history`, `ui::search_input`, `ui::filter_bar`,
`ui::product_grid`

`ui::chat(:field(prompt), :label, :placeholder)` renders a conversation thread
plus its send button. `ui::chat_history(:title, :collapsible)` renders persisted
SQLite turns and can collapse to a narrow rail. Both require an `llm::complete`
portal.

An inventory view uses `ui::filter_bar(ui::search_input(...))`,
`ui::product_grid`, and `ui::chat`. The compiler provides seeded read-only
products, deterministic filters, optional AI filter interpretation, and chat
grounded only in the currently visible products. See
`examples/grocery_inventory.silc`.

Props are semantic (`:title`, `:label`, `:field`, `:options([...])`,
`:variant(primary|secondary|destructive)`, `:size(sm|md|lg)`, `:submit`,
`:active`, `:collapsible`). Slots use nested components
(`:app_bar(ui::app_bar(...))`).

`:field(name)` must match a Contract field feeding `ui::web`. Every form view
needs a `ui::button` with `:submit`.

If a requested widget is not in the catalog (charts, dialogs, data tables, …),
**stop and report a Silc compiler limitation** — do not invent React/ShadCN/HTML.

Never emit HTML, CSS, React components, Tailwind configs, ShadCN CLI trees,
OpenTUI trees, Vite config, or `package.json`. React, Tailwind, ShadCN
primitives, and OpenTUI are **implementation substrates**, not authoring APIs.

Legacy aliases (still runnable): `html::form` + `http::serve` → same web profile.

## Declarative HTTP services (do not write Go/Gin)

Express backend API intent in Silc only:

- `service::http(:port, :route, :method)` — Contract-bound HTTP route
  (compiler lowers to Go + Gin)

Example:

```silc
FeedbackRecord
    ==> service::http(:port(18081), :route("/api/feedback"), :method(GET))
```

Never emit Go, Gin routers, or `go.mod`. Those are compiler-owned under
`.runtime/`. UI stays on Bun; backend APIs use Go/Gin.

One `.silc` file can declare a full app (Contract + optional view + service +
processor + sink). Polyglot workers under `.runtime/` are compiler output, not
an authoring model.

## Runnable v1 operations

Executable today:

- `service::http` — API-only (Go/Gin; no processor/sink required)
- UI portal: `ui::web` (optional `:view(Name)` or `html::form` + `http::serve`),
  optional `ui::terminal(:port)`, either `text::score` or `llm::complete`, then
  `ipc::publish`, `store::sqlite`, `store::commit`
- Inventory profile: `ui::web(:view(...))` where the view contains
  `ui::product_grid`, plus `llm::complete` and optional `ui::terminal`

For `llm::complete`, name only a Silc catalog id such as `llama3.2-1b`.
Never add Ollama, model paths, pip packages, or Python inference code to Silc
source; the compiler owns those under `~/.silc/models/` and `.runtime/`.

Other namespaces still parse/route/stub-emit but do not execute yet.

## Supported surface (not full Raku)

Raku-inspired subset only. See ADR-002. Do not use full Raku, `.sil`, or
`@domain`. Typical shape: one contract + optional view + `service` →
`processor` → `sink`.
