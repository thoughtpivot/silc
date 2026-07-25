# Silc project guidance for AI tools

This directory is a **Silc 0.2.0** project. Silc (said like “silk”) is an
AI-native language and local Rust compiler. Edit `.silc` source only.

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
- Declarative UI: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-003-declarative-ui.md
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
| `class X { has T $.f; }` | **Contract** — typed data schema |
| `class X is component` | **Component** — props, `has state`, slots, `emit`, handlers, `render()` |
| `class X is resource` | **Resource** — `query` / `mutation` data layer (CRUD over SQLite) |
| `class X is app` | **App** — `route` table + `method serve()` |
| `class X is service\|processor\|sink` | Optional backend modules |

Removed in 0.2.0 (do not use):

- `is view`
- Portal profiles / `PortalKind` (Feedback, LlmChat, Inventory, Shopping, …)
- Contract-left-of-`ui::web` portal binding
- Separate `stdlib/` component resolver or seeded domain catalogs

High-level UIs (forms, chat, shop) are **compositions** of components and
resources — not compiler modes that take over the application.

## Dual-surface UI (required)

Authors write **one** semantic component tree. The compiler lowers it to:

- `ui::web` → React (compiler-owned)
- `ui::terminal` → terminal adapter (compiler-owned)

Every UI app **must** declare both surfaces in `serve()`. No component may be
web-only or terminal-only. Never write HTML, CSS, React, Tailwind, OpenTUI,
ShadCN trees, or bundler config in Silc source.

Built-in primitives (`ui::page`, `ui::button`, `ui::form`, …) are compiler-owned
catalog entries with web + terminal contracts. Author-defined reusable
components live in app `.silc` source beside the rest of the program.

## Valid patterns

### Component with state and events

```silc
class HomePage is component {
    has state Str $.text = "";
    method render() {
        ui::page(
            ui::form(:on(submit(on_submit)),
                ui::textarea(:field(text), :label("Note")),
                ui::button(:label("Submit"), :variant(primary), :submit)
            )
        )
    }
    method on_submit() { submit(); }
}
```

### App with routes and dual-surface serve

```silc
class MyApp is app {
    route "/" => HomePage;
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
    mutation create(Product $item) {
        $item ==> resource::create(:table(products))
    }
}
```

Wire handlers with `:on(click(handler))`, `:on(submit(handler))`, navigation
with `ui::nav_item(:to("/path"))`, collections with `for $.items -> $item { … }`,
and conditionals with `when expr { … }`.

## Runnable operations (0.2.0)

Executable today:

`ui::web`, `ui::terminal`, `service::http`, `text::score`, `llm::complete`,
`ipc::publish`, `store::sqlite`, `store::commit`,
`resource::list`, `resource::get`, `resource::create`, `resource::update`,
`resource::delete`.

Stub-only (parse/route/emit, do not run): `http::get`, `tensor::*`, `pandas::*`,
and other non-registry ops. Mixing stub-only ops into a runnable graph is a
**compile error**.

## Rules for agents

1. Edit only `.silc` (and conforming `.raku`) source.
2. Prefer author-defined `is component` + `is app` over inventing profiles.
3. Always serve UI with **both** `ui::web` and `ui::terminal`.
4. Use Contracts + resources for persistence; do not hard-code seeded product DBs.
5. Do not create a `stdlib/` directory or escape into React/OpenTUI/CSS.
6. Do not invent new compiler portal kinds to make an app run.
7. Validate with `silc build`; report errors instead of patching `.runtime/`.
