# ThoughtPivot Silc Architecture

This repository uses **subject-based architecture** for the compiler's semantic
core. Code is organized around durable language concepts instead of allowing
compiler phases to become the owners of the model.

The first runnable vertical slice implements subjects, lexing, parsing,
deterministic routing, runnable Bun/Python/Go codegen, supervisor-owned mmap +
UDS IPC, and transactional SQLite persistence for the feedback portal. Other
operation sets still emit inspectable stubs.

Runtime and IPC direction is fixed by
[ADR-001](ADR-001-runtime-and-ipc.md): Silc emits TypeScript for Bun and uses a
ThoughtPivot-owned shared-memory ABI rather than requiring Apache Arrow.
Engine strength catalogs that justify Go / CPython / Bun routing are in
[ADR-004](ADR-004-runtime-strengths.md).

Surface syntax is Raku-inspired
([ADR-002](ADR-002-silc-surface-syntax.md)): `.silc` is primary and conforming
`.raku` files are accepted. `.sil` is not supported.

## The subject boundary

A Silc subject is a durable semantic concept that has shared types, owns
invariants, and participates in several workflows. The initial subjects are:

| Subject | Owns |
| --- | --- |
| `Contract` | Schemas, fields, annotations, type compatibility, and memory-layout invariants |
| `Module` | Services, processors, sinks, tasks, properties, and functions |
| `Constraint` | Typed execution limits, preferences, normalization, and validation |
| `Pipeline` | Ordered intent steps, value flow, step compatibility, and references |
| `Target` | Runtime capabilities and the resolved Go/Python/Bun assignment (Bun executes emitted TypeScript) |

These subjects live together in `sil-core`, with one Rust module per subject:

```text
crates/sil-core/src/
├── contract.rs
├── module.rs
├── constraint.rs
├── pipeline.rs
├── target.rs
└── lib.rs
```

This is the compiler equivalent of placing a subject's shared types and
behavior at the subject level. As each subject grows, related submodules may be
nested beneath it, but canonical types and invariants stay at the subject root.

## Subjects versus boundary services

Not every named compiler concept is a subject. A phase is a boundary service
when it transforms or coordinates subjects without owning their definitions:

| Boundary | Responsibility |
| --- | --- |
| `sil-lexer` | Convert source text into syntax tokens |
| `sil-parser` | Construct `sil-core` subjects from syntax |
| `sil-router` | Resolve a module's `Target` from subject data |
| `sil-codegen` | Project validated subjects into target source |
| `sil-ipc` | Implement the cross-runtime transport boundary |
| `silc` | Keep CLI composition thin and orchestrate the workflow |

This distinction prevents phase-owned duplicate models such as parser
contracts, router contracts, and codegen contracts. Boundary crates depend
inward on `sil-core`; `sil-core` does not depend on them.

```text
Silc source  (workdir/myprogram.silc)
    │
    ▼
sil-lexer ──► sil-parser
                  │ constructs
                  ▼
             sil-core subjects
             ├── Contract
             ├── Module
             ├── Constraint
             ├── Pipeline
             └── Target
                  │
          validates and exposes
                  ▼
             sil-router
                  │ resolves Target
                  ▼
             sil-codegen
                  │
                  ▼
        {workdir}/.runtime/
          ├── go/
          ├── python/
          └── typescript/  # executed by Bun
                  │
                  ▼
               sil-ipc (+ supervisor boot)
```

## Ownership rules

1. **Canonical types have one subject owner.** `Contract` field types belong to
   the Contract subject, not copies in parser, router, or emitter crates.
2. **Invariants live with the subject.** Contract layout checks, constraint
   normalization, and pipeline compatibility are semantic behavior, not CLI or
   codegen behavior.
3. **Boundaries translate; subjects decide.** The parser translates syntax,
   codegen translates semantics, and IPC translates transport. Semantic
   validity is decided by `sil-core`.
4. **Cross-subject access is explicit.** Rust paths should name the owner, for
   example `sil_core::contract::Contract` and
   `sil_core::pipeline::Pipeline`.
5. **Sub-workflows remain under their subject.** If Contract gains Silc buffer layout
   lowering or Pipeline gains graph analysis, those modules begin beneath
   their subject rather than as detached top-level utility crates.
6. **Shared means truly domain-neutral.** Source spans, diagnostics, and stable
   IDs may become common primitives. A folder named `utils` must not become an
   ownership escape hatch.
7. **No cyclic subject ownership.** Relationships use IDs/references or
   explicit coordinating services; subjects do not hide bidirectional global
   state.

## Applying the model to routing, codegen, and IPC

### Semantic router

Routing is an application service, while `Target` is the durable subject. The
router reads `Module`, `Constraint`, and `Pipeline`, then records a
`Target` decision with provenance:

1. module kind traits and hard constraints;
2. namespace evidence from pipeline steps;
3. deterministic fallback.

Routing policy stays in `sil-router`; target identity, capabilities, and
resolved assignment types stay in `sil-core::target`. Provenance strings cite
the strength catalog in [ADR-004](ADR-004-runtime-strengths.md).

### Code generation

Generators are adapters grouped by target (`go`, `python`, `typescript`).
The TypeScript adapter emits source for Bun, which is the runtime engine.
They consume one validated semantic model and must not define parallel AST
types. Reusable lowering that expresses Silc meaning belongs to the owning
subject; target-specific rendering belongs to `sil-codegen`.

