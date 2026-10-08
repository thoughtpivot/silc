# ADR-019: Vran 0.1 language folded into `loop::*`

- **Status:** Accepted
- **Date:** 2026-10-08
- **Related:** [ADR-014](ADR-014-loop-subject.md),
  [ADR-015](ADR-015-silc-loop-and-vran.md),
  [ADR-016](ADR-016-generic-kernel-domain-layers.md),
  [vran-0.1-loop-grammar.md](vran-0.1-loop-grammar.md)
- **Supersedes (partial):** the 4 October 2026 pause and "second meaning of
  loop" paragraphs in [ADR-015](ADR-015-silc-loop-and-vran.md). See that ADR's
  amendment of the same date.
- **Canonical:** the shipped catalog is
  [`LOOP_NODE_CATALOG`](../crates/sil-core/src/loops.rs). This ADR does not
  add nodes to it.

## Context

The private repository `thoughtpivot/vran-oss` (`main`, commit `699ec980`,
29 September 2026) holds a draft 0.1 language for deterministic work that
occasionally reasons. Billy (Bilyal Mestanov) proposed the Loop grammar.
That commit, which he authored, is the version of the proposal the repository
implemented. The message is "docs(spec): add Vran 0.1 language spec, plugin
model, grammar, and construction examples." There is no parser, compiler, or
interpreter. The files under `examples/construction/` were checked by hand
against `spec/vran-0.1.md`, `spec/grammar.ebnf`, and `spec/plugins.md`.

The grammar and the syntax sections of the spec are copied verbatim in
[vran-0.1-loop-grammar.md](vran-0.1-loop-grammar.md). This ADR does not claim
that the `loop::*` catalog conforms to that grammar. The comparison table in
the grammar note is for Dan to confirm. Every row there is unconfirmed.

On 8 October 2026 Dan Stephenson decided that this draft is folded into Silc
as an ADR on the `loop` declaration, and that the vran-oss repository will be
retired because Silc now carries the ideas. The Vran application
(`thoughtpivot/vran`, internal and closed source) is a separate product. It
keeps "workflow" as its product noun. ADR-015 records that split.

Silc already shipped a closed `loop::*` catalog (ADR-014). This record maps
the draft onto that catalog. It states what the compiler does today, which
catalog additions are **Proposed** (not in `LOOP_NODE_CATALOG`, not lowered,
not run by the Go kernel), and what Silc does not take. Construction files in
the draft are examples of the draft. They are not Silc vocabulary.

## What the draft is

A `.vran` file starts with `vran 0.1`, then `use` lines, then declarations.
The unit of execution is `workflow`, with an optional IANA time zone, exactly
one `on` trigger, and a sequence of steps. Indentation is two spaces. Hard
keywords include `workflow`, `on`, and the step verbs. Interface files
(`.vrani`) declare plugins. `vran.lock` pins each package's version and the
SHA-256 of its interface. A plan hash covers the canonical source, imported
local modules, and the lockfile. A run is pinned to the deployment it started
on.

The core guarantees, which the draft says an interpreter must enforce, are:

1. A completed effect is never repeated, including after a crash.
2. A `check` that is false or unknown fails closed.
3. Model output reaches an effect only after a `check` or a `signoff`, unless
   the step says `unchecked` with a reason.
4. An effect marked `irreversible` is preceded by a signoff, unless the step
   says `unattended` with a reason.
5. Every `each`, plural `find`, and list type has a static bound (at most
   10000). `retry` is at most 10. `within` and `catch up` are at most 30 days.
   From those bounds a checker can report the worst case for one run.
6. A run finishes on the plan it started on. Reads, model answers, register
   rows, receipts, signoff decisions, `today`, and `now` are appended to an
   event log and reused on resume.

Plugins add operations and types. They do not add syntax, and they cannot
weaken those guarantees. The draft's standard library (`vran/math`,
`vran/date`, `vran/text`, `vran/table`) is pure functions. Connectors (email,
Teams, forms, files, PDF, SharePoint, and illustrative domain packages) supply
triggers, reads, effects, storage, and approval channels. Model vendors are
deployment bindings, not imports.

