# Silc — Intent-Native Polyglot Application Compiler

ThoughtPivot’s **Silc** (pronounced *silk*) is an independent intent language
with a Raku-inspired surface and a local Rust compiler. It is not a Raku
subset. You write a short `.silc` intent program. The compiler validates it,
routes each module to the right engine, provisions pinned Bun / CPython / Go
runtimes, emits workers, and runs the app — without asking an LLM to invent
React, Python, Go, manifests, or glue.

```text
.silc intent  →  Rust compiler  →  Bun · CPython · Go workers  →  mmap IPC + UDS
```

---

## Status (0.4.0)

Silc is **pre-1.0**. Release 0.4.0 makes runtime mechanics
compiler-synthesized: authors declare intent (`app` routes, resource
capabilities, processors, domain ops); the compiler owns dual-surface UI,
SQLite persistence, and IPC/store staging ([ADR-009](docs/ADR-009-compiler-synthesized-runtime.md)).

**Shipped today**

- Parse → validate → deterministic Tier 1/2 route → codegen
- Declaration-based `component` / `resource Name for Contract` / `app` routes
- Dual-surface UI synthesized from `app` (web + terminal; no author `serve()`)
- Generic resource CRUD over SQLite; optional `text::score` / `llm::complete`
- Runnable `scrape::*` and closed MiniLM `tensor::*` pipeline
  ([ADR-006](docs/ADR-006-scrape-namespace.md),
  [ADR-010](docs/ADR-010-tensor-minilm-pipeline.md))
- `service::http` API-only programs
- Runnable `silc init` scaffold; experimental `silc assist` ([ADR-008](docs/ADR-008-recursive-silclm-assist.md))
- Compiler-owned Bun 1.2.18, CPython 3.12.12, and Go 1.23.6 under `~/.silc/runtimes/`
- Semantic release automation (release-plz); SemVer 0.x with Conventional Commits

**Removed in 0.4.0 (author source)**

- `method serve()`, author `ui::web` / `ui::terminal` ops, `sink` modules
- `ipc::*`, `store::*`, and `resource::*` pipelines

**Removed in 0.2.0**

- `PortalKind` profiles (Feedback / LlmChat / Inventory)
- `is view` and Contract-left-of-`ui::web` portal binding
- Seeded inventory catalogs and separate component-source `stdlib/`

**Stub-only today**

- Broader pipeline ops (`http::get`, `html::*`, `numpy::*`, `pandas::*`, …)
  parse, route, and emit inspectable stubs; they do not execute

---

## Quick start

```bash
cargo install --path crates/silc --force

silc init myapp
cd myapp
silc build main.silc   # runnable dual-surface app
silc main.silc         # run it (OpenTUI attaches in a real TTY)

# web:      http://127.0.0.1:18088  (override SILC_HTTP_PORT)
# terminal: OpenTUI on the local TTY (primary)
# fallback: telnet 127.0.0.1 18023  (remote TCP CLI; SILC_TERMINAL_PORT)
```

`silc init` writes `main.silc`, `AGENTS.md`, `.gitignore`, and a runtime lock.
The scaffold is a small note-form app with an author-defined component and an
`app` route table; dual-surface web/terminal serving is synthesized.

Experimental authoring help (ADR-008) explores Silc examples via a closed-tool
recursive loop around **silclm**:

```bash
silc assist "dual-surface notes app with submit" --out notes.silc
```

OpenTUI is the **primary** terminal surface when stdin/stdout are a TTY (or
`SILC_FORCE_OPENTUI=1`). The TCP telnet CLI on the terminal port remains a
remote fallback for non-TTY sessions.

---

## How Silc 0.4.0 works

```text
Contracts + Components + Resources + App routes
        │
        ▼
   semantic IR (state, events, queries, actions)
        │
        ├─► React web adapter
        ├─► OpenTUI terminal adapter (+ TCP CLI fallback)
        ├─► Bun action / resource HTTP
        ├─► Python processor (score / LLM)
        └─► Go SQLite persistence
```

Authors express intent in Silc. The compiler owns substrates (React, Bun,
CPython, Go, IPC). Agents and humans edit `.silc` only — never `.runtime/`.

| Construct | Purpose |
| --- | --- |
| Contract | Typed record schema (`contract Note { has Str $.text; }`) |
| Component | Reusable UI: props, `has state`, slots, `emit`, handlers, `render()` |
| Resource | `query` / `mutation` data layer → SQLite CRUD HTTP |
| App | `route "/path" => Page;` — dual-surface serving is synthesized |
| Module | Optional `service` / `processor` / `task` pipelines |

Built-in primitives (`ui::page`, `ui::button`, `ui::form`, …) are compiler-owned
catalog entries with web and terminal contracts. Author-defined components live
in app source. There is no separate component-source stdlib or resolver.