### Declarative UI

UI intent lives in the `ui` namespace (`ui::web`, `ui::terminal`) and optional
`class … is view` subjects. Authors never emit HTML, CSS, React, Tailwind,
ShadCN, OpenTUI, or package manifests. Named views describe a typed semantic
component tree (`ui::page`, `ui::app_bar`, `ui::side_panel`, form controls,
toolbars, …); `ui::web(:view(Name), …)` binds that tree to a Contract. When
`:view` is omitted, the compiler keeps profile templates (feedback / LLM chat).
Codegen lowers either path to a Bun-owned React + Tailwind + ShadCN-primitives
app plus HTTP/API worker under `.runtime/`. `ui::terminal` adds a Bun-owned
loopback TCP/telnet interface; a future rich local-terminal adapter uses
OpenTUI. See [ADR-003-declarative-ui.md](ADR-003-declarative-ui.md). Legacy
`html::form` + `http::serve` remain compatibility aliases for the same web
profile.

### IPC

IPC is both a runtime boundary and a significant technical subsystem. Contract
owns logical schema and layout requirements. `sil-codegen` lowers validated
Contracts into generated accessors. `sil-ipc` owns the versioned Silc Shared
Buffer ABI, mmap/shared-memory allocation, process-safe handles, lifecycle
rules, and UDS signaling. Large payloads remain mapped while small control
frames identify `{ segment_id, offset, len, schema_id }`. This avoids letting
transport details leak into every semantic subject while preserving Contract as
the source of truth.

Apache Arrow is not required by the runtime. A future export adapter may expose
Silc buffers as Arrow for external analytical tools.

## Crate map

| Crate | Role |
| --- | --- |
| `silc` | Thin compiler CLI and workflow composition |
| `sil-core` | Subject-owned semantic model and invariants |
| `sil-lexer` | Lexical boundary |
| `sil-parser` | Syntax-to-subject adapter |
| `sil-router` | Target-resolution service |
| `sil-codegen` | Go/Python/Bun-TypeScript output adapters |
| `sil-ipc` | Silc shared-memory ABI and UDS runtime boundary |

## Project layout and execution

`silc` is the compile-and-run entrypoint (CLI or shebang
`#!/usr/bin/env silc`). Runnable-v1 programs execute under the Rust supervisor;
other programs emit inspectable stubs.

| Concept | Meaning |
| --- | --- |
| Entry file | A `.silc` or conforming `.raku` program |
| Workdir | Directory that contains the entry file |
| `.runtime/` | Generated output under the workdir only |

**Invocation**

- `silc init` / `silc init <path>` — scaffold project files, provision Silc-owned
  Bun/CPython/Go into `~/.silc/runtimes/`, and write `.silc/runtimes.lock.json`
- `silc build <entry>` — compile only
- `silc <entry>` — compile; run when the program is runnable v1
- shebang `#!/usr/bin/env silc` still works

The first runnable build provisions checksum-verified Bun/CPython/Go into
`~/.silc/runtimes/` and writes compiler-owned `.silc/runtimes.lock.json`.
Engines are never copied into the workdir. `.runtime/` holds per-app workers,
IPC, and SQLite only. There is no user/AI runtime configuration surface.

Bare `init` is the subcommand; `init.silc` still compiles as a path.

**`.runtime` contract**

- Path: `{workdir}/.runtime/`
- Contents: generated Go / Python / TypeScript-for-Bun (including React UI assets
  for `ui::web`), IPC slots/run metadata, and application data
- First run builds this tree (slower); later runs reuse it when still valid
- Gitignored; inspectable for debugging; not the authoring surface
- User programs never emit into this repository’s `runtime/` directory
- Frontend dependency installs and bundles are compiler-owned (Silc Bun only)

**Repo `runtime/` vs project `.runtime/`**

| Path | Role |
| --- | --- |
| `runtime/{go,python,typescript}/` in this repo | Future **compiler-shipped harness templates**; `typescript` is executed by Bun |
| `{workdir}/.runtime/` | Per-program **generated codegen** produced when `silc` runs |

`models/` holds the future embedded ONNX intent classifier used by the router.

## Growth test

Before adding a crate or top-level module, ask:

- Is this a durable Silc concept with shared types and owned invariants? Put it
  in `sil-core` as a subject.
- Is it translating across a boundary or coordinating subjects? Keep it as a
  boundary service.
- Is it target-specific behavior? Keep it beneath the corresponding adapter.
- Is it merely reused code? First identify the subject that should own it.

This adapts subject-based thinking to compiler architecture without copying
frontend file conventions: ownership and cohesion transfer; UI-specific
splitting does not.

## Future work

- Expand subject validation beyond the example suite
- Expand the parser beyond the first-pass grammar
- Add program-level orchestration semantics
- Generalize executable target adapters beyond the feedback operation set
- Rich local `ui::terminal` rendering via compiler-owned OpenTUI/Bun (see ADR-003)
- Add typed field views atop the implemented mmap/UDS ABI
- Add program-level crash recovery and deployment bundles

See [`examples/article_pipeline.silc`](../examples/article_pipeline.silc) for the
NetworkIngress → EmbeddingEngine → RealtimeCache example.