`spec/rationale.md` is not normative. It says the language is readable by the
person who runs the work, deterministic by construction, and small: plugins
grow the vocabulary without growing the grammar. `spec/ir-delta.md` lists
changes for a Plan IR that lives in the Vran application, not in Silc.
`spec/coverage.md` lists constructs the corpus uses and constructs the draft
explicitly deferred (in-run waits for replies, a bounded `loop` step inside a
run, concurrency inside `each`, more than one trigger, webhooks as a core
form).

## Mapping

Shipped means the node or rule is in `LOOP_NODE_CATALOG` and described by
ADR-014. Proposed means this ADR adopts the idea as future catalog work.
Nothing in the Proposed column is implemented. Neither label means the
catalog has been checked against Billy's grammar. That check is the
unconfirmed table in
[vran-0.1-loop-grammar.md](vran-0.1-loop-grammar.md).

| Draft concept | Silc |
| --- | --- |
| `workflow` name, one trigger, then steps | Shipped as `loop Name { loop::flow(trigger, steps...) }`. The keyword stays `loop`. |
| `on schedule every weekday at 08:00 in <zone>`, `every day`, a named weekday, `every month on day N`, and `on schedule cron "..."` | Shipped as `loop::schedule` with a five-field `:cron`, a required IANA `:tz`, and optional `:catch_up` (latest missed firing only, at most 30 days). The English recurrences are spellings of cron. Silc does not add them. |
| `on manual` | Shipped as `loop::manual`. |
| Plugin triggers (`form.submitted`, `email.received`), `once per`, and `shape` | Not adopted. The shipped "something changed" trigger is `loop::on_mutation` of a declared Silc resource. A webhook is not a `loop::*` node. |
| `let` | Shipped as `loop::let`. |
| `find` / `find one`, `where`, `order by`, `max` | Shipped as `loop::find` (`:max`, `:one`, `:where`, `:order`, `:desc`). The source is a Silc resource query, not a plugin-backed register. |
| `read` with `retry` | Shipped as `loop::read`. The closed operations are `scrape::page` and `mcp::call`. Answers are recorded for replay. |
| `ask` returning a declared type, with `retry` and an `else` that must terminate | Shipped as `loop::ask` (`:into` a contract, `:retry`, `loop::otherwise` ending in `stop`, `fail`, or `skip`). The model is silclm. |
| `ask` `cite` / `using` a model profile | Citations are **Proposed** as options on `loop::ask`, not shipped. Named model profiles and vendor packages are not adopted. |
| `check` (fail closed, including unknown) | Shipped as `loop::gate`. |
| `match` | Shipped as `loop::branch` with `loop::when` and a required `loop::otherwise`. |
| `each` with a required `max` | Shipped as `loop::each`. |
| `fallback` (two or more reads, `accept when`, terminating `else`) | **Proposed** as `loop::fallback`. Not shipped. |
| `signoff` with `by`, `within`, required declined and timed-out blocks | Shipped as `loop::approve` with `loop::declined` and `loop::timed_out`. The inbox is `/loops`. A plugin `channel` (`via teams.approval`) is not adopted. |
| `upsert` into a register | Shipped as `loop::write` to a Silc resource mutation, with a `:key`. |
| `do` an external effect | Shipped only as `loop::notify`, which writes an in-app notice. External delivery is not shipped. |
| `stop`, `fail`, `skip` | Shipped under those names. `skip` is valid only inside `each`. |
| Keys and receipts | Shipped. `:key` must interpolate at least one placeholder. The kernel reserves a receipt, performs the effect, and commits the receipt in one transaction. |
| Model output gated before an effect, or `unchecked` with a reason | Shipped for `loop::write` and `loop::notify`. |
| Irreversible effects and `unattended` | **Proposed** as a static rule on future effect nodes: an irreversible effect needs `loop::approve` on every path, or an explicit reason. Not shipped. There is no irreversible class in the catalog. |
| Uncertain effects (`idempotency none` becomes `needs_attention` instead of a silent retry) | **Proposed** as kernel behavior for a future external effect, not as syntax. Not shipped. Today's `write` and `notify` receipts are local and idempotent. |
| Bounds and a worst-case report | Shipped. `silc build` reports the worst case for one run. |
| Pinned plans, event log, replay, suspension only while a person decides | Shipped. `loop_lower` writes `loop/plan.json` and `loop/plans/<sha256>.json`. The hash covers the lowered plan and the compiler version. |
| Runs | Shipped as rows in `loop_runs`, with kernel-private `loop_events` and `loop_receipts`. |
| Registers: `person owns`, `derived`, `sequence`, `readonly`, storage plugins | Not adopted. ADR-014 already left person-owned fields and pluggable loop storage out. Silc state is the program's resources and the loop tables. |
| `.vran` grammar, `.vrani` interfaces, `use`, `vran.lock` | Not adopted. |
| Pure-function plugins | Not adopted. Values in a loop are Silc expressions. |
| Plan IR delta aimed at the Vran application's plan package | Not adopted. |
| Stable `Vxxxx` / `Rxxxx` diagnostic codes and `vran fmt` | Not adopted. |