---

## Authoring API (0.4.0)

This section is the human-facing mirror of the canonical agent contract in
[`crates/silc/templates/AGENTS.md`](crates/silc/templates/AGENTS.md). Detailed
semantics live in the ADRs after the API is visible here.

### Language structure

| Construct | Role |
| --- | --- |
| `@version("0.4.0")` | Required exact source-version annotation |
| `subset Name of Base where { … }` | Semantic type alias; v1 `where` predicates (Str): `.contains` / `.starts-with` / `.ends-with` (ADR-002) |
| `contract X { has T $.f; }` | **Contract** — typed data schema |
| `component X` | **Component** — props, `has state`, slots, `emit`, handlers, `render()` |
| `resource X for Contract` | **Resource** — capability CRUD (`query list;`, `mutation create;`, …) |
| `app X` | **App** — `route` table (dual-surface serving synthesized) |
| `service X` / `processor X` / `task X` | Optional workflow modules |
| `==>` | Pipeline feed between values and `ns::op(...)` calls |

### Types

Built-in named types: `Str`, `UUID`, `num32`, `num64`, `int32`, `int64`,
`Bool`, `Int`.

Also valid: contract names, subset names, arrays (`[Product]`), and fixed
vectors (`Vec[num32; 768]`).

### Expressions and control flow

Supported in handlers / templates:

- Literals, `$name` / `$.field`, member access, calls, `Type.new(:field(value))`
- Arithmetic / comparison / boolean ops, unary `!` / `-`
- Assignment to component state: `$.field = expr;`
- Lists: `[a, b]`
- `emit event(payload)`, `navigate("/path")`, `await expr`
- Template control: `when expr { … } else { … }`, `for expr -> $item { … }`

### Dual-surface UI (required)

Authors write **one** semantic component tree. The compiler lowers it to:

- `ui::web` → React/Tailwind (compiler-owned)
- `ui::terminal` → OpenTUI (compiler-owned); TCP telnet CLI is a remote fallback

Every UI `app` synthesizes both surfaces automatically (web + terminal).
Authors never write `method serve()`, `ui::web`, or `ui::terminal` as program
operations. No component may be web-only or terminal-only. Override ports with
`SILC_HTTP_PORT` / `SILC_TERMINAL_PORT` if needed.

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

### Component / resource / app patterns

```silc
component HomePage {
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

resource Products for Product {
    query list;
    mutation create;
}

app MyApp {
    route "/" => HomePage;
}
```

Derived HTTP (compiler-owned): `GET/POST /api/{table}`,
`GET/PUT/DELETE /api/{table}/:id`.

Wire handlers with `:on(click(handler))` / `:on(event => handler)`. Chat that
must reason over live data uses `ui::chat(:context(...), :persona(...),
:session(...))`. Local completions use **silclm** — call `llm::complete()` with
no `:model` (or `:model("silclm")`).

### Executable operations

Executable today (registry in [`crates/sil-core/src/operation.rs`](crates/sil-core/src/operation.rs)):

`service::http`, `text::score`, `llm::complete`,
`scrape::page`, `scrape::site`, `scrape::select`, `scrape::render`,
`scrape::extract`, `tensor::tokenize`, `tensor::infer`.

Pipeline-only programs run a closed
`scrape::page` → `scrape::extract` → `tensor::tokenize` →
`tensor::infer(:prefer(CPU))` → SQLite path. Their output is a normalized
`Vec[num32; 384]` using compiler-pinned `minilm-l6-v2`; run with
`silc run main.silc --input-json '{"url":"https://…"}'`.

Stub-only namespaces (parse/route/emit, do not run): `http`, `html`,
`numpy`, `pandas`, `ws`, `sys`, `schema`, `payload`, `json`, plus non-registry
ops under runnable namespaces. Mixing stub-only ops into a runnable graph is a
**compile error**. Prefer `scrape::*` over stub `http::get` / `html::*`
([ADR-006](docs/ADR-006-scrape-namespace.md)).

Graph constraints:

1. UI apps require an `app` with non-empty `route`s; dual-surface web/terminal
   serving is synthesized by the compiler.
2. `text::score` and `llm::complete` cannot both appear in one program.
3. Score/LLM/tensor paths need exactly one processor; SQLite persistence is
   synthesized (do not declare `sink` / `ipc::*` / `store::*`).
4. API-only `service::http` programs must not declare processor modules.
5. `scrape::*` cannot mix with `text::score`; it may feed scraped content into
   `llm::complete` for grounded SilcLM summaries.
6. Tensor pipelines are CPU-only, require MiniLM, and emit exactly 384
   normalized `num32` values.

### Generated runtime surfaces

