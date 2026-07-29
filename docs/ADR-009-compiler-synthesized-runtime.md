# ADR-009: Compiler-Synthesized Runtime (Silc 0.4.0)

- **Status:** Accepted
- **Date:** 2026-07-27
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-002](ADR-002-silc-surface-syntax.md),
  [ADR-003](ADR-003-declarative-ui.md),
  [ADR-005](ADR-005-local-llm-complete.md),
  [ADR-007](ADR-007-pipeline-feeds.md),
  [ADR-010](ADR-010-tensor-minilm-pipeline.md),
  [ADR-012](ADR-012-webgpu-game-subject.md),
  [intent-vs-subjects.md](intent-vs-subjects.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)
- **Supersedes (partial):** Author-facing `method serve()`, `ui::web` /
  `ui::terminal` declarations, author `sink` modules, and author
  `ipc::*` / `store::*` / `resource::*` pipelines described in ADR-002,
  ADR-003, ADR-005, and ADR-007 (0.2.0–0.3.0 examples).
- **Canonical:** [`EXECUTABLE_OPS`](../crates/sil-core/src/operation.rs),
  [`sil-parser`](../crates/sil-parser/src/lib.rs) diagnostics,
  [`resource` capability synthesis](../crates/sil-core/src/resource.rs),
  codegen templates under `crates/sil-codegen/templates/`

## Context

Silc 0.2.0–0.3.0 required authors (and agents) to wire runtime mechanics in
`.silc`: dual-surface `serve()` chains, SQLite `sink` modules, and
`resource::*` / `ipc::*` / `store::*` pipelines. That duplicated compiler-owned
work, invited invalid substrate invention, and blurred the intent language with
implementation.

Release 0.4.0 makes the product rule explicit: **authors declare intent; the
compiler synthesizes runtime mechanics.**

## Decision

### Author-facing surface

Authors may declare:

| Construct | Intent |
| --- | --- |
| `@version("0.4.0")` | Exact source-version match with the compiler |
| `contract` / `subset` | Domain schemas |
| `component` | UI units (`render()`, state, events) |
| `resource Name for Contract { query …; mutation …; seed …; }` | Capability CRUD (no method bodies); optional idempotent seeds |
| `app { route … }` | Route table only |
| `service` / `processor` / `task` | Optional workflows |
| Author `EXECUTABLE_OPS` | `service::http`, `text::score`, `llm::complete`, `scrape::*`, `doc::extract`, `tensor::*` |
| `==>` | Domain pipeline feeds between values and author ops |

### Forbidden in author source (compiler-owned)

- `method serve()`
- `ui::web` / `ui::terminal` as program operations
- `sink` modules
- `ipc::*`, `store::*`, `resource::*` pipelines
- Engine names, package manifests, HTML/CSS/React/OpenTUI trees

### What the compiler synthesizes

| From author intent | Synthesized runtime |
| --- | --- |
| `app` with routes | Dual-surface web (React/Tailwind) + terminal (OpenTUI); default ports 18088 / 18023; override via `SILC_HTTP_PORT` / `SILC_TERMINAL_PORT` |
| `resource Name for Contract` capabilities (+ optional `seed`) | HTTP CRUD + SQLite table wiring + idempotent seed inserts |
| Processor + Contract (`text::score`, `llm::complete`, `tensor::infer`) | Go/SQLite sink and IPC/store staging |
| Pipeline-only graph | Ingress via `silc run --input-json` / `--input`; larger mmap payload slots when needed |

Dual-surface **parity remains required** as a product outcome: every UI app
gets both surfaces. Authors no longer declare the surfaces in source.

**Exception (ADR-012):** `game` programs synthesize a **web-only** WebGPU
browser surface (no OpenTUI). Dual-surface parity does not apply to `game`; do
not declare `app`, `component`, or UI resources alongside `game` in v1. Game
programs still synthesize Bun + CPython bake + Go persistence (ADR-001/004).

### Example (0.4.0)

```silc
@version("0.4.0")

contract Note { has Str $.text; }

component HomePage {
    has state Str $.text = "";
    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Notes"))),
            ui::form(:on(submit(on_submit)),
                ui::textarea(:field(text), :label("Note")),
                ui::button(:label("Submit"), :variant(primary), :submit)
            )
        )
    }
    method on_submit() { submit(); }
}

resource Notes for Note {
    query list;
    mutation create;
}

app NotesApp {
    route "/" => HomePage;
}
```

Catalog primitives such as `ui::page` remain template vocabulary inside
`render()`; they are not runnable program operations.

## Consequences

### Positive

- Shorter, intent-dense programs; agents cannot invent sink/IPC glue.
- Dual-surface and persistence invariants are enforced once in the compiler.
- ADRs and AGENTS.md can point at registries instead of re-teaching wiring.

### Costs

- Pre-0.4.0 examples and ADRs that show `serve()` / author sinks are historical.
- Diagnostics must clearly reject removed constructs with migration guidance.

## Non-goals

- User-selectable substrate overrides
- Author-defined sink or IPC modules
- Web-only or terminal-only UI apps
- Restoring `resource::*` as author-facing ops

**ADR-012 exception:** `game` programs are intentionally web-only for the
player-facing surface (WebGPU, no terminal chrome). That exemption applies to
the `game` subject only — not to `app` UI programs — and does not exempt games
from polyglot Bun/CPython/Go synthesis.

## Historical note

0.2.0–0.3.0 Decision text in ADR-002, ADR-003, ADR-005, and ADR-007 that
describes author-declared surfaces, sinks, or resource pipelines remains as
history. Current authoring rules are this ADR plus the AGENTS.md contract.
