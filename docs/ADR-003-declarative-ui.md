# ADR-003: Declarative UI Surfaces (`ui::web`, `ui::terminal`, components)

- **Status:** Accepted (0.2.0 dual-surface implementation)
- **Date:** 2026-07-25
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-002](ADR-002-silc-surface-syntax.md),
  [ADR-004](ADR-004-runtime-strengths.md), [ARCHITECTURE.md](ARCHITECTURE.md)

## Context

Silc authors—humans and AI agents—express intent in dense Silc source. They must
not emit HTML, CSS, component frameworks, bundler configuration, or package
manifests. Silc 0.1 used profile-selected portals (`PortalKind`) and `is view`
trees as skins over compiler-owned applications. That prevented general apps
(for example shopping carts) without adding another profile.

## Decision (0.2.0)

### Authoring surface

| Construct | Meaning |
| --- | --- |
| `class X is component` | Author-defined UI unit: props, `has state`, slots, emit, handlers, `render()` |
| `class X is resource` | Query/mutation data layer backed by Contracts / SQLite |
| `class X is app` | Routes + `method serve()` |
| `ui::web(:root(App), :port, :route)` | Browser surface for the app |
| `ui::terminal(:port)` | Terminal surface for the **same** component graph |

**Dual-surface is required:** every UI app must declare both `ui::web` and
`ui::terminal`. No component may be web-only or terminal-only. Authors write one
semantic tree; the compiler lowers it to React/Tailwind (web) and OpenTUI
(terminal). A TCP telnet CLI remains available as a remote/headless fallback
only — it is not the primary definition of `ui::terminal`.

`is view`, Contract-left-of-`ui::web` binding, and `PortalKind` profiles are
removed. Chat, scored forms, and product UIs are built from compiler-owned
catalog primitives and author-defined app components, not compiler modes.

### Semantic primitives

Built-ins (`ui::page`, `ui::button`, `ui::table`, `ui::select`, …) expose
semantic events (`:on(click(handler))`, `:on(submit(handler))`) and expression
props. Catalog entries declare both `web` and `terminal` surfaces and must have
matching lowerers for React and OpenTUI.

Shared prop vocabulary:

- `:field(ident)` — bind to component state
- `:value(expr)` — controlled/display value
- `:variant` / `:tone` / `:size` — closed role enums
- Flags (`:disabled`, `:sortable`, `:searchable`, `:selectable`, `:dense`, …)

### Out-of-box components

Out-of-box UI capability lives in the compiler-owned primitive/component
catalog and codegen templates. There is no separate component-source standard
library or resolver. Author-defined components remain in application source,
and the presence of any primitive never selects application behavior.

### Non-goals

- Authoring React, Tailwind, OpenTUI, or CSS in Silc source
- Unrestricted framework escape hatches
- Surface-specific component catalogs

## Consequences

- Shopping and other CRUD apps are expressible without a domain `PortalKind`
- Terminal is a first-class equal of web for every component
- Codegen emits one app worker + dual-surface modules from a shared IR
