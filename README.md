# Silc — Intent-Native Polyglot Application Compiler

ThoughtPivot’s **Silc** (pronounced *silk*) is a compact, contract-bound
language and local Rust compiler. You write a short intent program. The
compiler validates it, routes each module to the right engine, provisions
pinned Bun / CPython / Go runtimes, emits workers, and runs the app — without
asking an LLM to invent React, Python, Go, manifests, or glue.

```text
.silc intent  →  Rust compiler  →  Bun · CPython · Go workers  →  mmap IPC + UDS
```

---

## Status (0.2.0)

Silc is **pre-1.0**. This release replaces profile-selected portals with
author-defined components, resources, apps, and dual-surface UI lowering.

**Shipped today**

- Parse → validate → deterministic Tier 1/2 route → codegen
- Author-defined `is component` / `is resource` / `is app` (typed props, state,
  slots, events, queries, routes)
- Dual-surface UI: every component graph lowers to **web** and **terminal**
- Generic resource CRUD over SQLite; optional `text::score` / `llm::complete`
- `service::http` API-only programs (Go/Gin)
- Compiler-owned Bun 1.2.18, CPython 3.12.12, and Go 1.23.6 under `~/.silc/runtimes/`
- Semantic release automation (release-plz); SemVer 0.x with Conventional Commits

**Removed in 0.2.0**

- `PortalKind` profiles (Feedback / LlmChat / Inventory)
- `is view` and Contract-left-of-`ui::web` portal binding
- Seeded inventory product catalogs owned by the compiler

**Stub-only today**

- Broader pipeline ops (`http::get`, `tensor::`, `pandas::`, …) parse, route,
  and emit inspectable stubs; they do not execute

---

## Quick start

```bash
cargo install --path crates/silc

silc init myapp
cd myapp
silc build main.silc

# Dual-surface scored form
silc /path/to/silc/examples/scored_form.silc
# web:      http://127.0.0.1:18080
# terminal: telnet 127.0.0.1 18023
```

---

## A runnable example

[`examples/scored_form.silc`](examples/scored_form.silc) declares components and
an app with **both** surfaces — no portal profile:

```silc
@version("0.2.0")

class FeedbackPage is component {
    has state Str $.author = "";
    has state Str $.text = "";
    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Feedback"))),
            ui::form(:on(submit(on_submit)),
                ui::text_input(:field(author), :label("Author")),
                ui::textarea(:field(text), :label("Feedback")),
                ui::button(:label("Submit"), :variant(primary), :submit)
            )
        )
    }
    method on_submit() { submit(); }
}

class FeedbackApp is app {
    route "/" => FeedbackPage;
    method serve() {
        ui::web(:root(FeedbackApp), :port(18080), :route("/"))
            ==> ui::terminal(:port(18023))
    }
}
```

Authors never write HTML, React, `package.json`, Python packaging, or Go modules.
Those are compiler substrates. See [ADR-003](docs/ADR-003-declarative-ui.md).

---

## Examples (0.2.0)

| Program | Purpose |
| --- | --- |
| [`examples/components.silc`](examples/components.silc) | Props, state, events, dual surfaces |
| [`examples/scored_form.silc`](examples/scored_form.silc) | Form + `text::score` + SQLite (CI path) |
| [`examples/chat_assistant.silc`](examples/chat_assistant.silc) | Local `llm::complete` chat |
| [`examples/shopping_app.silc`](examples/shopping_app.silc) | Resources, routes, cart — no Shopping profile |
| [`examples/http_api.silc`](examples/http_api.silc) | `service::http` → Go/Gin |
| [`examples/data_pipeline.silc`](examples/data_pipeline.silc) | Stub routing across Bun / Python / Go |

Out-of-box UI capability is compiler-owned in the primitive catalog and
codegen templates. Author-defined reusable components live directly in app
source; Silc has no separate component-source stdlib or resolver.

Runnable operations are gated by an explicit registry in
[`crates/sil-core/src/operation.rs`](crates/sil-core/src/operation.rs).

Mixing stub-only ops into a runnable graph is a compile error — by design.

---

## Architecture

```text
Silc source (.silc)
        │
        ▼
   sil-lexer → sil-parser → sil-core subjects
        │     (Contract · Component · Resource · App · Module · Pipeline)
        ▼
   sil-router   Tier 1 (kind + traits) + Tier 2 (namespaces)
        ▼
   sil-codegen  stub emit  or  runnable workers + dual-surface UI lowering
        ▼
   silc supervisor
        ├── Bun  (web + terminal + resource HTTP)
        ├── CPython (scoring / local LLM)
        ├── Go (SQLite persistence / HTTP API)
        └── sil-ipc mmap slots + UDS
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## Versioning

Silc remains **pre-1.0**. Releases follow SemVer 0.x:

- Breaking language/compiler changes → minor bump (`0.x` → `0.(x+1)`)
- Backward-compatible features → minor bump
- Fixes → patch bump

`1.0.0` is reserved for a future stability milestone. Changelog and release PRs
are managed with [release-plz](release-plz.toml) and Conventional Commits.

---

## License

Apache-2.0 — see [LICENSE](LICENSE).