- `POST /submit` — form `submit()` handlers (scrape jobs when `scrape::*` present)
- `POST /scrape` — scrape ingest when `scrape::*` present
- `POST /complete` — chat / `*.complete()` processors
- `GET|POST|PUT|DELETE /api/{table}` — resource queries/mutations
- Web: React app served by Bun
- Terminal: OpenTUI app (local TTY); TCP telnet CLI on the terminal port as remote fallback

Default fallback ports if omitted: web `18088`, terminal `18023`, API `8080`.

---

## Examples

Examples are **standalone Silc projects** (same shape as `silc init`). See
[`examples/README.md`](examples/README.md).

| App | Purpose |
| --- | --- |
| [`examples/chatApp/`](examples/chatApp/) | Multi-session local chat via **silclm** |
| [`examples/inventoryApp/`](examples/inventoryApp/) | Inventory CRUD + browse/admin + grounded silclm assistant |
| [`examples/scraperApp/`](examples/scraperApp/) | URL + depth form; `scrape::site` crawl; results table |
| [`examples/pipelineApp/`](examples/pipelineApp/) | One-shot scrape → MiniLM/ONNX → SQLite ([ADR-010](docs/ADR-010-tensor-minilm-pipeline.md)) |

Each example `AGENTS.md` embeds the compiler template common block byte-for-byte
and appends app-specific notes after `<!-- END SILC_AGENTS_TEMPLATE -->`.

Local chat uses **silclm** (Silc's owned model identity; v0 is a pinned Llama
3.2 3B GGUF). Omit `:model` or pass `:model("silclm")`. See
[ADR-005](docs/ADR-005-local-llm-complete.md).

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
        ├── Bun  (web + terminal + resource HTTP + static scrape)
        ├── CPython (scoring / local LLM / Playwright scrape)
        ├── Go (SQLite persistence / HTTP API / Colly crawl)
        └── sil-ipc mmap slots + UDS
```

Workspace crates: `sil-core`, `sil-lexer`, `sil-parser`, `sil-router`,
`sil-codegen`, `sil-ipc`, `silc` (CLI + supervisor), `sil-rlm` (assist loop),
`sil-training` (provider-neutral silclm dataset harness).

Per-app output lands in `{workdir}/.runtime/{program}/` (gitignored). Engines
stay in `~/.silc/runtimes/`. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## For AI agents

`silc init` copies [`crates/silc/templates/AGENTS.md`](crates/silc/templates/AGENTS.md)
into the project. That file is the operational contract:

- Edit Silc source only; never patch `.runtime/`
- Prefer components + apps + resources over inventing portal profiles
- Declare `app` routes; dual-surface web/terminal serving is synthesized
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
| [docs/ADR-INDEX.md](docs/ADR-INDEX.md) | ADR index (decisions, specs, appendices) |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Subject model and crate layout |
| [docs/intent-vs-subjects.md](docs/intent-vs-subjects.md) | Intent authoring vs subject architecture |
| [docs/ADR-001-runtime-and-ipc.md](docs/ADR-001-runtime-and-ipc.md) | Engines and IPC |
| [docs/ADR-002-silc-surface-syntax.md](docs/ADR-002-silc-surface-syntax.md) | Language surface (Raku-inspired, not Raku-compatible) |
| [docs/ADR-003-declarative-ui.md](docs/ADR-003-declarative-ui.md) | Dual-surface UI policy |
| [docs/ADR-004-runtime-strengths.md](docs/ADR-004-runtime-strengths.md) | Why Bun / CPython / Go |
| [docs/ADR-005-local-llm-complete.md](docs/ADR-005-local-llm-complete.md) | Local LLM completions |
| [docs/ADR-006-scrape-namespace.md](docs/ADR-006-scrape-namespace.md) | `scrape::*` namespace |
| [docs/ADR-007-pipeline-feeds.md](docs/ADR-007-pipeline-feeds.md) | `==>` pipeline feed semantics |
| [docs/ADR-008-recursive-silclm-assist.md](docs/ADR-008-recursive-silclm-assist.md) | Recursive `silc assist` / silclm RLM |
| [docs/ADR-009-compiler-synthesized-runtime.md](docs/ADR-009-compiler-synthesized-runtime.md) | Compiler-synthesized UI / persistence |
| [docs/ADR-010-tensor-minilm-pipeline.md](docs/ADR-010-tensor-minilm-pipeline.md) | MiniLM embedding pipeline |
| [docs/subject-first-decision.md](docs/subject-first-decision.md) | Historical benchmark evidence (appendix) |
| [docs/subject-first-declarators.md](docs/subject-first-declarators.md) | Benchmark harness (appendix) |
| [docs/SILC-IPC-ABI-v1.md](docs/SILC-IPC-ABI-v1.md) | Shared buffer ABI |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |

---

## License

Apache-2.0 — see [LICENSE](LICENSE).
