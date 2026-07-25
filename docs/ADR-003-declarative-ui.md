# ADR-003: Declarative UI Surfaces (`ui::web`, `ui::terminal`, `view`)

- **Status:** Accepted (v1 partial implementation)
- **Date:** 2026-07-25
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-002](ADR-002-silc-surface-syntax.md),
  [ADR-004](ADR-004-runtime-strengths.md), [ARCHITECTURE.md](ARCHITECTURE.md)

## Context

Silc authors—humans and AI agents—express intent in dense Silc source. They must
not emit HTML, CSS, component frameworks, bundler configuration, or package
manifests. Early runnable portals used fixed compiler templates (`html::form` +
`http::serve`, then profile-selected `App.tsx` shells). Authors still need a
**common abstract language** for layout and controls—side panels, app bars,
toolbars, radio groups, button variants—without naming React, Tailwind, or
ShadCN.

## Decision

### Authoring surface

Silc declares UI capability through the `ui` namespace and optional `view`
subjects:

| Construct | Meaning | v1 status |
| --- | --- | --- |
| `ui::web(:port, :route)` | Browser UI bound to a Contract (profile template) | Runnable |
| `ui::web(:view(Name), :port, :route)` | Browser UI lowered from a named `view` | Runnable |
| `ui::terminal(:port)` | Telnet-compatible terminal UI bound to a Contract | Runnable alongside `ui::web` |
| `class X is view { method render() { … } }` | Semantic component tree | Runnable when referenced |

Canonical authoring with a custom view:

```silc
class FeedbackView is view {
    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Feedback"))),
            :side_panel(ui::side_panel(
                ui::nav_item(:label("Inbox"), :active)
            )),
            ui::form(
                ui::stack(
                    ui::text_input(:field(author), :label("Author")),
                    ui::radio_group(:field(rating), :options(["Good", "Okay", "Bad"])),
                    ui::toolbar(
                        ui::button(:label("Submit"), :variant(primary), :submit)
                    )
                )
            )
        )
    }
}

FeedbackRecord
    ==> ui::web(:view(FeedbackView), :port(18088), :route("/"))
```

When `:view` is omitted, the bound processor still selects a compiler-owned
application profile: `text::score` → feedback form, `llm::complete` → local chat
(ADR-005). Agents and new examples must prefer `ui::web` / `view`; Silc source
never names React, Tailwind, ShadCN, OpenTUI, CSS frameworks, or package
managers.

Compatibility aliases remain executable and lower to the same web profile:

```silc
FeedbackRecord
    ==> html::form()
    ==> http::serve(:port(18080), :route("/"))
```

### Semantic component catalog (web vertical slice)

Components are **capabilities**, not React class names. The compiler owns the
registry; unknown components or props are hard errors.

| Component | Role | Key props / slots |
| --- | --- | --- |
| `ui::page` | Root shell | slots `:app_bar`, `:side_panel`; children |
| `ui::app_bar` | Top bar | `:title` |
| `ui::side_panel` | Side navigation | children: `nav_item` |
| `ui::nav_item` | Nav entry | `:label`, optional `:active` |
| `ui::toolbar` | Action row | children: `button` |
| `ui::stack` / `ui::row` / `ui::grid` | Layout | children |
| `ui::card` | Grouped surface | children |
| `ui::heading` / `ui::text` | Copy | `:text`, optional `:level` |
| `ui::form` | Submit boundary | children |
| `ui::text_input` / `ui::textarea` | Fields | `:field` (Contract), optional `:label` |
| `ui::radio_group` | Exclusive choice | `:field`, `:options([...])`, optional `:label` |
| `ui::button` | Action | `:label`, optional `:variant(primary\|secondary\|destructive)`, `:size(sm\|md\|lg)`, `:submit` |
| `ui::chat` | Conversation thread + composer (LLM portals) | `:field` (Contract), optional `:label`, `:placeholder` |
| `ui::chat_history` | SQLite-backed turn history (LLM portals) | optional `:title`, `:collapsible` |
| `ui::search_input` | Deterministic text filter plus AI interpretation action | `:field`, optional `:label`, `:placeholder` |
| `ui::filter_bar` | Inventory filter toolbar | children: `search_input`, `button` |
| `ui::product_grid` | Read-only cards for compiler-seeded products | optional `:empty_text` |

