# ADR-016: Generic kernels, domain layers

- **Status:** Accepted
- **Date:** 2026-10-04
- **Related:** [ADR-012](ADR-012-webgpu-game-subject.md),
  [ADR-014](ADR-014-loop-subject.md),
  [ADR-015](ADR-015-silc-loop-and-vran.md),
  [domains/vdc.md](domains/vdc.md)
- **Canonical:** [`SCENE_KERNEL_NODES`](../crates/sil-core/src/game.rs),
  [`node_namespace`](../crates/sil-core/src/game.rs)

## Context

The September 29 Vran sync considered making Silc's built-in nodes
construction-centred and rejected it: a construction-specific core would make
the language unusable for anything else. The open-source compiler should be
generic. Verticals sit on top.

The 0.5.0 `game::` catalog mixed two vocabularies in one namespace. Entity,
mesh, light, camera, and zone are a real-time scene kernel. Pawn, weapon,
encounter, and objective are gameplay. The README already said the namespace
was named `game::` because the runtime borrowed game-engine patterns, and that
the same nodes should carry simulations and project environments. The catalog
did not match that sentence: a digital-twin author had to import "pawn" and
"weapon" to place a camera.

## Decision

The compiler ships generic kernels. A vertical's vocabulary is a layer or a
package, never a compiler default.

Kernels in 0.6.0:

- `ui::` — dual-surface interface nodes.
- `scene::` — the real-time scene kernel (entity, mesh, light, camera, asset, zone, and the rest of `SCENE_KERNEL_NODES`).
- `loop::` — scheduled, approval-gated work.
- Pipeline operations — `scrape`, `doc`, `tensor`, `llm`, `text`, `service`, `mcp`.

The first domain layer is gameplay. Those nodes stay `game::` (pawn, mode,
ability, weapon, projectile, and the rest of the catalog that is not in
`SCENE_KERNEL_NODES`). The root keyword is `scene`. `game Name` parses as the
same root for one release.

A source that still says `@version("0.5.0")` is rejected. When that source
still writes a kernel node as `game::`, the diagnostic lists each renamed
node in that program (`game::mesh` → `scene::mesh`) and no others.

Architecture, engineering, and construction is a go-to-market domain. It is
documented in [domains/vdc.md](domains/vdc.md) and exemplified under
`examples/domains/aec/`. It does not add nodes to the compiler.

## Consequences

- `scene::mesh` and `game::pawn` are the spellings. A kernel node written `game::` does not compile.
- Codegen still keys off the node name, so `scene::mesh` and a leftover `game::mesh` would lower the same way. Validation is what enforces the split.
- The next vertical (VDC semantics, construction connectors) is a package or an example, on the same pattern as `game::`.
