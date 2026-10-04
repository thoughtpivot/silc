# ADR-017: `ui::embed` dual-surface URL viewport

- **Status:** Accepted
- **Date:** 2026-10-04
- **Related:** [ADR-003](ADR-003-declarative-ui.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ADR-012](ADR-012-webgpu-game-subject.md)
- **Canonical:** `ui::embed` in
  [`UI_COMPONENT_CATALOG`](../crates/sil-core/src/ui.rs);
  web template
  [`ui_web_embed.tsx`](../crates/sil-codegen/templates/ui_web_embed.tsx);
  terminal card in
  [`ui_terminal_components.ts`](../crates/sil-codegen/templates/ui_terminal_components.ts)

## Context

Apps need to host an author-supplied URL (for example a separately running
`game` on another loopback port) inside a page or dialog. ADR-012 still forbids
mixing `game` with `app` / `component` / `resource` in one program. The gap is a
catalog primitive that *points at* a foreign surface without merging the two
runtimes.

## Decision

Add dual-surface `ui::embed`:

| Surface | Lowering |
| --- | --- |
| Web | Sandboxed `<iframe>` with `src` from `:src`, `title` from `:title` (fallback: the URL) |
| Terminal | Card: heading (`:title` or “Embedded page”), full URL, “Open in a browser” |

Catalog: props `src` (required), `title?`; events none; slots none; children none.

### Web sandbox and permissions

v1 tokens:

- `sandbox="allow-scripts allow-pointer-lock"`
- `allow="fullscreen; gamepad"`
- **Do not** set `allow-same-origin` together with `allow-scripts` for arbitrary
  author URLs (that combination is equivalent to no sandbox).
- No `srcdoc`, no author HTML, no `postMessage` / event bridge.

### WebGPU-in-iframe finding

Chrome validation (headless Chrome, loopback parent on one port, child on
another; `--enable-unsafe-webgpu`):

| Frame mode | `self.origin` | `isSecureContext` | `navigator.gpu` | `localStorage` |
| --- | --- | --- | --- | --- |
| `sandbox="allow-scripts"` | `"null"` (opaque) | `true` | present | blocked (`SecurityError`) |
| `sandbox="allow-scripts allow-same-origin"` | real loopback origin | `true` | present | allowed |
| no `sandbox` | real loopback origin | `true` | present | allowed |

Notes:

1. Opaque-origin sandboxed frames on loopback still report
   `isSecureContext === true` and a non-null `navigator.gpu` in current Chrome.
   Storage isolation still holds (`localStorage` throws).
2. There is **no** standardized Permissions-Policy / iframe `allow` token named
   `webgpu` (the gpuweb proposal was not pursued). Adding `allow="webgpu"` has
   no specified effect. v1 therefore uses `allow="fullscreen; gamepad"` only.
3. This environment could not obtain a GPU adapter in any mode (including
   top-level), so `requestAdapter()` success inside the sandboxed frame was
   **not** hardware-verified here. Authors should smoke-test a real Silc game
   host under a GPU-capable Chrome before relying on in-dialog play.

v1 keeps `allow-scripts` without `allow-same-origin` for arbitrary author URLs.
That preserves sandbox isolation while still exposing `navigator.gpu` on Chrome
loopback in the probe above. Do not fold a game/app merge into ADR-012; embed
remains a URL viewport, not a shared runtime.

Game hosts should continue to serve on their own origin. If a future game host
opts into COOP/COEP cross-origin isolation, re-validate iframe embedding; v1
does not require those headers.

## Non-goals

- Allowing `game::scene` inside an `app`, or `component` / `resource` inside a
  `game`
- Cross-frame RPC or shared Silc state
- Compile-time URL fetch / byte inlining
- Port `18140` or the string `game` as special cases

## Consequences

- Authors can place `ui::embed` in `ui::dialog` / `ui::page` with a literal or
  state-bound `:src($.game_url)`
- Dual-surface parity stays honest: terminal never pretends to be a canvas
- Catalog size moves 39 → 40; AGENTS.md and `docs_conformance` track the line
