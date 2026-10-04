<p align="center">
  <img src="assets/brand/thoughtpivot.svg" alt="ThoughtPivot" width="280" />
</p>

# Silc: an intent language and compiler for applications, real-time scenes, pipelines, and loops

**Silc** (pronounced *silk*) is a generic intent language and compiler.
Its lead use cases, in order, are **games**, **data pipelines**, **loops**,
and **dual-surface UI**.

Write a concise `.silc` program describing the domain. Silc validates the
intent, selects the right engines, and synthesizes the runtime.

1. **Games.** Scenes, entities, physics, cameras, and gameplay, synthesized
   as a browser-native WebGPU runtime.
2. **Data pipelines.** Scrape, extract, embed, and persist without naming
   the implementation framework.
3. **Loops.** Scheduled, approval-gated, model-assisted work, beside an app
   or once as a command.
4. **Dual-surface UI.** One component tree for a web app and a terminal
   interface.

After those four come real-time 3D and operational applications. The compiler
thesis, pinned runtimes, and the 0.5.0 `loop` subject follow in full.
Architecture, engineering, and construction (AEC) is a strong domain Silc
already speaks to: Virtual Design and Construction (VDC), construction
project environments, and RFIs. Silc stays a generic intent language and
compiler.

Silc is **open source** from **[ThoughtPivot](https://github.com/thoughtpivot)**.

```text
.silc intent  →  Rust compiler  →  Bun · CPython · Go workers  →  mmap IPC + UDS
```

---

## What you can build

### 1. Games

Declare scenes, imported assets, prefabs, entities, physics, navigation,
cameras, materials, effects, and gameplay systems. The compiler synthesizes a
Babylon.js WebGPU runtime without making Babylon, Unity, or Unreal the authoring
surface.

Examples include a cinematic first-person arena
([`examples/core/arenaGameApp`](examples/core/arenaGameApp/)) and a 2D platformer
([`examples/core/platformGameApp`](examples/core/platformGameApp/)).

**Inspired by the big three:**

| Pattern | Inspiration | Silc surface |
| --- | --- | --- |
| Entity hierarchy | Godot node tree | Nested `scene::entity` with parent/child transforms |
| Signals and groups | Godot signals | `scene::signal`, `scene::group` |
| Prefabs and data assets | Unity prefabs + ScriptableObjects | `scene::prefab`, `scene::spawn`, `scene::data` + `:ref` |
| Mode / Pawn / Controller | Unreal gameplay framework | `game::mode`, `game::pawn`, `scene::controller` |
| Abilities | Unreal GAS | `game::ability` with cooldowns, costs, and cue children |
| Asset bake | Unity import pipeline | CPython → `public/baked/` (PBR textures, collision hulls) |

That `game::` surface is also Silc's generic real-time 3D subject.
[Real-time 3D](#real-time-3d) covers simulations, training tools,
digital-twin foundations, and VDC experiences.

### 2. Data pipelines

Use `scrape::page`, `scrape::site`, `doc::extract`, `tensor::tokenize`,
`tensor::infer`, and `llm::complete` to express data movement and processing
without naming the implementation framework. Silc routes work to its
compiler-owned engines and synthesizes persistence where supported.

[`examples/core/pipelineApp`](examples/core/pipelineApp/) is pipeline-only: scrape →
MiniLM/ONNX → SQLite. [`examples/core/scraperApp`](examples/core/scraperApp/) is a URL +
depth crawl with a results table and summaries.
[`examples/core/dataExtractorApp`](examples/core/dataExtractorApp/) uploads a file, runs
`doc::extract`, and writes a documents ledger.

The pitch is simple: **fewer tokens per working system**. Engine choice,
dual-surface parity, persistence, asset handling, and IPC are compiler
decisions—not prompt decisions.

### 3. Loops

`loop Name { loop::flow(...) }` declares work that runs on its own. Release
0.5.0 adds this subject for scheduled, approval-gated, model-assisted work
([ADR-014](docs/ADR-014-loop-subject.md)). A cron schedule in an explicit time
zone, Run now, or a resource mutation starts a run. Steps come from a closed
`loop::*` catalog: find rows, read outside data (`scrape::page`, or one MCP
tool with `mcp::call`), ask silclm for a typed contract, gate, branch, iterate
with a bound, approve, and write or notify exactly once per key.

A loop can sit beside an `app`, and the compiler adds a `/loops` inbox. With
no `app` and only `loop::manual` triggers, it is a command: `silc main.silc`
runs each loop once, prints the notices, and exits.

[`examples/core/mcpLoopApp`](examples/core/mcpLoopApp/) is the generic MCP loop
command. Its README also shows the scheduled inbox-only service and the form
that sits beside an `app`. The weekday RFI chase is
[`examples/domains/aec/rfiChaseApp`](examples/domains/aec/rfiChaseApp/). The full
subject, including a complete program, is under [Scheduled loops](#scheduled-loops).

### 4. Dual-surface UI

**Operational tools that work everywhere.** One component tree compiles to both
a React/Tailwind web app and an OpenTUI terminal interface. Teams get a browser
dashboard and SSH access to the same workflows. Authors declare routes only.

`silc init` scaffolds this shape: a form, an app route table, and an optional
scorer. Dual-surface web/terminal serving and SQLite persistence are
**synthesized**. The notes app under
[How it looks in practice](#how-it-looks-in-practice) is that scaffold.

---

## Real-time 3D

The real-time kernel is `scene::`: entities, assets, physics, cameras, lights,
and environment, with no gameplay vocabulary. Gameplay (pawns, weapons,
encounters) is the `game::` layer on that kernel. The root keyword is `scene`;
`game Name` remains a one-release alias. Babylon.js is the WebGPU adapter, not
the authoring surface. The same kernel carries entertainment, simulation,
training, and project-environment scenes.

A `scene` program and a dual-surface `app` program are distinct roots today.
Silc does not yet embed a `ui::` application inside a `scene::` world. They
share the language, compiler, runtime ownership model, and generic primitives.

The 0.5.0 node list is in What ships today, under Real-time 3D. See
[ADR-012](docs/ADR-012-webgpu-game-subject.md).

---

## Operational applications

**CRUD apps with zero boilerplate.** Declare a contract and a resource; Silc
synthesizes SQLite tables, HTTP APIs, and form bindings. No Express routers,
ORM setup, or migration scripts.

**Local assistants grounded on project data.** Add `ui::chat` with live query
context and a persona. The compiler provisions **silclm** and connects it to
the application's resources.

The same model covers internal tools and assistants.
[`examples/core/inventoryApp`](examples/core/inventoryApp/) is CRUD plus a grounded
assistant. [`examples/core/chatApp`](examples/core/chatApp/) is multi-session local chat
via silclm. [`examples/core/blogApp`](examples/core/blogApp/) is a seeded blog with
filters, admin modal CRUD, and grounded search.

---

## Domains

The compiler ships generic kernels: `ui::`, `scene::`, `loop::`, and pipeline
operations. Vocabulary for a vertical is a domain layer or a package, never a
compiler default. Gameplay is the first such layer (`game::` on the `scene::`
kernel). Architecture, engineering, and construction is the lead go-to-market,
documented separately so it does not become the language's identity.

- [Virtual Design and Construction](docs/domains/vdc.md) — project walkthroughs, field tools, and the RFI loop in [`examples/domains/aec/`](examples/domains/aec/).

---

## The Silc thesis

Modern AI coding workflows still spend too much of their budget on decisions
that should be deterministic: framework selection, UI parity, persistence,
worker boundaries, IPC, asset handling, and runtime setup.

Agents and humans repeatedly invent React trees, Python services, Go stores,
package manifests, engine scaffolding, and integration glue. That burns tokens,
creates drift, and blurs the line between *domain intent* and *runtime
substrate*.

**Authors and agents should declare intent; the compiler should own substrate.**
Deterministic routing, closed operation registries, and compiler-synthesized
mechanics let models spend tokens on project meaning—spaces, equipment,
workflows, records, simulations, and decisions—while Silc handles the rest.

---

## How it looks in practice

Examples below are Silc 0.6.0 source. GitHub fences use `raku` for highlighting
only. The surface is **Raku-inspired**, not Raku-compatible. Source files are
`.silc` only.

The samples follow the lead order: a game, a data pipeline, a loop, and
dual-surface UI. Operational CRUD and a VDC project walkthrough follow
those four.

### 1. Real-time 3D game: first-person shooter

From [`examples/core/arenaGameApp`](examples/core/arenaGameApp/) — a cinematic FPS with
weapons, hostile AI, and modular level geometry. It exercises the same reusable
scene, asset, physics, camera, and entity kernel available to VDC and simulation
programs.

```raku
@version("0.6.0")

scene Arena {
    scene::scene(:title("MEGASTRUCTURE"), :renderer(webgpu), :target_fps(90),
        scene::data(:name("WalkDefault"), :speed(5.5)),
        scene::data(:name("VanguardData"), :damage(16), :fire_rate(9), :magazine(30)),

        scene::prefab(:name("Player"),
            scene::mesh(:shape(capsule), :size(1.8)),
            scene::collider(:shape(capsule), :size(1.8)),
            scene::movement(:style(first_person), :ref("WalkDefault")),
            game::attribute(:name("health"), :value(100), :max(100)),
            game::pawn()
        ),

        scene::spawn(:prefab("Player"), :x(0), :y(1), :z(0), :as_pawn),
        game::weapon(:name("VanguardAR"), :slot(1), :fire_mode(hitscan), :ref("VanguardData")),
        game::mode(:id("arena"), :possess("Player")),
        scene::controller(:scheme(wasd_mouse)),
        scene::camera(:mode(first_person), :follow(pawn))
    )
}
```

**You declared:** player prefab, weapon stats, spawn point, camera mode.
**Silc synthesizes:** Babylon WebGPU scene, physics colliders, input handling,
HUD, and Go/SQLite persistence for saves and analytics.

### 2. Pipeline-only: scrape → embed → store

From [`examples/core/pipelineApp`](examples/core/pipelineApp/) — no UI app required. One
intent file becomes a Bun/CPython/Go ingestion graph.

```raku
@version("0.6.0")

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

### 3. A scheduled loop

Release 0.5.0 adds `loop Name { loop::flow(...) }` for scheduled,
approval-gated, model-assisted work
([ADR-014](docs/ADR-014-loop-subject.md)). A complete program is under
[Scheduled loops](#scheduled-loops).

- [`examples/core/mcpLoopApp`](examples/core/mcpLoopApp/) — MCP loop command: one `mcp::call`, a silclm brief, one keyed note. The same file's README shows the scheduled inbox and the `app` form
- [`examples/domains/aec/rfiChaseApp`](examples/domains/aec/rfiChaseApp/) — weekday `loop` beside an app: overdue RFIs, a silclm draft, a PM approval, a keyed reminder

### 4. A dual-surface notes app

What `silc init` scaffolds — a form, an app route table, and an optional
scorer. Dual-surface web/terminal serving and SQLite persistence are
**synthesized**.

```raku
@version("0.6.0")

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

### 5. Resource CRUD + grounded local chat

From [`examples/core/inventoryApp`](examples/core/inventoryApp/) — capability-style
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

### 6. A project walkthrough

A browser-native project environment uses the same `scene::` kernel as a game.
The worked example, including why this is a domain and not a language default,
lives in [docs/domains/vdc.md](docs/domains/vdc.md) and
[`examples/domains/aec/vdcWalkthrough`](examples/domains/aec/vdcWalkthrough/).

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

## Quick start

```bash
cargo install --path crates/silc --force

silc init myapp
cd myapp
silc build main.silc   # validate + codegen
silc main.silc              # run web by default
silc main.silc --terminal   # also attach OpenTUI (+ telnet)

# web:      http://127.0.0.1:18088  (override SILC_HTTP_PORT)
# terminal: silc main.silc --terminal  (or SILC_TERMINAL=1)
# fallback: telnet 127.0.0.1 18023 when --terminal is set
```

`silc init` writes `main.silc`, `AGENTS.md`, `.gitignore`, and a runtime lock,
then provisions pinned engines on first use.

### Example projects

| App | Purpose | Web | Terminal |
| --- | --- | --- | --- |
| [`examples/core/arenaGameApp/`](examples/core/arenaGameApp/) | Real-time WebGPU kernel plus gameplay: assets, environments, physics, and modular scenes | 18140 | — |
| [`examples/core/platformGameApp/`](examples/core/platformGameApp/) | 2D platformer: sprites, collectibles, patrols, and a level end | 18140 | — |
| [`examples/core/pipelineApp/`](examples/core/pipelineApp/) | Scrape → MiniLM/ONNX → SQLite | — | — |
| [`examples/core/scraperApp/`](examples/core/scraperApp/) | URL crawl at depth 2; results table + summaries | 18110 | 18111 |
| [`examples/core/dataExtractorApp/`](examples/core/dataExtractorApp/) | File upload + `doc::extract` → documents ledger | 18130 | 18131 |
| [`examples/core/mcpLoopApp/`](examples/core/mcpLoopApp/) | MCP loop command: one `mcp::call`, a silclm brief, one keyed note | — | — |
| [`examples/core/chatApp/`](examples/core/chatApp/) | Multi-session local chat via silclm | 18090 | 18091 |
| [`examples/core/inventoryApp/`](examples/core/inventoryApp/) | CRUD + browse/admin + grounded assistant | 18096 | 18097 |
| [`examples/core/hotelSignupApp/`](examples/core/hotelSignupApp/) | Smallest two-route app: sign-up form + ledger table | 18088 | 18023 |
| [`examples/core/blogApp/`](examples/core/blogApp/) | Seeded blog; year/month filters; admin modal CRUD; grounded search | 18120 | 18121 |
| [`examples/domains/aec/rfiChaseApp/`](examples/domains/aec/rfiChaseApp/) | Weekday `loop`: overdue RFIs → silclm draft → PM approval → keyed reminder | 18088 | — |
| [`examples/domains/aec/vdcWalkthrough/`](examples/domains/aec/vdcWalkthrough/) | Project-environment scene on the `scene::` kernel | 18140 | — |

See [`examples/README.md`](examples/README.md).

---

## What ships today (0.6.0)

Silc is **pre-1.0**. Release 0.4.0 made the product rule explicit: authors
declare intent; the compiler synthesizes runtime mechanics
([ADR-009](docs/ADR-009-compiler-synthesized-runtime.md)). Release 0.5.0 adds
the `loop` subject for scheduled, approval-gated, model-assisted work
([ADR-014](docs/ADR-014-loop-subject.md)).

### Applications

Every UI `app` synthesizes **both** surfaces automatically — compiler-owned
`ui::web` (React/Tailwind) and `ui::terminal` (OpenTUI). Authors declare routes
only; they never write `method serve()`, `ui::web`, or `ui::terminal` as program
operations. The full UI primitive catalog, closed option enums, and agent rules
live in
[`crates/silc/templates/AGENTS.md`](crates/silc/templates/AGENTS.md).

**Shipped for apps:**
- Parse → validate → deterministic Tier 1/2 route → codegen → supervised run
- Declaration-based `component` / `resource Name for Contract` / `app` routes
- Dual-surface UI synthesized from `app` (web + terminal)
- Generic resource CRUD over SQLite
- `silc init` scaffold and experimental `silc assist`
- Compiler-owned Bun / CPython / Go under `~/.silc/runtimes/`

### Real-time 3D

The WebGPU-only `game` subject is Silc's current generic real-time 3D surface.
You declare intent with `game::*` nodes; the compiler synthesizes a Babylon.js
runtime. Babylon is the WebGPU adapter, not the authoring surface, and the
namespace does not limit the kernel to entertainment games.

**What you can declare:**
- `scene::scene` — root with title, renderer, target FPS
- `scene::entity` — transform node with mesh, collider, light children
- `scene::prefab` / `scene::spawn` — reusable templates with override options
- `game::weapon` — hitscan, pellet, projectile, or beam fire modes
- `game::npc` / `game::perception` / `game::nav_agent` — hostile AI with nav mesh
- `game::ability` — cooldowns, attribute costs, particle/light/impulse cues
- `scene::camera`, `scene::controller`, `scene::hud`, `scene::post_process`

**Polyglot spine:** Real-time 3D programs use the full stack. CPython bakes
assets at compile time. Go persists saves, runs, and analytics to SQLite. Bun
serves the WebGPU host and handles HTTP for settings and telemetry.

See [ADR-012](docs/ADR-012-webgpu-game-subject.md) for the full design.

### Scheduled loops

`loop Name { loop::flow(...) }` declares work that runs on its own beside an
`app`: a cron schedule in an explicit time zone, Run now, or a resource
mutation starts a run. Steps come from a closed `loop::*` catalog: find rows,
read outside data (`scrape::page`, or one MCP tool with `mcp::call`), ask
silclm for a typed contract, gate, branch, iterate with a bound, approve, and
write or notify exactly once per key.

```silc
loop OneThingToday {
    loop::flow(
        loop::schedule(:cron("0 5 * * *"), :tz("UTC")),
        loop::read(:as(decisions), :op("mcp::call"), :server("https://moz.example/mcp"),
            :auth_env("MOZ_MCP_TOKEN"), :tool("kb_jsonl_read_window"),
            :args(WindowArgs.new(:path("memory/decisions.jsonl"), :tailRecords(6)))),
        loop::ask(:as(action), :into(OneThing), :from($decisions.data),
            :prompt("One imperative sentence for {$calendar.today}.")),
        loop::gate(:that($action.sentence != ""), :reason("one sentence or nothing")),
        loop::notify(:to("Dan"), :text("{$action.sentence}"), :key("one-thing:{$calendar.today}"))
    )
}
```

Gates fail closed, model output must pass a gate or approval before an effect,
and `silc build` prints the worst-case model calls, effects, and reads per run.
A compiler-owned Go kernel records every outside input and model answer, so a
run resumed after a crash or an approval replays instead of redoing work. See
[ADR-014](docs/ADR-014-loop-subject.md).

### Executable operations

Author-facing operations that run today:

`service::http`, `text::score`, `llm::complete`,
`scrape::page`, `scrape::site`, `scrape::select`, `scrape::render`,
`scrape::extract`, `doc::extract`, `tensor::tokenize`, `tensor::infer`.

### Boundaries

- Broader pipeline namespaces (`http::*`, `html::*`, `numpy::*`, `pandas::*`, …)
  are stub-only: they parse/route/emit but do not execute
- Tensor path is CPU-only MiniLM → exactly 384 normalized `num32` values
- IPC ABI v1 is schema-tagged JSON in mmap (not typed zero-copy views)
- No self-contained `silc bundle` deployment artifact yet
- Assist is experimental; fine-tuned assist weights are not shipped

Authoring contract for agents:
[`crates/silc/templates/AGENTS.md`](crates/silc/templates/AGENTS.md).

---

## Runtime: why Bun, CPython, and Go

You never pick a language — the compiler does. Each engine handles what it does
best, and they communicate through shared memory.

Silc does not ask models (or developers) to pick languages. The router assigns
work from complementary strengths
([ADR-004](docs/ADR-004-runtime-strengths.md)):

| Engine | Role in Silc |
| --- | --- |
| **Bun** | Generated TypeScript: web UI, terminal UI, HTTP ingress, static scrape helpers |
| **CPython** | Scoring, local LLM (llama.cpp / silclm), Playwright scrape, ONNX MiniLM, game asset baking |
| **Go** | SQLite persistence, HTTP APIs, high-concurrency Colly crawls |

Engines are pinned and checksum-verified (Bun 1.2.18, CPython 3.12.12,
Go 1.23.6) under `~/.silc/runtimes/`. There is no PATH override surface and no
author-facing engine picker.

```text
Silc source (.silc)
        │
        ▼
   sil-lexer → sil-parser → sil-core subjects
        │     (Contract · Component · Resource · App · Module · Pipeline · Game)
        ▼
   sil-router   Tier 1 (kind + traits) + Tier 2 (namespaces)
        ▼
   sil-codegen  runnable workers + dual-surface UI lowering + game kernel
        ▼
   silc supervisor
        ├── Bun  (web + terminal + resource HTTP + static scrape)
        ├── CPython (scoring / local LLM / Playwright / ONNX / game bake)
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

Cross-engine data movement uses ThoughtPivot's **Silc Shared Buffer ABI v1**
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
- **Silc Assist (experimental):** `silc assist` drafts and modifies `.silc`
  files with silclm ([ADR-008](docs/ADR-008-recursive-silclm-assist.md)). It
  auto-retrieves relevant examples and `AGENTS.md` rules, asks for a complete
  program via the chat template (stop marker `# END`), then compile-and-repairs.
  Creating a file adapts the `silc init` **starter** as a skeleton, so the usual
  run lands on the first attempt in ~6–12s. Repairs escalate cheapest-first:
  mechanical diagnostics are auto-fixed with no model call, structural ones get
  an explicit rule, and only the rest fall back to error-targeted corpus search.
  The slower tool loop is opt-in (`--explore`). Inference uses a warm silclm
  worker with Metal GPU offload by default on Apple Silicon.

```bash
silc assist "dual-surface notes app with submit" notes.silc
silc assist "refine the form" notes.silc --explore   # optional slower fallback
```

Assist is Phase 1: useful, bounded, and experimental. A fine-tuned
`silclm-assist` model is reserved but not shipped yet. In-app chat and Assist
remain separate products on the same local model family.

**Token efficiency, concretely:** every framework/engine/persistence decision
the compiler owns is a decision the model no longer has to negotiate in
context. Compiler diagnostics then act as a hard oracle — accepted programs
parse, validate, and route before they run.

---

## Editor support (VS Code / Cursor)

Silc ships a VS Code / Cursor extension that provides syntax highlighting and a
Rust language server (`sil-lsp`) for semantic hover on `.silc` sources — resource
methods, query bindings, contracts and fields, components, options and state, UI
primitives, executable operations, keywords, operators, and builtin types.

Install it with the bundled script:

```bash
./editors/vscode-silc/install.sh
```

The script:

1. Builds `sil-lsp` in release mode (`cargo build -p sil-lsp --release`)
2. Installs npm dependencies and compiles the TypeScript language client
3. Bundles the host-platform server binary into a VSIX
4. Installs the extension with the `cursor` CLI, falling back to `code`

Requirements: a Rust toolchain, Node.js/npm, and a `cursor` (or `code`) CLI on
your `PATH`. In Cursor, you can add the CLI via **Shell Command: Install 'cursor'
command in PATH**. Set `SILC_EDITOR_CLI` to override CLI detection.

After it finishes, run **Developer: Reload Window**. Open any `.silc` file — the
language indicator should read **Silc**, and hovering a symbol should show a
Markdown tooltip. To point the editor at a locally built server without
reinstalling, set `silc.languageServerPath` to your
`target/release/sil-lsp` path.

See [`editors/vscode-silc/README.md`](editors/vscode-silc/README.md) for hover
coverage, highlighting scopes, and development details.

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
| [docs/SILC-LANGUAGE.md](docs/SILC-LANGUAGE.md) | Normative language surface (declarations, nodes, options, operations) |
| [docs/GLOSSARY.md](docs/GLOSSARY.md) | One meaning per term; retired vocabulary |
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
| [docs/ADR-011-document-extract.md](docs/ADR-011-document-extract.md) | `doc::*` upload + extract |
| [docs/ADR-012-webgpu-game-subject.md](docs/ADR-012-webgpu-game-subject.md) | WebGPU game kernel |
| [docs/ADR-013-procedural-asset-generation.md](docs/ADR-013-procedural-asset-generation.md) | Procedural asset generation (`scene::generate`) |
| [docs/ADR-014-loop-subject.md](docs/ADR-014-loop-subject.md) | Loop declaration: scheduled, approval-gated, model-assisted work |
| [docs/ADR-015-silc-loop-and-vran.md](docs/ADR-015-silc-loop-and-vran.md) | Silc `loop` and Vran |
| [docs/ADR-016-generic-kernel-domain-layers.md](docs/ADR-016-generic-kernel-domain-layers.md) | Generic kernels and domain layers |
| [docs/ADR-017-ui-embed.md](docs/ADR-017-ui-embed.md) | `ui::embed` dual-surface URL viewport |
| [docs/domains/vdc.md](docs/domains/vdc.md) | VDC / AEC domain (go-to-market, not language identity) |
| [docs/SILC-IPC-ABI-v1.md](docs/SILC-IPC-ABI-v1.md) | Shared buffer ABI |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |

---

## License

Apache-2.0 — see [LICENSE](LICENSE).

Maintained by the **[ThoughtPivot](https://github.com/thoughtpivot)** engineering
team.