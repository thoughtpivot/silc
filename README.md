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

Silc is **pre-1.0**. Release 0.2.0 replaces profile-selected portals with
author-defined components, resources, apps, and mandatory dual-surface UI
(web + terminal from one semantic tree).

**Shipped today**

- Parse → validate → deterministic Tier 1/2 route → codegen
- `is component` / `is resource` / `is app` (props, state, slots, events, queries, routes)
- Dual-surface UI: the same component graph lowers to **web** and **terminal**
- Generic resource CRUD over SQLite; optional `text::score` / `llm::complete`
- `service::http` API-only programs (Go/Gin)
- Runnable `silc init` scaffold (component + app + both surfaces)
- Compiler-owned Bun 1.2.18, CPython 3.12.12, and Go 1.23.6 under `~/.silc/runtimes/`
- Semantic release automation (release-plz); SemVer 0.x with Conventional Commits

**Removed in 0.2.0**

- `PortalKind` profiles (Feedback / LlmChat / Inventory)
- `is view` and Contract-left-of-`ui::web` portal binding
- Seeded inventory catalogs and separate component-source `stdlib/`

**Stub-only today**

- Broader pipeline ops (`http::get`, `tensor::`, `pandas::`, …) parse, route,
  and emit inspectable stubs; they do not execute

---

## Quick start

```bash
cargo install --path crates/silc

silc init myapp
cd myapp
silc build main.silc   # runnable dual-surface app
silc main.silc         # run it

# web:      http://127.0.0.1:18080
# terminal: telnet 127.0.0.1 18023
```

`silc init` writes `main.silc`, `AGENTS.md`, `.gitignore`, and a runtime lock.
The scaffold is a small note-form app with an author-defined component, an
`is app` route table, and both `ui::web` and `ui::terminal`.

---

## How Silc 0.2.0 works

```text
Contracts + Components + Resources + App routes
        │
        ▼
   semantic IR (state, events, queries, actions)
        │
        ├─► React web adapter
        ├─► terminal adapter
        ├─► Bun action / resource HTTP
        ├─► Python processor (score / LLM)
        └─► Go SQLite persistence
```

Authors express intent in Silc. The compiler owns substrates (React, Bun,
CPython, Go, IPC). Agents and humans edit `.silc` only — never `.runtime/`.

| Subject | Purpose |
| --- | --- |
| Contract | Typed record schema (`class Note { has Str $.text; }`) |
| Component | Reusable UI: props, `has state`, slots, `emit`, handlers, `render()` |
| Resource | `query` / `mutation` data layer → SQLite CRUD HTTP |
| App | `route "/path" => Page;` + `serve()` with **both** surfaces |
| Module | Optional `service` / `processor` / `sink` pipelines |

Built-in primitives (`ui::page`, `ui::button`, `ui::form`, …) are compiler-owned
catalog entries with web and terminal contracts. Author-defined components live
in app source. There is no separate component-source stdlib or resolver.

---

## Language surface (concise)

### Component state and events

```raku
class HomePage is component {
    has state Str $.author = "";
    has state Str $.text = "";

    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("My Silc App"))),
            ui::form(:on(submit(on_submit)),
                ui::text_input(:field(author), :label("Author")),
                ui::textarea(:field(text), :label("Note")),
                ui::button(:label("Submit"), :variant(primary), :submit)
            )
        )
    }

    method on_submit() { submit(); }
}
```

### App routes and dual-surface serve

```raku
class MyApp is app {
    route "/" => HomePage;

    method serve() {
        ui::web(:root(MyApp), :port(18080), :route("/"))
            ==> ui::terminal(:port(18023))
    }
}
```

### Resources, collections, and navigation

```raku
class Products is resource {
    has Str $.table = "products";
    query list() -> [Product] {
        Product ==> resource::list(:table(products))
    }
    mutation create(Product $item) {
        $item ==> resource::create(:table(products))
    }
}

# in render():
ui::nav_item(:label("Shop"), :to("/"))
for $.products -> $product {
    ui::card(ui::heading(:text($product.name)))
}
when $.products {
    ui::empty(:text("No products yet."))
}
```

---

## Examples

Examples are **standalone Silc projects** (same shape as `silc init`). See
[`examples/README.md`](examples/README.md).

| App | Purpose |
| --- | --- |
| [`examples/chatApp/`](examples/chatApp/) | Multi-session local chat via **silclm** |
| [`examples/inventoryApp/`](examples/inventoryApp/) | Inventory CRUD + browse/admin + grounded silclm assistant |

Runnable operations are gated by an explicit registry in
[`crates/sil-core/src/operation.rs`](crates/sil-core/src/operation.rs):

`ui::web`, `ui::terminal`, `service::http`, `text::score`, `llm::complete`,
`ipc::publish`, `store::sqlite`, `store::commit`,
`resource::list|get|create|update|delete`.

Local chat uses **silclm** (Silc's owned model identity; v0 is a pinned Llama
3.2 1B GGUF). Omit `:model` or pass `:model("silclm")`. See
[ADR-005](docs/ADR-005-local-llm-complete.md).

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

Workspace crates: `sil-core`, `sil-lexer`, `sil-parser`, `sil-router`,
`sil-codegen`, `sil-ipc`, `silc` (CLI + supervisor), `sil-training`
(provider-neutral silclm dataset harness).

Per-app output lands in `{workdir}/.runtime/{program}/` (gitignored). Engines
stay in `~/.silc/runtimes/`. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## For AI agents

`silc init` copies [`crates/silc/templates/AGENTS.md`](crates/silc/templates/AGENTS.md)
into the project. That file is the operational contract:

- Edit Silc source only; never patch `.runtime/`
- Prefer components + apps + resources over inventing portal profiles
- Always declare both `ui::web` and `ui::terminal`
- Stay inside the compiler-owned UI catalog and runnable op set
- Validate with `silc build`; report limits instead of escaping to React/OpenTUI

---

## Versioning

Silc remains **pre-1.0**. Releases follow SemVer 0.x:

- Breaking language/compiler changes → minor bump (`0.x` → `0.(x+1)`)
- Backward-compatible features → minor bump
- Fixes → patch bump

`1.0.0` is reserved for a future stability milestone. Changelog and release PRs
are managed with [release-plz](release-plz.toml) and Conventional Commits.

---

## Development

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace -- --test-threads=1
```

CI runs fmt, check, lib tests, codegen smoke, scored_form / shopping builds, and
concurrent `/submit` POSTs with SQLite checks.

---

## Documentation

| Doc | Topic |
| --- | --- |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Subject model and crate layout |
| [docs/ADR-001-runtime-and-ipc.md](docs/ADR-001-runtime-and-ipc.md) | Engines and IPC |
| [docs/ADR-002-silc-surface-syntax.md](docs/ADR-002-silc-surface-syntax.md) | Language surface |
| [docs/ADR-003-declarative-ui.md](docs/ADR-003-declarative-ui.md) | Dual-surface UI |
| [docs/ADR-004-runtime-strengths.md](docs/ADR-004-runtime-strengths.md) | Why Bun / CPython / Go |
| [docs/ADR-005-local-llm-complete.md](docs/ADR-005-local-llm-complete.md) | Local LLM completions |
| [docs/SILC-IPC-ABI-v1.md](docs/SILC-IPC-ABI-v1.md) | Shared buffer ABI |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |

---

## License

Apache-2.0 — see [LICENSE](LICENSE).