`loop::read` of one named `mcp::call` is the shipped way to reach an external
tool. Arguments are a `Contract.new(...)`. The bearer token is an environment
variable named in `:auth_env`, not a plugin `secret` block. That is a closed
call, not an imported package.

## Proposed catalog additions

These are proposals only. They are not nodes, options, or kernel behavior
until a later change lands in `crates/sil-core/src/loops.rs` and the loop
kernel.

1. **`loop::fallback`.** Two or more `loop::read` attempts, in order, kept
   when a condition holds, with a terminating `otherwise` when none hold.
   `retry` on one read stays "same operation again." Fallback stays "a
   different read."
2. **Citations on `loop::ask`.** An optional list of source bindings. The
   contract that receives the answer has a citations field. The kernel
   accepts an answer only when each quote occurs in a cited source. The
   model remains silclm.
3. **Irreversible effects.** When an external effect node exists, the checker
   rejects it unless every path to it has passed `loop::approve`, or the
   effect carries a non-empty reason that it ran unattended. `loop::write`
   and `loop::notify` do not grow this flag in the current catalog.
4. **Uncertain external effects.** If a future effect cannot dedupe at the
   far end, a crash between reserve and commit pauses the run for a person.
   The kernel does not call that effect again on its own. This does not
   change today's SQLite receipts.

New effects, if any, arrive as closed `loop::*` nodes. They do not arrive as
packages.

## Not adopted

- The `.vran` surface, the indentation grammar, the `workflow` keyword, the
  `vran 0.1` header, and the hard-keyword list.
- `.vrani` files, `use`, package owners, major-version imports, `vran.lock`,
  and interface hashes as the identity of a plan.
- An open set of connectors. Email, chat, forms, files, PDF, spreadsheets,
  and domain systems are not Silc nodes because a draft used them.
- Plugin storage for registers, person-owned fields, derived fields, key
  sequences, and read-only projections.
- Approval delivered by an external channel plugin. Approval stays the
  synthesized inbox.
- Choosing a model vendor from a workflow. `loop::ask` stays silclm, under
  the same compatibility row as `llm::complete`.
- The Vran application's Plan IR, including the renames in `spec/ir-delta.md`.
- More than one trigger on a loop, in-run waits for an external reply, and a
  step that cycles inside a run. The draft deferred the last two as well.
  One loop has one trigger. A reply that arrives later is another run.

## Examples

The draft's construction directory is a hand-checked corpus, not a product
surface. Five workflows share `project.vran`. Fifteen `negative/` files each
break one static rule. Fifteen `.vrani` files and one `vran.lock` describe
the packages those workflows import. `research/` holds the notes the draft
was written from. None of those files are Silc programs, and this ADR does
not port them.

Silc stays generic (ADR-016). Architecture, engineering, and construction is
a go-to-market domain and an example, documented in
[domains/vdc.md](domains/vdc.md) and under `examples/domains/aec/`.
`rfiChaseApp` exercises the shipped catalog (`loop::schedule`, `loop::find`,
`loop::each`, `loop::gate`, `loop::ask`, `loop::approve`, `loop::write`,
`loop::notify`). It is not a translation of the draft's `rfi-chase.vran`,
which sends mail and chat and does not call a model.

## Consequences

- Authors keep one declaration, `loop`, and one closed catalog. The draft's
  guarantees that Silc already enforces stay in ADR-014.
- The four proposals above are not implemented. A program cannot write
  `loop::fallback`, cannot cite sources from `loop::ask`, and cannot mark an
  effect irreversible.
- vran-oss is the source of this mapping and is to be retired. Silc does not
  vendor its grammar, lockfile, or examples.
- The Vran application is unchanged by this ADR. Its product noun remains
  workflow. ADR-015, as amended on this date, is the record of how that
  product's Loop trigger differs from Silc's `loop` declaration.
