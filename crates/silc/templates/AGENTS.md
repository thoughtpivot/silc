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

- `ui::web(:port, :route)` — runnable browser UI (compiler lowers to React +
  Tailwind + ShadCN primitives on Bun)
- `ui::terminal(:port)` — runnable loopback TCP/telnet UI on Bun; OpenTUI remains
  the future rich local-terminal substrate

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

One `.silc` file can declare a full app (Contract + service + processor + sink).
Polyglot workers under `.runtime/` are compiler output, not an authoring model.

## Runnable v1 operations

Executable today:

- `service::http` — API-only (Go/Gin; no processor/sink required)
- Feedback-portal shape: `ui::web` (or `html::form` + `http::serve`), optional
  `ui::terminal(:port)`, `text::score`, `ipc::publish`, `store::sqlite`,
  `store::commit`

Other namespaces still parse/route/stub-emit but do not execute yet.

## Supported surface (not full Raku)

Raku-inspired subset only. See ADR-002. Do not use full Raku, `.sil`, or
`@domain`. Typical shape: one contract + `service` → `processor` → `sink`.
