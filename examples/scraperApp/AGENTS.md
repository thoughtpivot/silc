<!-- BEGIN SILC_AGENTS_TEMPLATE -->
# Silc project guidance for AI tools

This directory is a **Silc 0.2.0** project. Silc (said like “silk”) is an
independent intent language with a Raku-inspired surface and a local Rust
compiler. Edit `.silc` source only (not `.raku` / `.sil`).

**Never** hand-edit `.runtime/` or `.silc/runtimes.lock.json`. Those are
compiler-owned outputs.

## Engines are owned by Silc

Silc provisions pinned **Bun**, **CPython**, and **Go** into `~/.silc/runtimes/`
and writes `.silc/runtimes.lock.json`. Do not install, choose, or configure those
engines. Do not invent `package.json`, Vite, Cargo workers, or Go modules for
application code.

`.runtime/` holds generated workers, IPC, SQLite data, UI bundles, and logs.

## Authoritative docs

- Surface syntax: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-002-silc-surface-syntax.md
- Pipeline feeds (`==>`): https://github.com/thoughtpivot/silc/blob/main/docs/ADR-007-pipeline-feeds.md
- Declarative UI: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-003-declarative-ui.md
- Local LLM: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-005-local-llm-complete.md
- Scrape: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-006-scrape-namespace.md
- Architecture: https://github.com/thoughtpivot/silc/blob/main/docs/ARCHITECTURE.md
- Examples: https://github.com/thoughtpivot/silc/tree/main/examples

## Workflow

```bash
silc init myapp
cd myapp
silc build main.silc          # compile + validate
silc main.silc                # build and run when mode is runnable
```

Treat compiler diagnostics as authoritative. Prefer `silc build` after each
meaningful edit. Stop and report limits instead of inventing substrates.

## Silc 0.2.0 authoring model

| Construct | Role |
| --- | --- |
| `@version("0.2.0")` | Optional program version annotation |
| `subset Name of Base where { … }` | Semantic type alias; v1 `where` predicates (Str): `.contains` / `.starts-with` / `.ends-with` (ADR-002) |
| `class X { has T $.f; }` | **Contract** — typed data schema |
| `class X is component` | **Component** — props, `has state`, slots, `emit`, handlers, `render()` |
| `class X is resource` | **Resource** — `query` / `mutation` data layer (CRUD over SQLite) |
| `class X is app` | **App** — `route` table + `method serve()` |
| `class X is service\|processor\|sink` | Optional backend modules |
| `==>` | Pipeline feed between values and `ns::op(...)` calls |

Removed in 0.2.0 (do not use):

- `is view`
- Portal profiles / `PortalKind` (Feedback, LlmChat, Inventory, Shopping, …)
- Contract-left-of-`ui::web` portal binding
- Separate `stdlib/` component resolver or seeded domain catalogs

High-level UIs (forms, chat, shop) are **compositions** of components and
resources — not compiler modes that take over the application.

## Types

Built-in named types: `Str`, `UUID`, `num32`, `num64`, `int32`, `int64`,
`Bool`, `Int`.

Also valid: contract names, subset names, arrays (`[Product]`), and fixed
vectors (`Vec[num32; 768]`).

## Expressions and control flow

Supported in handlers / templates:

- Literals, `$name` / `$.field`, member access, calls, `Type.new(:field(value))`
- Arithmetic / comparison / boolean ops, unary `!` / `-`
- Assignment to component state: `$.field = expr;`
- Lists: `[a, b]`
- `emit event(payload)`, `navigate("/path")`, `await expr`
- Template control: `when expr { … } else { … }`, `for expr -> $item { … }`

## Dual-surface UI (required)

Authors write **one** semantic component tree. The compiler lowers it to:

- `ui::web` → React/Tailwind (compiler-owned)
- `ui::terminal` → OpenTUI (compiler-owned); TCP telnet CLI is a remote fallback

Every UI app **must** declare both surfaces in `serve()` with distinct ports.
No component may be web-only or terminal-only. Never write HTML, CSS, React,
Tailwind, OpenTUI, ShadCN trees, or bundler config in Silc source.

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

## Valid patterns

### Component with state and events

```silc
class HomePage is component {
    has state Str $.text = "";
    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Notes"))),
            ui::form(:on(submit(on_submit)),
                ui::textarea(:field(text), :label("Note")),
                ui::button(:label("Submit"), :variant(primary), :submit)
            )
        )
    }
    method on_submit() { submit(); }
}
```

### Author component composition + emit forwarding

```silc
class ItemCard is component {
    has Item $.item;
    emit remove(Item);
    method render() {
        ui::card(
            ui::heading(:text($.item.name)),
            ui::button(:label("Delete"), :variant(destructive), :on(click(on_remove)))
        )
    }
    method on_remove() { emit remove($.item); }
}

# parent:
ItemCard(:item($item), :on(remove => on_delete))
```

### Resource queries on a component

```silc
class BrowsePage is component {
    query $.items = Inventory.list();
    method render() {
        ui::table(
            :rows($.items),
            :columns(["name", "category"]),
            :sortable,
            :searchable
        )
    }
}
```

### App with routes and dual-surface serve

```silc
class MyApp is app {
    route "/" => HomePage;
    route "/admin" => AdminPage;
    method serve() {
        ui::web(:root(MyApp), :port(18080), :route("/"))
            ==> ui::terminal(:port(18023))
    }
}
```

