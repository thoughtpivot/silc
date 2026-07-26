# ADR-003: Declarative UI Surfaces (dual-surface components)

- **Status:** Accepted (0.2.0 dual-surface; 0.4.0 synthesized serving)
- **Date:** 2026-07-25
- **Updated:** 2026-07-27
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-002](ADR-002-silc-surface-syntax.md),
  [ADR-004](ADR-004-runtime-strengths.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)
- **Superseded by (partial):** [ADR-009](ADR-009-compiler-synthesized-runtime.md)
  for author-declared `ui::web` / `ui::terminal` / `method serve()` mechanics.
- **Canonical:** [`UI_COMPONENT_CATALOG`](../crates/sil-core/src/ui.rs);
  rendered lines via `format_component_catalog_line` in
  [`AGENTS.md`](../crates/silc/templates/AGENTS.md) and the root README.

## Context

Silc authors—humans and AI agents—express intent in dense Silc source. They must
not emit HTML, CSS, component frameworks, bundler configuration, or package
manifests. Silc 0.1 used profile-selected portals (`PortalKind`) and `is view`
trees as skins over compiler-owned applications. That prevented general apps
(for example shopping carts) without adding another profile.

## Decision

### Authoring surface (0.4.0)

| Construct | Meaning |
| --- | --- |
| `component X` | Author-defined UI unit: props, `has state`, slots, emit, handlers, `render()` |
| `resource X for Contract` | Capability CRUD backed by Contracts / SQLite |
| `app X` | Route table only |
| Catalog `ui::*` in `render()` | Template vocabulary for the semantic tree |

**Dual-surface is required as a product outcome:** every UI `app` synthesizes
both web (`ui::web` → React/Tailwind) and terminal (`ui::terminal` → OpenTUI)
surfaces automatically. Authors declare routes only — never `method serve()`,
`ui::web`, or `ui::terminal` as program operations. No component may be
web-only or terminal-only. A TCP telnet CLI remains a remote/headless fallback
only — it is not the primary definition of the terminal surface.

Override ports at runtime with `SILC_HTTP_PORT` / `SILC_TERMINAL_PORT`
(defaults 18088 / 18023).

`is view`, Contract-left-of-`ui::web` binding, and `PortalKind` profiles are
removed. Chat, scored forms, and product UIs are built from compiler-owned
catalog primitives and author-defined app components, not compiler modes.

### Historical (0.2.0–0.3.0)

Earlier releases required authors to write:

```silc
method serve() {
    ui::web(:root(MyApp), :port(18080), :route("/"))
        ==> ui::terminal(:port(18023))
}
```

That authoring mechanic is superseded by ADR-009. The dual-surface **parity**
requirement is unchanged.

### Shared prop vocabulary

| Concern | Shape | Closed values / notes |
| --- | --- | --- |
| State bind | `:field(name)` | forms, tabs, filters |
| Display | `:value(expr)` | controlled inputs |
| Role | `:variant(...)` | `primary` \| `secondary` \| `destructive` \| `ghost` |
| Tone | `:tone(...)` | `default` \| `muted` \| `info` \| `success` \| `warning` \| `danger` |
| Size | `:size(...)` | `sm` \| `md` \| `lg` |
| Capability flags | bare flags | `:disabled`, `:sortable`, `:searchable`, `:selectable`, `:dense`, `:active`, `:submit`, `:dismissible`, `:collapsible` |

Unknown closed tokens are compile errors. `:field` stays a prop pattern;
`ui::field` is optional chrome around a control.

### Complete UI primitive catalog (38)

Do **not** duplicate the 38-line catalog in this ADR. Source of truth:

- `UI_COMPONENT_CATALOG` in [`crates/sil-core/src/ui.rs`](../crates/sil-core/src/ui.rs)
- Canonical rendered lines in [`crates/silc/templates/AGENTS.md`](../crates/silc/templates/AGENTS.md)
  and the root [README](../README.md)
- Drift fails `docs_conformance` tests

Every builtin is dual-surface (`web+terminal`).

### Page slots and child rules

- `ui::page` accepts optional slots `:app_bar` → `ui::app_bar`,
  `:side_panel` → `ui::side_panel`, `:footer` → `ui::footer`.
- Body children are constrained by each primitive's `ChildPolicy`
  (`none` / `any` / `anyOf(...)`).
- Author components compose catalog nodes and may emit events that parents wire
  with `:on(event => handler)`.

### Out-of-box components

Out-of-box UI capability lives in the compiler-owned primitive/component
catalog and codegen templates. There is no separate component-source standard
library or resolver. Author-defined components remain in application source,
and the presence of any primitive never selects application behavior.

### Non-goals

- Authoring React, Tailwind, OpenTUI, or CSS in Silc source
- Unrestricted framework escape hatches
- Surface-specific component catalogs
- Author-declared `serve()` / surface ops (owned by ADR-009)

## Consequences

- Shopping and other CRUD apps are expressible without a domain `PortalKind`
- Terminal is a first-class equal of web for every component
- Codegen emits one app worker + dual-surface modules from a shared IR
- Documentation drift from the catalog is a compile-test failure
