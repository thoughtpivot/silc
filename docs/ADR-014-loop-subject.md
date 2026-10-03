# ADR-014: Loop subject (`loop::*`)

- **Status:** Accepted
- **Date:** 2026-10-02
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-004](ADR-004-runtime-strengths.md),
  [ADR-005](ADR-005-local-llm-complete.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ADR-012](ADR-012-webgpu-game-subject.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)
- **Canonical:** [`crates/sil-core/src/loops.rs`](../crates/sil-core/src/loops.rs),
  [`loop_lower`](../crates/sil-codegen/src/loop_lower.rs),
  [`templates/loop_kernel.go`](../crates/sil-codegen/templates/loop_kernel.go)

## Context

Silc programs answer requests: a person clicks, a terminal key fires, a
pipeline row arrives. Teams also need work that runs *on its own*: every
weekday morning, find the overdue RFIs, draft a reminder, and ask the project
manager before anything goes out. Writing that as a hidden cron job outside
the program loses everything Silc guarantees: the compiler cannot see it, the
runtime cannot replay it, and nobody can say what it costs.

We studied a declarative operations language built for exactly this problem
(the "loop" deck and its open spec). We did **not** adopt its syntax, IR,
package format, or runtime. We took its lessons and expressed them as one more
Silc subject with a closed node catalog, the same way `game` (ADR-012) did.

## Lessons taken

1. **Deterministic first.** Most steps are plain lookups, comparisons, and
   writes. A model is called only by an explicit `loop::ask`, and its answer is
   a typed contract, never free text that flows straight into an effect.
2. **One trigger per loop.** `loop::schedule` (5-field cron, required IANA
   time zone, optional bounded catch-up, never overlapping itself),
   `loop::manual`, or `loop::on_mutation` of a declared resource.
3. **Gates fail closed.** A condition that is false *or unknown* (missing
   field, null) stops the path. A gate's `loop::otherwise` block must end in
   `stop`, `fail`, or `skip`; it cannot fall through.
4. **Model output is checked before it acts.** The compiler tracks values
   that came from `loop::ask` (and anything derived from them). They may reach
   `loop::write` / `loop::notify` only after a `loop::gate` that inspects them,
   an `loop::approve` that shows them to a person, or an explicit
   `:unchecked("reason")`.
5. **Effects are keyed and produce receipts.** Every `write` and `notify`
   carries a `:key` template. The kernel reserves a receipt, performs the
   effect, and commits the receipt in one transaction. A replayed or retried
   run with the same key does nothing twice.
6. **Static bounds and a cost report.** Every `find` and `each` has a `:max`,
   every `ask` a bounded `:retry`, every `approve` a `:within`. From these,
   `silc build` prints the worst-case model calls, effects, and approvals for
   one run of each loop.
7. **Pinned, replayed runs.** A run is pinned to the hash of the plan it
   started on. Nondeterministic inputs (clock, finds, reads, model answers,
   approval decisions) are recorded in an append-only event log. Resuming
   after a crash or an approval replays the log instead of asking again.
8. **Fixed tool calls, not an agent.** Where the source system let a model
   choose which tools to call, a Silc loop names each MCP tool call and its
   arguments. The model only reads what the loop fetched, so the reads, their
   cost, and their replay are known at build time.

## Left out (v1)

Plugins and connector packages, package/lock files, pluggable storage for
loop state, person-owned fields, citations, money types, irreversible-effect
classes, external email or chat delivery, and the source language's syntax
and IR, and model-driven tool selection. `loop::notify` writes to a
synthesized in-app notices table. `loop::read` supports `scrape::page` (HTTP
GET) and `mcp::call` (one named tool per step).

## Decision

1. **First-class `loop` subject.** `loop Name { loop::flow(...) }` with a
   closed `LOOP_NODE_CATALOG` in `sil-core`. `loop` is a contextual keyword at
   top level only; it stays an ordinary identifier elsewhere (for example
   `game::audio(:kind(loop))`).
2. **Catalog (v1).** Root `flow`; triggers `schedule`, `manual`,
   `on_mutation`; steps `let`, `find`, `read`, `ask`, `gate`, `branch`
   (`when` + required `otherwise`), `each`, `write`, `notify`, `approve`
   (`declined` + `timed_out` blocks required), `stop`, `fail`, `skip`.
3. **Coexistence.** Loops live beside `app`, `component`, `resource`, and
   processor modules. They may not be mixed with `game`, and a program that
   uses `loop::ask` may not also use `text::score` (the same rule as
   `llm::complete`).
4. **Runtime split (ADR-004).**
   - **Go kernel** (`go/loop/`): scheduler, run table, event log, receipts,
     replay, suspension for approvals, manual-run and mutation polling. It
     shares the program's SQLite file.
   - **CPython** answers `loop::ask` through the existing silclm worker. The
     kernel sends an `ASK` control frame over the Silc UDS; the supervisor
     routes it to Python through the mmap slot and returns the reply.
   - **Bun** serves a synthesized dual-surface inbox at `/loops`: pending
     approvals (approve / decline with a name that must match `:by`), a
     run-now button per manual or scheduled loop, recent runs, and notices. It is built
     from synthesized `LoopApproval`, `LoopRun`, `LoopNotice`, and
     `LoopRequest` resources, so it renders on web and terminal like any app.
   - **Supervisor** builds and keeps the kernel alive.
5. **State tables.** `loop_runs`, `loop_approvals`, `loop_notices`, and
   `loop_requests` are resource tables (readable through `/api`).
   `loop_events`, `loop_receipts`, and `loop_schedule` are kernel-private.
6. **Outside data.** `loop::read(:op("mcp::call"), :server, :tool, :args,
   :auth_env, :select)` calls one tool on an MCP server over streamable HTTP
   (initialize, initialized, `tools/call`; JSON or SSE replies). Arguments are
   a `Contract.new(...)`, so they are typed and checked. The bearer token is
   read from the named environment variable at run time and never appears in
   source or the plan. `:select` is a dotted path into the result
   (`records.parsed`); lists map over their items. The result binds `text`
   and `data`. The reserved `$calendar` binding carries the run's date window
   (`today`, `weekday`, `last7_start`, `last7_end`, `next7_end`) in the loop's
   time zone and is recorded with the clock.
7. **Run now.** The inbox starts `manual` and `schedule` loops on request, so
   a scheduled loop can be run off schedule without a second declaration. The
   no-overlap rule still applies.
8. **Plans are pinned.** `loop_lower` emits `loop/plan.json` and a
   content-addressed copy at `loop/plans/<sha256>.json`. The hash covers the
   lowered plan and the compiler version. Older plan files are kept so
   in-flight runs finish on the plan they started with.

## Consequences

- Authors get scheduled, approval-gated, model-assisted work without leaving
  the language, and reviewers can read the worst case from `silc build`.
- The kernel is a fourth long-lived worker; programs without loops do not
  build or start it.
- v1 deliberately narrows effects to Silc resources and in-app notices.
  External delivery would arrive as new catalog nodes, not as plugins.
- Proof programs: `examples/rfiChaseApp` (finds, approvals, keyed writes) and
  `examples/oneThingApp`, a port of a production daily-briefing workflow
  (four Moz MCP reads, two silclm asks, a gate, one keyed write per day).
