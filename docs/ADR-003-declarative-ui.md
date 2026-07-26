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

Canonical one-liner per primitive (props / events / slots / children /
surfaces). Source of truth: `UI_COMPONENT_CATALOG` in
[`crates/sil-core/src/ui.rs`](../crates/sil-core/src/ui.rs). The same lines
appear in [`crates/silc/templates/AGENTS.md`](../crates/silc/templates/AGENTS.md)
and the root [README](../README.md).

Every builtin is dual-surface (`web+terminal`). Lines below are the canonical
API contract (props / events / slots / children).

#### Shell and navigation

- `ui::page` — props: none; events: none; slots: `app_bar`→`app_bar`, `side_panel`→`side_panel`, `footer`→`footer`; children: anyOf(`stack`, `row`, `grid`, `card`, `heading`, `text`, `form`, `text_input`, `textarea`, `radio_group`, `select`, `checkbox`, `switch`, `field`, `button`, `toolbar`, `chat`, `chat_history`, `search_input`, `filter_bar`, `collection`, `list`, `table`, `badge`, `alert`, `divider`, `section`, `description_list`, `tabs`, `dialog`, `loading`, `empty`, `nav_item`); surfaces: web+terminal
- `ui::app_bar` — props: `title` (required); events: none; slots: none; children: none; surfaces: web+terminal
- `ui::side_panel` — props: none; events: none; slots: none; children: anyOf(`nav_item`); surfaces: web+terminal
- `ui::nav_item` — props: `label` (required), `to?`, `active?` (flag); events: `click`; slots: none; children: none; surfaces: web+terminal
- `ui::toolbar` — props: none; events: none; slots: none; children: anyOf(`button`); surfaces: web+terminal
- `ui::footer` — props: none; events: none; slots: none; children: any; surfaces: web+terminal

#### Layout

- `ui::stack` — props: none; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::row` — props: none; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::grid` — props: none; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::card` — props: none; events: none; slots: `actions`→`row`; children: any; surfaces: web+terminal
- `ui::section` — props: `title?`, `description?`; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::divider` — props: `label?`; events: none; slots: none; children: none; surfaces: web+terminal
- `ui::heading` — props: `text` (required), `level?`; events: none; slots: none; children: none; surfaces: web+terminal
- `ui::text` — props: `text` (required); events: none; slots: none; children: none; surfaces: web+terminal

#### Forms

- `ui::form` — props: none; events: `submit`; slots: none; children: anyOf(`stack`, `row`, `grid`, `card`, `heading`, `text`, `text_input`, `textarea`, `radio_group`, `select`, `checkbox`, `switch`, `field`, `button`, `toolbar`, `badge`, `alert`, `divider`, `section`, `loading`, `empty`); surfaces: web+terminal
- `ui::text_input` — props: `field?`, `value?`, `label?`, `placeholder?`, `disabled?` (flag); events: `input`, `change`; slots: none; children: none; surfaces: web+terminal
- `ui::textarea` — props: `field?`, `value?`, `label?`, `disabled?` (flag); events: `input`, `change`; slots: none; children: none; surfaces: web+terminal
- `ui::radio_group` — props: `field?`, `value?`, `options` (required), `label?`, `disabled?` (flag); events: `change`; slots: none; children: none; surfaces: web+terminal
- `ui::select` — props: `field?`, `value?`, `options` (required), `label?`, `placeholder?`, `disabled?` (flag); events: `change`; slots: none; children: none; surfaces: web+terminal
- `ui::checkbox` — props: `field?`, `label` (required), `checked?`, `disabled?` (flag); events: `change`; slots: none; children: none; surfaces: web+terminal
- `ui::switch` — props: `field?`, `label` (required), `checked?`, `disabled?` (flag); events: `change`; slots: none; children: none; surfaces: web+terminal
- `ui::field` — props: `label?`, `hint?`, `error?`; events: none; slots: none; children: anyOf(`stack`, `row`, `grid`, `card`, `heading`, `text`, `text_input`, `textarea`, `radio_group`, `select`, `checkbox`, `switch`, `field`, `button`, `toolbar`, `badge`, `alert`, `divider`, `section`, `loading`, `empty`); surfaces: web+terminal
- `ui::button` — props: `label` (required), `variant?`, `size?`, `submit?` (flag), `active?`, `disabled?` (flag); events: `click`; slots: none; children: none; surfaces: web+terminal

#### Chat and search

- `ui::chat` — props: `field?`, `value?`, `label?`, `placeholder?`, `session?`, `loading?`, `error?`, `context?`, `persona?`; events: `send`; slots: none; children: none; surfaces: web+terminal
- `ui::chat_history` — props: `title?`, `items?`, `collapsible?` (flag); events: none; slots: none; children: none; surfaces: web+terminal
- `ui::search_input` — props: `field?`, `value?`, `label?`, `placeholder?`; events: `input`, `submit`; slots: none; children: none; surfaces: web+terminal
- `ui::filter_bar` — props: none; events: none; slots: none; children: anyOf(`search_input`, `button`, `text_input`); surfaces: web+terminal

#### Data display

- `ui::collection` — props: `items` (required), `empty_text?`; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::list` — props: `items?`; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::table` — props: `rows` (required), `columns` (required), `empty_text?`, `filter_field?`, `filter_column?`, `filter_all?`, `sortable?` (flag), `searchable?` (flag), `selectable?` (flag), `dense?` (flag); events: none; slots: none; children: none; surfaces: web+terminal
- `ui::description_list` — props: `items` (required); events: none; slots: none; children: none; surfaces: web+terminal

#### Feedback and overlays

- `ui::badge` — props: `text` (required), `tone?`; events: none; slots: none; children: none; surfaces: web+terminal
- `ui::alert` — props: `text` (required), `title?`, `tone?`, `dismissible?` (flag); events: `dismiss`; slots: none; children: none; surfaces: web+terminal
- `ui::tabs` — props: `field?`, `value?`; events: `change`; slots: none; children: anyOf(`tab`); surfaces: web+terminal
- `ui::tab` — props: `label` (required), `value` (required); events: none; slots: none; children: any; surfaces: web+terminal
- `ui::dialog` — props: `open` (required), `title?`; events: `confirm`, `cancel`; slots: none; children: any; surfaces: web+terminal
- `ui::loading` — props: `text?`; events: none; slots: none; children: none; surfaces: web+terminal
- `ui::empty` — props: `text?`; events: none; slots: none; children: none; surfaces: web+terminal

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

## Consequences

- Shopping and other CRUD apps are expressible without a domain `PortalKind`
- Terminal is a first-class equal of web for every component
- Codegen emits one app worker + dual-surface modules from a shared IR
- Documentation drift from the catalog is a compile-test failure
