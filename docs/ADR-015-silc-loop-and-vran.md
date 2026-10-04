# ADR-015: Silc `loop` and Vran

- **Status:** Accepted
- **Date:** 2026-10-04
- **Related:** [ADR-014](ADR-014-loop-subject.md),
  [ADR-016](ADR-016-generic-kernel-domain-layers.md)
- **Canonical:** [`crates/sil-core/src/loops.rs`](../crates/sil-core/src/loops.rs)

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

## Consequences

- Author-facing text calls this a `loop` declaration. "Subject" stays a compiler-internal word, as [intent-vs-subjects.md](intent-vs-subjects.md) already asks.
- `scrape::*`, `loop::read`, and `mcp::call` stay generic. A construction RFI chase is an example, not a namespace.
- A future Vran that wants the same semantics starts from this catalog and from ADR-014, rather than from a private dialect.
