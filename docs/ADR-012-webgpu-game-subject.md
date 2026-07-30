# ADR-012: WebGPU game subject (compiler-synthesized)

- **Status:** Accepted
- **Date:** 2026-07-29
- **Updated:** 2026-07-29
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-004](ADR-004-runtime-strengths.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ADR-003](ADR-003-declarative-ui.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)
- **Canonical:** [`crates/sil-core/src/game.rs`](../crates/sil-core/src/game.rs),
  [`game_lower`](../crates/sil-codegen/src/game_lower.rs),
  templates under [`crates/sil-codegen/templates/game/`](../crates/sil-codegen/templates/game/)

## Context

Authors need a first-class way to declare real-time WebGPU scenes with a
general game framework (entity trees, prefabs, signals, mode/pawn/controller,
abilities). Dual-surface `app` + React/OpenTUI (ADR-003 / ADR-009) cannot
express those systems as intent.

Silc already provisions Bun, CPython, and Go for runnable programs. A `game`
subject must not become a Bun-only escape hatch that abandons the polyglot
spine (ADR-001 / ADR-004).

## Decision

1. **First-class `game` subject.** Authors declare
   `game Name { game::scene(...) }` with a closed `GAME_NODE_CATALOG`.
2. **Web-only surface exemption.** Game programs synthesize a browser WebGPU
   surface only. ADR-009 dual-surface parity remains mandatory for `app`; it
   does **not** apply to `game` (no OpenTUI / terminal chrome).
3. **No mix with route UI.** A program may not declare both `game` and `app`
   (or UI `component` / `resource`) in v1.
4. **Lower → manifest → kernel runtime.** `game_lower` encodes the scene tree
   as JSON (prefabs, data assets, signals, mode). Compiler-owned TypeScript
   templates implement a Silc game kernel on Babylon WebGPU + Vite + Bun.
5. **Polyglot synthesis (locked).** Every game program synthesizes:
   - **Bun** — WebGPU host, static `dist/` serve, HTTP for settings/saves/telemetry/runs, UDS client
   - **CPython** — compile-time asset bake (`game_bake.json`: resolved data refs, collider hulls, spawn/signal tables)
   - **Go** — SQLite migrations and durable tables (`game_saves`, `game_runs`, `game_events`, `game_settings`)
6. **Pins.** `@babylonjs/*` **9.16.2**, Vite **8.1.5**. No CDN asset fetches;
   no WebGL/mobile fallback. Missing `navigator.gpu` → one-line stop.
7. **No title-named compiler branches.** Runtime behavior is driven by the
   lowered manifest and catalog nodes, never by string-matching a demo title.

## Consequences

- Emit branch `emit_game` plus supervisor `build_game_web`,
  `build_game_python_bake`, `build_go_worker`, and IPC-aware `run_game`.
- Assist teaches reusable `game::*` scene composition.
- Default HTTP port **18140** (18141 reserved).
- Example apps under `examples/` prove capability; they are not the product
  definition of the subject.

## Addendum: game kernel synthesis (2026-07-29)

Templates under `templates/game/` implement a Silc-owned gameplay kernel with
Babylon as the WebGPU adapter only:

| Layer | Pattern | Silc surface |
|---|---|---|
| Hierarchy | Godot node tree | Nested `game::entity`; parent/child transforms |
| Messaging | Godot signals / groups | `game::signal`, `game::group`, manifest edges |
| Reuse | Unity prefabs + ScriptableObjects | `game::prefab`, `game::spawn` overrides, `game::data` + `:ref` |
| Ownership | Unreal Mode / Pawn / Controller | `game::mode`, `game::pawn`, `game::controller` |
| Abilities | GAS-lite | `game::ability` + cue children; cost/cooldown/attributes |
| Bake | Unity-style import | CPython → `public/baked/game_bake.json` |
| Persist | Unreal save / analytics | Go SQLite + Bun HTTP edge |

Author intent remains `main.silc` + `AGENTS.md` only.
