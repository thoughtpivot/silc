# ADR-003: Declarative UI Surfaces (`ui::web`, `ui::terminal`)

- **Status:** Accepted (v1 partial implementation)
- **Date:** 2026-07-25
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-004](ADR-004-runtime-strengths.md), [ARCHITECTURE.md](ARCHITECTURE.md)

## Context

Silc authors—humans and AI agents—express intent in dense Silc source. They must
not emit HTML, CSS, component frameworks, bundler configuration, or package
manifests. Today's runnable feedback portal used `html::form` + `http::serve`,
which leaked a markup-oriented surface into the language even though codegen
owned the actual HTML string.

Web and terminal UIs need first-class semantic operations so the compiler can
select and pin implementation substrates without the author knowing React,
Tailwind, ShadCN, OpenTUI, Vite, or npm.

## Decision

### Authoring surface

Silc declares UI intent through the `ui` namespace:

| Operation | Meaning | v1 status |
| --- | --- | --- |
| `ui::web(:port, :route)` | Browser UI bound to a Contract | Runnable (feedback-portal shape) |
| `ui::terminal(:port)` | Telnet-compatible terminal UI bound to a Contract | Runnable alongside `ui::web` |

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

Agents and new examples must prefer `ui::web`. Silc source never names React,
Tailwind, ShadCN, OpenTUI, CSS frameworks, or package managers.

### Compiler-owned substrates

| Surface | Engine | Substrate | Author visibility |
| --- | --- | --- | --- |
| `ui::web` | Bun | React + Tailwind CSS + ShadCN-style primitives + Bun HTTP/API worker | None — generated under `.runtime/` |
| `ui::terminal` | Bun | Line-oriented TCP/telnet adapter now; OpenTUI for local rich TTY later | None — generated under `.runtime/` |

OpenTUI informs the terminal boundary: a native Zig renderer with TypeScript
bindings and Bun FFI, proven in production (OpenCode). Silc adopts that
**substrate shape**—compiler-owned, Bun-executed, not Electron—without exposing
OpenTUI APIs, components, or JSX in Silc source.

React is the blessed web substrate because it is the default ecosystem Silc
should ride: one codegen path, one pinned dependency set, no agent choice among
React/Vue/Svelte. Tailwind is the low-level styling primitive; ShadCN-style
primitives (compiler-vendored components, not a user CLI) materialize forms,
inputs, buttons, and layout shells. Authors express UI intent in Silc; the
compiler lowers Contracts into those tools.

### Capability ownership

UI ops are capabilities, not package picks:

```text
capability: ui.web
  → adapter: react-bun-v1
  → engine: bun
  → deps: compiler-pinned react, react-dom, tailwindcss, shadcn-style primitives

capability: ui.terminal
  → adapter: telnet-bun-v1 (runnable remote terminal)
  → engine: bun
  → deps: none

capability: ui.terminal.rich
  → adapter: opentui-bun-v1 (future local TTY)
  → engine: bun
  → deps: compiler-pinned @opentui/core
```

The compiler emits exact manifests and lock data into `.runtime/`, installs with
the Silc-owned Bun binary, and builds browser assets during `silc build`. Users
and AI tools never run npm/yarn/pnpm, `shadcn` CLI, or edit package
configuration.

### Routing

Namespace evidence `ui` selects Bun (tier 2), consistent with `http` / `html` /
`ws`. Services already prefer Bun at tier 1. See
[ADR-004](ADR-004-runtime-strengths.md).

### Non-goals for this slice

- Rich local OpenTUI renderer (the current terminal surface is telnet/TCP)
- Multiple competing web frameworks as user choices
- Authoring React components, Tailwind configs, ShadCN CLI, or OpenTUI trees in
  Silc projects
- SSR / hydration product features beyond a simple Bun-served SPA shell
- Contract-driven dynamic form generation beyond the feedback-portal template
  (ShadCN primitives are the foundation for that later growth)
- Theme DSL beyond the compiler-shipped Silc theme pack

## Consequences

### Positive

- Agents stay in semantic ops; HTML/CSS/framework knowledge drops out of the
  authoring loop.
- Web and terminal share one namespace and one routing target (Bun).
- Substrates can be swapped later without changing Silc programs.
- React + Tailwind + ShadCN ride the largest web ecosystem while remaining
  invisible to authors.

### Costs and risks

- React, Tailwind, ShadCN primitives, and OpenTUI versions become compiler
  maintenance surfaces.
- Frontend dependency install/build adds latency to the first runnable build.
- Telnet is unencrypted and unauthenticated in v1; the listener is loopback-only.
