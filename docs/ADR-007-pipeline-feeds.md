# ADR-007: Silc Pipeline Feeds (`==>`)

- **Status:** Accepted
- **Date:** 2026-07-26
- **Related:** [ADR-002](ADR-002-silc-surface-syntax.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)

## Context

Silc borrows the `==>` glyph from Raku’s feed operator for dense dataflow
authoring. In Raku, `==>` threads the left-hand result as the last argument to
the right-hand callable at runtime. Silc uses the same surface for a different
purpose: building a compile-time **Pipeline** subject that the router and
codegen lower into Bun / CPython / Go workers and IPC stages.

## Decision

Silc `==>` constructs **Pipeline IR**. It is not Rakudo call-threading.

### Value and type per step

| Left-hand form | Meaning |
| --- | --- |
| Contract name (e.g. `Product`) | Binds the schema / primary record type for following ops |
| Field access (e.g. `$record.prompt`) | Selects a value from a record |
| Identifier | Named value or binding in the method’s pipeline |
| Prior op result | Implicit input to the next `ns::op` |

Type rules come from Contracts, subsets, and the operation registry
(`EXECUTABLE_OPS`). Unknown or stub-only ops in a runnable graph are compile
errors.

### Argument position

- The left-hand value is the **primary input** to the next step.
- Colon-pair arguments (`:table(products)`, `:port(8080)`) are **named options**
  on the call, not positional Raku feed targets.
- Example: `Product ==> resource::list(:table(products))` feeds `Product` as the
  primary schema/input; `:table` is an option.

### Synchronous vs asynchronous execution

| Layer | Behavior |
| --- | --- |
| Compile time | Entire `==>` chain is a static Pipeline graph |
| Within a worker | Steps in that worker run synchronously in process order |
| Across workers | Supervisor stages (e.g. Bun ingest → Python → Go) are asynchronous over UDS + mmap slots |

Authors do not schedule threads. The compiler and supervisor own staging.

### Error propagation

| Phase | Behavior |
| --- | --- |
| Compile | Unknown ops, stub/executable mixes, graph invariant failures → hard error |
| Runtime (control) | Workers report `ERROR` / failed `RESPONSE` frames over UDS |
| Runtime (request) | HTTP handlers return failure to the client; request does not silently succeed |

### Contract transforms

- A Contract name on the left establishes the logical schema for following ops.
- Ops may read, attach, or replace fields according to their registry semantics
  (e.g. `llm::complete` attaches a reply; `store::sqlite` persists the record).
- Subset-typed `Str` fields are validated at ingress when predicates are declared
  (ADR-002 v1).

### Versus Raku

| | Raku `==>` | Silc `==>` |
| --- | --- | --- |
| Meaning | Pass LHS as last arg to RHS callable | Build Pipeline subject for codegen |
| Runtime | Rakudo evaluates callables | Workers synthesized by `sil-codegen` |
| Extensibility | Any routine in scope | Closed / stub-gated `ns::op` registry |

## Consequences

- Agents and docs must not assume Raku feed semantics.
- Expanding pipeline power means growing `EXECUTABLE_OPS` and adapters, not
  opening arbitrary callables.
- AGENTS.md and the README link here as the feed contract.
