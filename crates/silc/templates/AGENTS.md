# Silc project guidance for AI tools

This directory is a **Silc** project. Silc (said like “silk”) is an AI-native
language and compiler. Edit Silc source only. Do not hand-edit `.runtime/` or
`.silc/runtimes.lock.json`.

## Engines are owned by Silc

Silc provisions pinned **Bun**, **CPython**, and **Go** into a global cache
(`~/.silc/runtimes/`). Projects get a compiler-owned lock file at
`.silc/runtimes.lock.json`. Users and AI tools never install, choose, or
configure those engines.

`.runtime/` holds only this app’s generated workers, IPC slots, SQLite data,
UI bundles, and logs — not copies of Bun/CPython/Go.

## Authoritative docs

- Surface syntax: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-002-silc-surface-syntax.md
- Declarative UI: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-003-declarative-ui.md
- Architecture: https://github.com/thoughtpivot/silc/blob/main/docs/ARCHITECTURE.md
- Examples: https://github.com/thoughtpivot/silc/tree/main/examples

Out-of-box components are compiler-owned catalog primitives and codegen
templates. Author-defined components belong in app source; do not create or
resolve a separate component-source standard library.

## Workflow

```bash
silc init myapp
cd myapp
silc build main.silc
silc examples/scored_form.silc   # from the Silc repo
```

## Silc 0.2.0 authoring model

- `class X { … }` — Contract (data schema)
- `class X is component` — reusable UI unit (props, `has state`, slots, emit, render)
- `class X is resource` — query/mutation data layer
- `class X is app` — routes + `method serve()` with **both** `ui::web` and `ui::terminal`
- `class X is service|processor|sink` — optional backend modules

`is view` and portal profiles (Feedback / LlmChat / Inventory) were removed.
High-level chat/form/catalog UIs are compositions of components and resources,
not compiler modes.

## Declarative UI (do not write HTML/CSS/React/Tailwind/OpenTUI)

Express UI intent in Silc only. The compiler lowers the same component graph to
**web (React)** and **terminal** adapters. Never emit HTML, CSS, React, Tailwind,
OpenTUI trees, Vite config, or `package.json`.

## Runnable 0.2.0 operations

`ui::web`, `ui::terminal`, `service::http`, `text::score`, `llm::complete`,
`ipc::publish`, `store::sqlite`, `store::commit`, `resource::list|get|create|update|delete`.

Mixing stub-only ops into a runnable graph is a compile error.
