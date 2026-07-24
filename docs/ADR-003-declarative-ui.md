# ADR-003: Declarative UI Surfaces (`ui::web`, `ui::terminal`)

- **Status:** Accepted (v1 partial implementation)
- **Date:** 2026-07-25
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md), [ARCHITECTURE.md](ARCHITECTURE.md)

## Context

Silc authors—humans and AI agents—express intent in dense Silc source. They must
not emit HTML, CSS, component frameworks, bundler configuration, or package
manifests. Today's runnable feedback portal used `html::form` + `http::serve`,
which leaked a markup-oriented surface into the language even though codegen
owned the actual HTML string.

Web and terminal UIs need first-class semantic operations so the compiler can
select and pin implementation substrates without the author knowing Vue,
OpenTUI, Vite, or npm.

## Decision

### Authoring surface

Silc declares UI intent through the `ui` namespace:

| Operation | Meaning | v1 status |
| --- | --- | --- |
| `ui::web(:port, :route)` | Browser UI bound to a Contract | Runnable (feedback-portal shape) |
| `ui::terminal()` | Interactive terminal UI bound to a Contract | Known stub; Bun-routed |

Canonical v1 web authoring:

```silc
FeedbackRecord
    ==> ui::web(:port(18080), :route("/"))
```

Compatibility aliases remain executable and lower to the same web profile:

```silc
FeedbackRecord
    ==> html::form()
    ==> http::serve(:port(18080), :route("/"))
```

Agents and new examples must prefer `ui::web`. Silc source never names Vue,
OpenTUI, CSS frameworks, or package managers.

### Compiler-owned substrates

| Surface | Engine | Substrate | Author visibility |
| --- | --- | --- | --- |
| `ui::web` | Bun | Vue (SFC/app + Silc theme) + Bun HTTP/API worker | None — generated under `.runtime/` |
| `ui::terminal` | Bun | OpenTUI (`@opentui/core` Zig native core + TypeScript bindings) | None — reserved for a future runnable path |

OpenTUI informs the terminal boundary: a native Zig renderer with TypeScript
bindings and Bun FFI, proven in production (OpenCode). Silc adopts that
**substrate shape**—compiler-owned, Bun-executed, not Electron—without exposing
OpenTUI APIs, components, or JSX in Silc source.

Vue is the blessed web substrate for the same reason: one codegen path, one
pinned dependency set, no agent choice among React/Vue/Svelte.

### Capability ownership

UI ops are capabilities, not package picks:

```text
capability: ui.web
  → adapter: vue-bun-v1
  → engine: bun
  → deps: compiler-pinned vue (+ build tooling via Bun)

capability: ui.terminal
  → adapter: opentui-bun-v1 (future runnable)
  → engine: bun
  → deps: compiler-pinned @opentui/core
```

The compiler emits exact manifests and lock data into `.runtime/`, installs with
the Silc-owned Bun binary, and builds browser assets during `silc build`. Users
and AI tools never run npm/yarn/pnpm or edit package configuration.

### Routing

Namespace evidence `ui` selects Bun (tier 2), consistent with `http` / `html` /
`ws`. Services already prefer Bun at tier 1.

### Non-goals for this slice

- Runnable `ui::terminal` supervisor / interactive TTY path
- Multiple competing web frameworks as user choices
- Authoring Vue SFCs, CSS, or OpenTUI trees in Silc projects
- SSR / hydration product features beyond a simple Bun-served SPA shell
- Theme DSL beyond the compiler-shipped Silc theme pack

## Consequences

### Positive

- Agents stay in semantic ops; HTML/CSS/framework knowledge drops out of the
  authoring loop.
- Web and terminal share one namespace and one routing target (Bun).
- Substrates can be swapped later without changing Silc programs.

### Costs and risks

- Vue and OpenTUI versions become compiler maintenance surfaces.
- Frontend dependency install/build adds latency to the first runnable build.
- `ui::terminal` must not be advertised as executable until a dedicated
  interactive supervisor path exists.