### Resource CRUD

```silc
class Products is resource {
    has Str $.table = "products";
    query list() -> [Product] {
        Product ==> resource::list(:table(products))
    }
    query get(Product $item) -> Product {
        $item ==> resource::get(:table(products))
    }
    mutation create(Product $item) {
        $item ==> resource::create(:table(products))
    }
    mutation update(Product $item) {
        $item ==> resource::update(:table(products))
    }
    mutation delete(Product $item) {
        $item ==> resource::delete(:table(products))
    }
}
```

Derived HTTP (compiler-owned): `GET/POST /api/{table}`,
`GET/PUT/DELETE /api/{table}/:id`.

### Chat with silclm

```silc
ui::chat(
    :value($.prompt),
    :session($.active_session),          # multi-session history
    :context($.items),                   # live grounding snapshot
    :persona("You are …, built on silclm."),
    :on(send(on_send))
)
```

`:context` and `:persona` ride the `/complete` ingest frame and are **not**
persisted into chat history.

### Processor + sink (score or LLM)

```silc
class Assistant is processor {
    method complete(ChatRecord $record) {
        $record.prompt ==> llm::complete()
    }
}
class ChatDb is sink is storage(SQLite) {
    method persist(ChatRecord $record) {
        $record ==> ipc::publish() ==> store::sqlite(:table(chats)) ==> store::commit()
    }
}
```

`text::score` and `llm::complete` cannot both appear in one program. Each needs
exactly one processor and one `is storage(SQLite)` sink with the
`ipc::publish ==> store::sqlite ==> store::commit` chain.

### API-only service

```silc
class Api is service {
    method create(Note $note) {
        $note ==> service::http(:port(8080), :route("/notes"), :method(POST))
    }
}
```

API-only programs must not declare processor/sink modules.

Wire handlers with `:on(click(handler))`, `:on(submit(handler))`, navigation
with `ui::nav_item(:to("/path"))`, collections with `for $.items -> $item { … }`,
and conditionals with `when expr { … }`.

## Runnable operations (0.2.0)

Executable today (registry in `sil-core`):

`ui::web`, `ui::terminal`, `service::http`, `text::score`, `llm::complete`,
`ipc::publish`, `store::sqlite`, `store::commit`,
`resource::list`, `resource::get`, `resource::create`, `resource::update`,
`resource::delete`,
`scrape::page`, `scrape::site`, `scrape::select`, `scrape::render`,
`scrape::extract`.

Local LLM chat uses **silclm** (default catalog id). Prefer
`llm::complete(:model("silclm"))` or omit `:model`. Do not invent Ollama,
OpenAI, or ad-hoc GGUF paths in `.silc`. Legacy alias `llama3.2-1b` resolves to
`silclm` for one release.

Scraping uses **`scrape::*`** (ADR-006). Authors never name Bun, Colly, or
Playwright. Prefer `scrape::site` for crawls and `scrape::page` /
`scrape::select` for single pages. Do **not** use stub `http::get` /
`html::extract_body` in runnable programs — migrate to `scrape::*`.

Stub-only namespaces (parse/route/emit, do not run): `http`, `html`, `tensor`,
`numpy`, `pandas`, `ws`, `sys`, `schema`, `payload`, `json`, plus non-registry
ops under runnable namespaces. Mixing stub-only ops into a runnable graph is a
**compile error**.

## Generated runtime surfaces

Compiler-owned (do not invent alternatives):

- `POST /submit` — form `submit()` handlers (also scrape jobs when `scrape::*` is present)
- `POST /scrape` — explicit scrape ingest when `scrape::*` is present
- `POST /complete` — chat / `*.complete()` processors
- `GET|POST|PUT|DELETE /api/{table}` — resource queries/mutations
- Web: React app served by Bun
- Terminal: OpenTUI app (local TTY); TCP telnet CLI on the terminal port as remote fallback

## Validation constraints agents must respect

1. UI apps require `is app`, non-empty `route`s, `serve()` with both
   `ui::web(:root(...))` and `ui::terminal(:port(...))`, distinct ports.
2. Every builtin UI node must use catalog props/events; unknown props/events fail.
3. Closed enums (`:variant`, `:tone`, `:size`) reject unknown tokens.
4. Resource `query` bindings must reference real resource query methods.
5. Do not mix `text::score` and `llm::complete`.
6. Do not mix `scrape::*` with `text::score`. Scrape pipelines may use
   `llm::complete` for grounded SilcLM summaries.
7. Do not mix executable and stub-only ops in one runnable graph.
8. Default fallback ports if omitted: web `18088`, terminal `18023`, API `8080`
   (examples often pick explicit ports instead).

## Rules for agents

1. Edit only `.silc` source (`.raku` / `.sil` are not accepted).
2. Prefer author-defined `is component` + `is app` over inventing profiles.
3. Always serve UI with **both** `ui::web` and `ui::terminal`.
4. Use Contracts + resources for persistence; do not hard-code seeded product DBs.
5. Do not create a `stdlib/` directory or escape into React/OpenTUI/CSS.
6. Do not invent new compiler portal kinds to make an app run.
7. Stay inside the UI catalog and runnable op set above.
8. Validate with `silc build`; report errors instead of patching `.runtime/`.
<!-- END SILC_AGENTS_TEMPLATE -->
