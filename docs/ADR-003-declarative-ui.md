# ADR-003: Declarative UI Surfaces (dual-surface components)

- **Status:** Accepted (0.2.0 dual-surface; 0.4.0 synthesized serving)
- **Date:** 2026-07-25
- **Updated:** 2026-10-04
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-002](ADR-002-silc-surface-syntax.md),
  [ADR-004](ADR-004-runtime-strengths.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ADR-017](ADR-017-ui-embed.md),
  [ADR-018](ADR-018-files-capability.md),
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

> **Superseded.** This block records an authoring mechanic that Silc no longer accepts. Do not write `method serve()`, `ui::web`, or `ui::terminal`. [ADR-009](ADR-009-compiler-synthesized-runtime.md) is the current rule: authors declare routes, and the compiler synthesizes both surfaces.

Earlier releases required authors to write:

```silc
method serve() {
    ui::web(:root(MyApp), :port(18080), :route("/"))
        ==> ui::terminal(:port(18023))
}
```

That authoring mechanic is superseded by ADR-009. The dual-surface **parity**
requirement is unchanged.

### Shared option vocabulary

| Concern | Shape | Closed values / notes |
| --- | --- | --- |
| State bind | `:field(name)` | forms, tabs, filters |
| Display | `:value(expr)` | controlled inputs |
| Role | `:variant(...)` | `primary` \| `secondary` \| `destructive` \| `ghost` |
| Tone | `:tone(...)` | `default` \| `muted` \| `info` \| `success` \| `warning` \| `danger` |
| Size | `:size(...)` | `sm` \| `md` \| `lg` |
| Capability flags | bare flags | `:disabled`, `:sortable`, `:searchable`, `:selectable`, `:dense`, `:active`, `:submit`, `:dismissible`, `:collapsible` |

Unknown closed tokens are compile errors. `:field` stays an option pattern;
`ui::field` is optional chrome around a control.

### Complete UI primitive catalog

Do **not** duplicate the full catalog in this ADR. Source of truth:

- `UI_COMPONENT_CATALOG` in [`crates/sil-core/src/ui.rs`](../crates/sil-core/src/ui.rs)
- Canonical rendered lines in [`crates/silc/templates/AGENTS.md`](../crates/silc/templates/AGENTS.md)
  and the root [README](../README.md)
- Drift fails `docs_conformance` tests

Every builtin is dual-surface (`web+terminal`).

### Addendum: `ui::embed` (2026-10-04)

`ui::embed` hosts an author-supplied URL. Web lowers to a sandboxed iframe;
terminal lowers to a title/URL/“Open in a browser” card. Full sandbox tokens,
WebGPU-in-iframe findings, and non-goals live in
[ADR-017](ADR-017-ui-embed.md). This addendum does **not** change ADR-012’s ban
on mixing `game` with `app` / `component` / `resource`.

### Addendum: `files` and `ui::file_browser` (2026-10-04)

`files "<dir>";` inside an `app` names one sysop-shared directory. `ui::file_browser`
lists it and downloads files as-is and folders as a zip, through synthesized
`GET /files/list` and `GET /files/download`. Terminal shows URLs instead of
transferring bytes. Full rules live in [ADR-018](ADR-018-files-capability.md).

### Page slots and child rules

- `ui::page` accepts optional slots `:app_bar` → `ui::app_bar`,
  `:side_panel` → `ui::side_panel`, `:footer` → `ui::footer`.
- Body children are constrained by each primitive's `ChildPolicy`
  (`none` / `any` / `anyOf(...)`).
- Author components compose catalog nodes and may emit events that parents wire
  with `:on(event => handler)`.
- `ui::table` accepts `:on(select(handler))`; the handler receives the selected
  row object on both web and terminal surfaces.

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