Rules:

- A view roots at `ui::page` and declares exactly one `method render()`.
- `:field(name)` must name a field on the Contract feeding `ui::web`.
- Every interactive form view must include a `ui::button` with `:submit`
  (`ui::chat` counts — it embeds its own send button).
- `ui::chat` / `ui::chat_history` require an `llm::complete` portal.
- A referenced view containing `ui::product_grid` plus `llm::complete` selects
  the inventory portal. It must also contain `ui::search_input` and `ui::chat`.
- Inventory `/products` filtering is deterministic. `/ai_search` converts
  natural language to bounded filter JSON, and `/complete` caps and validates
  the visible-product context before local inference.
- LLM workers expose the latest 200 SQLite-backed turns through `/history`;
  generated chat views load them on mount so history survives page reloads.
- No raw CSS, Tailwind utilities, JSX, HTML, event handlers, or package names.
- Charts, data tables, dialogs, richer data sources, and terminal view lowering
  are catalog extensions—not authoring escapes into ShadCN.

Accessibility defaults (labels, radiogroup roles, current-page nav) are applied
by the compiler-owned primitives.

### Compiler-owned substrates

| Surface | Engine | Substrate | Author visibility |
| --- | --- | --- | --- |
| `ui::web` | Bun | React + Tailwind CSS + ShadCN-style primitives + Bun HTTP/API worker | None — generated under `.runtime/` |
| `ui::terminal` | Bun | Line-oriented TCP/telnet adapter now; OpenTUI for local rich TTY later | None — generated under `.runtime/` |

React is the blessed web substrate: one codegen path, one pinned dependency set.
Tailwind is the low-level styling primitive; ShadCN-style primitives
(compiler-vendored, not a user CLI) materialize the catalog. Authors express UI
intent in Silc; the compiler lowers views and Contracts into those tools.

### Capability ownership

```text
capability: ui.web
  → adapter: react-bun-v1
  → engine: bun
  → deps: compiler-pinned react, react-dom, tailwindcss, shadcn-style primitives

capability: ui.web.view
  → same adapter; App.tsx is deterministically lowered from UiView AST

capability: ui.terminal
  → adapter: telnet-bun-v1 (runnable remote terminal)
  → engine: bun
```

The compiler emits exact manifests and lock data into `.runtime/`, installs with
the Silc-owned Bun binary, and builds browser assets during `silc build`. Users
and AI tools never run npm/yarn/pnpm, `shadcn` CLI, or edit package
configuration.

### Routing

Namespace evidence `ui` selects Bun (tier 2). See
[ADR-004](ADR-004-runtime-strengths.md). Views are not modules and are not
routed; they are semantic subjects consumed by codegen.

### Non-goals for this slice

- Rich local OpenTUI renderer (the current terminal surface is telnet/TCP)
- Multiple competing web frameworks as user choices
- Authoring React components, Tailwind configs, ShadCN CLI, or OpenTUI trees in
  Silc projects
- SSR / hydration product features beyond a simple Bun-served SPA shell
- Charts, data tables, dialogs, and terminal `view` lowering
- Theme DSL beyond the compiler-shipped Silc theme pack
- Persisting every optional Contract field through the feedback IPC path
  (bound fields appear in the form payload; author/text remain the v1 ingest
  contract for `text::score` portals)

## Consequences

### Positive

- Agents stay in a semantic catalog humans can read and AIs can emit reliably.
- Custom layouts compose without forking portal templates.
- Substrates can be swapped later without changing Silc programs.
- Unsupported components fail at compile time instead of leaking framework code.

### Costs and risks

- The component catalog and ShadCN-style primitives become compiler maintenance
  surfaces.
- Frontend dependency install/build adds latency to the first runnable build.
- Telnet is unencrypted and unauthenticated in v1; the listener is loopback-only.
