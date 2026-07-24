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
and logs — not copies of Bun/CPython/Go.

## Authoritative docs

- Surface syntax: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-002-silc-surface-syntax.md
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

## Runnable v1 operations

Executable today (feedback-portal shape):

- `html::form`, `http::serve`
- `text::score`
- `ipc::publish`, `store::sqlite`, `store::commit`

Other namespaces still parse/route/stub-emit but do not execute yet.

## Supported surface (not full Raku)

Raku-inspired subset only. See ADR-002. Do not use full Raku, `.sil`, or
`@domain`. Typical shape: one contract + `service` → `processor` → `sink`.
