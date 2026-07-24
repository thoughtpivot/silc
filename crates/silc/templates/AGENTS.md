# Silc project guidance for AI tools

This directory is a **Silc** project. Silc (said like “silk”) is an AI-native
language and compiler. Edit Silc source; do not hand-edit generated workers.

## Authoritative docs

When unsure, check the public Silc repository:

- Language surface (Raku-inspired subset): https://github.com/thoughtpivot/silc/blob/main/docs/ADR-002-silc-surface-syntax.md
- Architecture & workdir contract: https://github.com/thoughtpivot/silc/blob/main/docs/ARCHITECTURE.md
- Runtime / IPC design: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-001-runtime-and-ipc.md
- Grammar overview: https://github.com/thoughtpivot/silc/blob/main/README.md#part-ii-language-specification--design-of-silc
- Example suite: https://github.com/thoughtpivot/silc/tree/main/examples

## Files

| Path | Role |
| --- | --- |
| `*.silc` | Primary Silc source (preferred) |
| `*.raku` | Accepted only when it conforms to Silc’s grammar |
| `.runtime/` | Compiler output — never edit by hand |
| `.sil` | Not supported — rename to `.silc` |

## Supported surface (not full Raku)

Silc uses a **Raku-inspired** authoring surface. Rakudo is not a Silc runtime.
Allowed constructs today:

- `@version("…")`
- `subset Name of Type [where { … }]`
- `class Name { has Type $.field; }` → Contract
- `class Name is service|processor|sink … { method … }` → Module
- Pipelines with `==>` and `ns::call(:opt(val))`
- Unit literals such as `1500ms`, `:prefer<CUDA>`

Do **not** use full Raku features (junctions, hyperoperators, topicalizers,
`EVAL`, arbitrary OO, synonym forms). Do not use `@domain`.

Typical shape: one contract + three modules (`service` → `processor` → `sink`).

## Routing targets

The compiler routes modules deterministically:

| Signal | Target |
| --- | --- |
| `is service`, namespaces `http` / `html` / `ws` | Bun (TypeScript stubs) |
| `is processor`, namespaces `tensor` / `numpy` / `pandas` | Python |
| `is sink` + low latency, namespaces `store` / `ipc` | Go |

## Workflow

```bash
silc main.silc
# or: chmod +x main.silc && ./main.silc
```

Output lands in `.runtime/<program>/` (stubs + `manifest.json`).

**Current MVP limits:** parse → validate → route → stub emit only. Worker
execution and IPC are not implemented yet.
