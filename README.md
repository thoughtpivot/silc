<p align="center">
  <img src="assets/brand/thoughtpivot.svg" alt="ThoughtPivot" width="280" />
</p>

# Silc — Intent-Native Polyglot Application Compiler

**Silc** (pronounced *silk*) is ThoughtPivot’s intent language and local Rust
compiler for building real software applications with far less accidental
complexity — and far fewer model tokens — than asking an LLM to invent a full
stack from scratch.

You write a short `.silc` program that declares *what the application is*:
typed contracts, UI components, resource capabilities, routes, and domain
pipelines. The compiler validates that intent, routes each module to the right
engine, provisions pinned runtimes, synthesizes dual-surface UI and
persistence, and runs a supervised polyglot runtime.

Silc is **open source** from **[ThoughtPivot](https://github.com/thoughtpivot)**.

```text
.silc intent  →  Rust compiler  →  Bun · CPython · Go workers  →  mmap IPC + UDS
```

The surface is **Raku-inspired**, not Raku-compatible. Source files are
`.silc` only. Code examples below use GitHub’s `raku` fence solely so syntax
highlighting renders well.

---

## The problem Silc solves

Modern AI coding workflows still spend most of their budget on decisions that
should be deterministic:

- Which framework should host the UI?
- How should web and terminal surfaces stay in sync?
- Where does SQLite wiring live, and who owns migrations?
- Which language should score text, call a local LLM, crawl a site, or embed a
  document?
- How do those processes exchange payloads without reinventing glue every time?

Agents and humans repeatedly invent React trees, Python services, Go stores,
package manifests, IPC schemes, and deployment scaffolding. That inventiveness
burns tokens, creates drift between runs, and blurs the line between *product
intent* and *runtime substrate*.

**Silc’s thesis:** authors and agents should declare intent; the compiler should
own substrate. Deterministic routing, closed operation registries, and
compiler-synthesized mechanics shrink the generation surface. Models spend
tokens on domain meaning — forms, inventory, scrapers, assistants — while Silc
handles the rest.

---

## Business cases

Silc is for teams that want AI-assisted software creation **at application
scale**, not just snippet scale:

| Business need | What Silc provides |
| --- | --- |
| Dual-surface internal tools | One component tree → web (React/Tailwind) + terminal (OpenTUI) |
| Operational CRUD apps | Contracts + resource capabilities → SQLite HTTP APIs |
| Local AI assistants | Grounded `ui::chat` over live data via **silclm** |
| Content / research ingestion | `scrape::*` crawls without naming Bun, Colly, or Playwright |
| Embedding pipelines | Closed MiniLM path: scrape → tokenize → infer → SQLite |
| Agent-authored software | Closed language + compiler oracle → fewer invented substrates |

The economic pitch is simple: **fewer tokens per working application**, because
framework choice, dual-surface parity, persistence, engine selection, and IPC
are compiler decisions — not prompt decisions.

---

## Design principles

1. **Intent over substrate.** Authors never write `serve()`, invent React or
   OpenTUI trees, declare sinks, or wire `ipc::*` / `store::*` pipelines.
2. **Deterministic compilation.** Tier 1/2 routing cites engine strengths; every
   decision has provenance.
3. **Scalable monolith.** One cohesive `.silc` intent model compiles into a
   supervised cluster of specialized workers (Bun, CPython, Go) that share
   memory-mapped slots. You author one program; the runtime is polyglot and
   co-located — not a sprawl of hand-maintained microservices.
4. **AI-native, compiler-first.** Models emit `.silc`. The compiler is the
   validation oracle. Assist explores corpus and checks drafts without stuffing
   the entire authoring contract into the root prompt.
5. **Pinned, owned runtimes.** Bun, CPython, and Go are checksum-verified into
   `~/.silc/runtimes/`. Authors and agents do not choose engines.

---

## How it looks in practice

Examples below are Silc 0.4.0 source. GitHub fences use `raku` for highlighting
only.

### 1. A dual-surface notes app

What `silc init` scaffolds — a form, an app route table, and an optional
scorer. Dual-surface web/terminal serving and SQLite persistence are
**synthesized**.

```raku
@version("0.4.0")

contract Note {
    has Str $.author;
    has Str $.text;
}

component HomePage {
    has state Str $.author = "";
    has state Str $.text = "";

    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("My Silc App"))),
            :side_panel(ui::side_panel(
                ui::nav_item(:label("Home"), :to("/"), :active)
            )),
            ui::stack(
                ui::heading(:text("Leave a note"), :level(2)),
                ui::form(:on(submit(on_submit)),
                    ui::text_input(:field(author), :label("Author")),
                    ui::textarea(:field(text), :label("Note")),
                    ui::toolbar(
                        ui::button(:label("Submit"), :variant(primary), :submit)
                    )
                )
            )
        )
    }

    method on_submit() {
        submit();
    }
}

app MyApp {
    route "/" => HomePage;
}

processor NoteScorer {
    method analyze(Note $note) {
        $note.text ==> text::score()
    }
}
```

**You declared:** schema, UI, routes, scoring intent.
**Silc synthesizes:** React web + OpenTUI terminal, `POST /submit`, Go/SQLite
sink, Bun ingress, and mmap staging between workers.

### 2. Resource CRUD + grounded local chat

From [`examples/inventoryApp`](examples/inventoryApp/) — capability-style
resources become HTTP CRUD; chat is grounded on a live inventory snapshot.

```raku
contract InventoryItem {
    has Str $.id;
    has Str $.name;
    has Str $.category;
    has Str $.location;
    has Str $.quantity;
    has Str $.reorder_level;
    has Str $.notes;
}

contract ChatRecord {
    has Str $.prompt;
    has Str $.reply;
}

resource InventoryItems for InventoryItem {
    query list;
    mutation create;
    mutation update;
    mutation delete;
}

component BrowsePage {
    has state Str $.category_filter = "All";
    query $.items = InventoryItems.list();

    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Inventory"))),
            :side_panel(ui::side_panel(
                ui::nav_item(:label("Browse"), :to("/"), :active),
                ui::nav_item(:label("Admin"), :to("/admin")),
                ui::nav_item(:label("Assistant"), :to("/assistant"))
            )),
            ui::stack(
                ui::section(
                    :title("Stock browser"),
                    :description("Filter by category, or ask the Assistant about live inventory.")
                ),
                ui::table(
                    :rows($.items),
                    :columns(["name", "category", "location", "quantity", "reorder_level", "notes"]),
                    :empty_text("No inventory items yet. Add some in Admin."),
                    :filter_field(category_filter),
                    :filter_column("category"),
                    :sortable,
                    :searchable
                )
            )
        )
    }
}

# … AdminPage omitted …

component AssistantPage {
    has state Str $.prompt = "";
    query $.items = InventoryItems.list();

    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Inventory Assistant"))),
            ui::chat(
                :value($.prompt),
                :context($.items),
                :persona("You are the Inventory Assistant for this Silc inventory app, built on silclm."),
                :placeholder("Which items are below reorder level?"),
                :on(send(on_send))
            )
        )
    }

    method on_send() {
        Assistant.complete();
    }
}

app InventoryApp {
    route "/" => BrowsePage;
    route "/admin" => AdminPage;
    route "/assistant" => AssistantPage;
}

processor Assistant {
    method complete(ChatRecord $record) {
        $record.prompt ==> llm::complete()
    }
}
```

**You declared:** domain model, CRUD capabilities, browse/admin/assistant
routes, and a local completion processor.
**Silc synthesizes:** `/api/inventory_items` CRUD, dual-surface UI, silclm
provisioning, and persistence for chat/processor results.

### 3. Pipeline-only: scrape → embed → store

From [`examples/pipelineApp`](examples/pipelineApp/) — no UI app required. One
intent file becomes a Bun/CPython/Go ingestion graph.

```raku
@version("0.4.0")

subset Uri of Str where { .starts-with("http") }
subset Emb384 of Vec[num32; 384];

contract ArticlePayload {
    has UUID $.id;
    has Uri $.url;
    has Str $.raw_content;
    has Emb384 $.vector_embedding;
}

service ArticleIngress {
    method fetch_article() {
        target_url
            ==> scrape::page(:js(false))
            ==> scrape::extract(:into(ArticlePayload))
    }
}

processor Embedder {
    method embed(ArticlePayload $article) {
        $article.raw_content
            ==> tensor::tokenize(:model("minilm-l6-v2"))
            ==> tensor::infer(:prefer(CPU))
    }
}
```

Run with:

```bash
silc run main.silc --input-json '{"url":"https://example.com/"}'
```

---

## Runtime: why Bun, CPython, and Go

Silc does not ask models (or developers) to pick languages. The router assigns
work from complementary strengths
([ADR-004](docs/ADR-004-runtime-strengths.md)):

| Engine | Role in Silc |
| --- | --- |
| **Bun** | Generated TypeScript: web UI, terminal UI, HTTP ingress, static scrape helpers |
| **CPython** | Scoring, local LLM (llama.cpp / silclm), Playwright scrape, ONNX MiniLM |
| **Go** | SQLite persistence, HTTP APIs, high-concurrency Colly crawls |

Engines are pinned and checksum-verified (Bun 1.2.18, CPython 3.12.12,
Go 1.23.6) under `~/.silc/runtimes/`. There is no PATH override surface and no
author-facing engine picker.

```text
Silc source (.silc)
        │
        ▼
   sil-lexer → sil-parser → sil-core subjects
        │     (Contract · Component · Resource · App · Module · Pipeline)
        ▼
   sil-router   Tier 1 (kind + traits) + Tier 2 (namespaces)
        ▼
   sil-codegen  runnable workers + dual-surface UI lowering
        ▼
   silc supervisor
        ├── Bun  (web + terminal + resource HTTP + static scrape)
        ├── CPython (scoring / local LLM / Playwright / ONNX)
        ├── Go (SQLite / HTTP API / Colly crawl)
        └── sil-ipc mmap slots + UDS
```

### Scalable monolith

A Silc program is a **monolith at the intent layer** and a **supervised polyglot
runtime** underneath. One file owns the product model. The compiler emits
specialized workers that scale *within* that model — for example, replica pools
for CPU-bound scoring — without forcing authors to design a microservice mesh.
That is the scalable-monolith shape: cohesive product semantics, partitioned
execution, shared contracts.

### IPC that stays out of your way

Cross-engine data movement uses ThoughtPivot’s **Silc Shared Buffer ABI v1**
([ADR-001](docs/ADR-001-runtime-and-ipc.md),
[SILC-IPC-ABI-v1.md](docs/SILC-IPC-ABI-v1.md)):

- **Data plane:** file-backed mmap slots under `.runtime/` (default
  512 × 16 KiB; larger for pipeline payloads). Magic bytes `SILC`.
- **Control plane:** small Unix domain socket wakeups
  (`segment_id`, `offset`, `len`, `schema_id`).

Payloads stay in shared memory between processor and synthesized persistence.
Workers do not retransmit application bodies over HTTP between those stages.
ABI v1 carries schema-tagged JSON in the mapped buffer; typed zero-copy field
views are a future ABI layer, not a current claim.

---

## AI-native authoring and Silc Assist

Silc is designed so language models author **intent programs**, not framework
scaffolding.

- **In-app intelligence:** `llm::complete` / `ui::chat` run on **silclm**
  (compiler-pinned local GGUF). Use `:context(...)` to ground answers on live
  resource data.
- **Silc Assist (experimental):** `silc assist` is a closed-tool recursive
  authoring loop around silclm ([ADR-008](docs/ADR-008-recursive-silclm-assist.md)).
  It searches examples and `AGENTS.md` incrementally, drafts `.silc`, and
  validates with parse → validate → route — instead of loading the entire
  authoring corpus into an 8K root window.

```bash
silc assist "dual-surface notes app with submit" --out notes.silc
```

Assist is Phase 1: useful, bounded, and experimental. A fine-tuned
`silclm-assist` model is reserved but not shipped yet. In-app chat and Assist
remain separate products on the same local model family.

**Token efficiency, concretely:** every framework/engine/persistence decision
the compiler owns is a decision the model no longer has to negotiate in
context. Compiler diagnostics then act as a hard oracle — accepted programs
parse, validate, and route before they run.

---

## Quick start

```bash
cargo install --path crates/silc --force

silc init myapp
cd myapp
silc build main.silc   # validate + codegen
silc main.silc         # run (OpenTUI attaches in a real TTY)

# web:      http://127.0.0.1:18088  (override SILC_HTTP_PORT)
# terminal: OpenTUI on the local TTY (primary)
# fallback: telnet 127.0.0.1 18023  (SILC_TERMINAL_PORT)
```

`silc init` writes `main.silc`, `AGENTS.md`, `.gitignore`, and a runtime lock,
then provisions pinned engines on first use.

### Example projects

| App | Purpose |
| --- | --- |
| [`examples/chatApp/`](examples/chatApp/) | Multi-session local chat via silclm |
| [`examples/inventoryApp/`](examples/inventoryApp/) | CRUD + browse/admin + grounded assistant |
| [`examples/scraperApp/`](examples/scraperApp/) | URL + depth crawl; results table + summaries |
| [`examples/pipelineApp/`](examples/pipelineApp/) | Scrape → MiniLM/ONNX → SQLite |

See [`examples/README.md`](examples/README.md).

---

## What ships today (0.4.0)

Silc is **pre-1.0**. Release 0.4.0 makes the product rule explicit: authors
declare intent; the compiler synthesizes runtime mechanics
([ADR-009](docs/ADR-009-compiler-synthesized-runtime.md)).

Every UI `app` synthesizes **both** surfaces automatically — compiler-owned
`ui::web` (React/Tailwind) and `ui::terminal` (OpenTUI). Authors declare routes
only; they never write `method serve()`, `ui::web`, or `ui::terminal` as program
operations. The full UI primitive catalog (38 dual-surface builtins), closed
prop enums, and agent rules live in
[`crates/silc/templates/AGENTS.md`](crates/silc/templates/AGENTS.md).

### Executable operations

Author-facing ops that run today:

`service::http`, `text::score`, `llm::complete`,
`scrape::page`, `scrape::site`, `scrape::select`, `scrape::render`,
`scrape::extract`, `tensor::tokenize`, `tensor::infer`.

**Shipped**

- Parse → validate → deterministic Tier 1/2 route → codegen → supervised run
- Declaration-based `component` / `resource Name for Contract` / `app` routes
- Dual-surface UI synthesized from `app` (web + terminal)
- Generic resource CRUD over SQLite
- `silc init` scaffold and experimental `silc assist`
- Compiler-owned Bun / CPython / Go under `~/.silc/runtimes/`

**Boundaries**

- Broader pipeline namespaces (`http::*`, `html::*`, `numpy::*`, `pandas::*`, …)
  are stub-only: they parse/route/emit but do not execute
- Tensor path is CPU-only MiniLM → exactly 384 normalized `num32` values
- IPC ABI v1 is schema-tagged JSON in mmap (not typed zero-copy views)
- No self-contained `silc bundle` deployment artifact yet
- Assist is experimental; fine-tuned assist weights are not shipped

Authoring contract for agents:
[`crates/silc/templates/AGENTS.md`](crates/silc/templates/AGENTS.md).

---

## For AI agents

`silc init` copies the agent contract into the project:

- Edit `.silc` only — never patch `.runtime/`
- Declare routes; dual-surface serving is synthesized
- Prefer components + resources over inventing portal profiles or frameworks
- Stay inside the UI catalog and runnable operation set
- Validate with `silc build`; report limits instead of escaping to React/OpenTUI

---

## Development

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace -- --test-threads=1
```

CI runs fmt, check, library tests, codegen smoke, dual-surface e2e builds, and
concurrent `/submit` POSTs with SQLite checks.

### Versioning

Pre-1.0 SemVer 0.x: breaking language/compiler changes bump the minor.
`1.0.0` is reserved for a future stability milestone. Releases use
[release-plz](release-plz.toml) and Conventional Commits.

---

## Documentation

| Doc | Topic |
| --- | --- |
| [docs/ADR-INDEX.md](docs/ADR-INDEX.md) | Decision index |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Subject model and crate layout |
| [docs/intent-vs-subjects.md](docs/intent-vs-subjects.md) | Intent authoring vs subject architecture |
| [docs/ADR-001-runtime-and-ipc.md](docs/ADR-001-runtime-and-ipc.md) | Engines and IPC |
| [docs/ADR-002-silc-surface-syntax.md](docs/ADR-002-silc-surface-syntax.md) | Language surface |
| [docs/ADR-003-declarative-ui.md](docs/ADR-003-declarative-ui.md) | Dual-surface UI policy |
| [docs/ADR-004-runtime-strengths.md](docs/ADR-004-runtime-strengths.md) | Why Bun / CPython / Go |
| [docs/ADR-005-local-llm-complete.md](docs/ADR-005-local-llm-complete.md) | Local LLM completions |
| [docs/ADR-006-scrape-namespace.md](docs/ADR-006-scrape-namespace.md) | `scrape::*` |
| [docs/ADR-007-pipeline-feeds.md](docs/ADR-007-pipeline-feeds.md) | `==>` semantics |
| [docs/ADR-008-recursive-silclm-assist.md](docs/ADR-008-recursive-silclm-assist.md) | Silc Assist |
| [docs/ADR-009-compiler-synthesized-runtime.md](docs/ADR-009-compiler-synthesized-runtime.md) | Synthesized UI / persistence |
| [docs/ADR-010-tensor-minilm-pipeline.md](docs/ADR-010-tensor-minilm-pipeline.md) | MiniLM embedding pipeline |
| [docs/SILC-IPC-ABI-v1.md](docs/SILC-IPC-ABI-v1.md) | Shared buffer ABI |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |

---

## License

Apache-2.0 — see [LICENSE](LICENSE).

Maintained by the **[ThoughtPivot](https://github.com/thoughtpivot)** engineering
team.
