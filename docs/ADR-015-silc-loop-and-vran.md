# ADR-015: Silc `loop` and Vran

- **Status:** Accepted
- **Date:** 2026-10-04
- **Updated:** 2026-10-08
- **Related:** [ADR-014](ADR-014-loop-subject.md),
  [ADR-016](ADR-016-generic-kernel-domain-layers.md),
  [ADR-019](ADR-019-vran-language-into-loop.md)
- **Superseded by (partial):** [ADR-019](ADR-019-vran-language-into-loop.md)
  for the vran-oss language draft. The pause described below is withdrawn
  by the amendment in this file.
- **Canonical:** [`crates/sil-core/src/loops.rs`](../crates/sil-core/src/loops.rs)

## Amendment (2026-10-08)

Vran is not paused. The Vran application (`thoughtpivot/vran`) is an internal,
closed-source product. Its product noun is workflow. A workflow there starts
from one of three trigger types: Manual, Webhook, or Loop. That Loop trigger
is a recurring schedule that starts a workflow.

Silc's `loop` is a different thing: a declaration of automation
(`loop Name { loop::flow(...) }`) with the closed `loop::*` catalog in
ADR-014. It is not a trigger type on a Vran workflow, and a Vran Loop trigger
is not a Silc declaration.

The paragraphs below that say Vran is paused until 2027, and that this ADR
exists so the two efforts do not invent a second meaning for "loop," record
the 4 October 2026 plan. They are not the current fact. The language draft
those paragraphs meant (the private `thoughtpivot/vran-oss` repository, which
ADR-014 called "the loop deck and its open spec") is superseded by
[ADR-019](ADR-019-vran-language-into-loop.md). Silc carries those language
ideas inside `loop::*`. The vran-oss work will be retired. That removes the
risk of a second *language* called loop. It does not rename the Vran
application's Loop trigger, and it does not make Silc a construction workflow
product.

## Context

Silc 0.5.0 shipped a `loop` declaration. ADR-014 describes it as borrowing
lessons from "the loop deck and its open spec" without naming that spec.
The spec is Vran, ThoughtPivot's generic loop language and runtime.

On 29 September 2026 the weekly Vran productization sync settled the
open-source core as generic: verticals are built on top under other names.
On 30 September and 1 October, Q4 planning moved the company onto the
scraping marketplace and deprioritized Vran until 2027. The 1 October
all-team meeting committed to keeping the open-source tools maintained and
generic.

With Vran paused, Silc's `loop` is the loop language the organization ships.
That sentence was the 4 October plan. The amendment at the top of this file
replaces it: the Vran application is not paused, and the language draft is
superseded by ADR-019.

## Decision

Silc `loop` is the shipping generic loop declaration. It is not a temporary
fork that will be deleted when Vran resumes, and it is not a construction-
specific workflow language.

What Silc borrowed from the Vran loop deck:

- A closed catalog of steps rather than an open scripting surface.
- An explicit trigger (schedule, manual, or a resource mutation) before any step.
- Recorded outside reads and model answers so a resumed run replays instead of repeating work.
- Approval as a step with a deadline, not as an out-of-band email.
- Effects (`write`, `notify`) keyed so each key happens once.
- A local model call (`loop::ask`) that returns a typed contract, gated before it can cause an effect.

`llm::complete` and `loop::ask` are two names for one local-model capability.
They share one compatibility row with `text::score` and with `tensor::infer`.

Vran, when work resumes in 2027, may target these same semantics. This ADR
does not promise that Vran will compile Silc, or that Silc will grow Vran's
later features. It records the borrowing and the shared target so the two
efforts do not invent a second meaning for "loop."

The 2026-10-08 amendment withdraws the 2027 resume date and the claim that
the two efforts must not both use the word loop. The Vran application is
current, and its Loop trigger is a schedule. Silc's `loop` is the
declaration. ADR-019 is where the language draft went.

## Consequences

- Author-facing text calls this a `loop` declaration. "Subject" stays a compiler-internal word, as [intent-vs-subjects.md](intent-vs-subjects.md) already asks.
- `scrape::*`, `loop::read`, and `mcp::call` stay generic. A construction RFI chase is an example, not a namespace.
- The vran-oss language draft is superseded by ADR-019. The Vran application's Manual, Webhook, and Loop triggers stay product vocabulary. Silc's catalog stays the closed `loop::*` nodes in ADR-014.
