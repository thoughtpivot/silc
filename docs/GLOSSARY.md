# Silc glossary

One meaning per term. Author-facing documents (README, AGENTS.md,
[SILC-LANGUAGE.md](SILC-LANGUAGE.md), example READMEs, compiler diagnostics,
and editor hovers) use these words and no synonyms. Historical ADRs are not
rewritten; where they use an older word, this page is the current meaning.

## Rule: full words only

Author-facing text does not abbreviate. In particular:

- **operation**, never "op". An operation is an executable `ns::name(...)`
  call such as `scrape::page` or `llm::complete`.
- **option**, never "prop", "adverb", "colon-pair", or "arg". An option is one
  `:name(value)` argument on a node, an operation, or a component invocation.
  A bare `:name` is a **flag** option. Collectively: **options**.

Rust identifiers (`EXECUTABLE_OPS`, `PropSpec`, `prop_doc`, …) are renamed when
the owning code is next touched; they are not author-facing.

## Language vocabulary

- **program** — one `.silc` file: a `@version` pragma followed by declarations.
- **declaration** — a top-level construct introduced by a keyword and a name:
  `subset`, `contract`, `component`, `resource`, `app`, `game`, `loop`,
  `service`, `processor`, `task`. Earlier docs called these "direct
  declarations" (as opposed to the removed `class X is …` spelling); plain
  **declaration** is the current word.
- **root** — a declaration that owns a tree of nodes: `app` (routes to
  components, whose `render()` holds `ui::` nodes), `game` (`scene::scene`),
  `loop` (`loop::flow`).
- **namespace** — the prefix before `::`. Namespaces are closed: `ui`, `game`,
  `loop`, `scrape`, `doc`, `tensor`, `llm`, `text`, `service`, and the
  stub-only set listed in [SILC-LANGUAGE.md](SILC-LANGUAGE.md).
- **node** — a namespaced tree element `ns::name(options…, children…)` that is
  *synthesized* by the compiler rather than executed as a pipeline step. `ui::`,
  `game::`, and `loop::` entries are nodes.
- **primitive** — a `ui::` node. Every primitive is dual-surface.
- **operation** — a namespaced pipeline step `ns::name(options…)` reached by
  `==>` inside a `service`/`processor` method. Operations are **executable**
  (run today), **stub-only** (parse and route but do not execute), or
  **synthesized** (the compiler writes them; authors never do).
- **option** / **flag** — see the rule above. Options have a **kind**
  (string, number, identifier, expression, reference, template) and may be
  **closed**: only the listed identifier values are accepted.
- **closed enum** — the value set of a closed option, for example
  `:variant(primary|secondary|destructive|ghost)`.
- **catalog** — the closed, compiler-owned list of nodes for one namespace
  (`UI_COMPONENT_CATALOG`, `GAME_NODE_CATALOG`, `LOOP_NODE_CATALOG`) or of
  executable operations (`EXECUTABLE_OPS`). Catalogs are the source of truth;
  documentation is generated from or tested against them.
- **registry** — the executable-operation catalog specifically.
- **surface** — where a UI renders: **web** (React/Tailwind) or **terminal**
  (OpenTUI, with telnet as a headless fallback). **Dual-surface** means both,
  always, from one component tree.
- **capability** — a `query` or `mutation` declared on a `resource`. (Internal
  code also uses `UiCapabilities`/`ScrapeCapabilities` for derived runtime
  features; that meaning is internal and will be renamed.)
- **contract** — a typed data schema (`contract X { has T $.f; }`).
- **subset** — a refinement type with a closed `where` predicate.
- **component** — a UI unit with options (`has T $.x`), state
  (`has state T $.x`), slots, emitted events, queries, handlers, and a
  `render()` template.
- **resource** — a persistent collection bound to a contract with capabilities
  and optional idempotent seeds.
- **app** — a route table from paths to components.
- **game** — a WebGPU real-time scene tree. The namespace name is under review
  (see the refinement plan: generic `scene::` kernel plus `game::` gameplay
  layer).
- **loop** — scheduled, approval-gated, model-assisted work.
- **loop command** — a program with only `loop::manual` triggers and no `app`
  or `game`; `silc main.silc` runs it once and exits.
- **module** — the internal grouping for `service`, `processor`, and `task`.
  Author-facing text names the keyword, not "module".
- **feed** — the `==>` operator; left value flows into the right operation.
- **handler** — a component method invoked by an event (`:on(click(name))`).
- **trigger** — the first child of `loop::flow`: `schedule`, `manual`, or
  `on_mutation`.
- **step** — any `loop::` node after the trigger.
- **gate** — a `loop::gate`; conditions fail closed.
- **effect** — a `loop::write` or `loop::notify`; keyed and receipted.
- **version pragma** — `@version("0.6.0")`; must equal the compiler version.

## Compiler and runtime vocabulary

- **subject** — a durable semantic concept inside `sil-core` (`Contract`,
  `Component`, `Resource`, `App`, `Game`, `Loop`, `Module`, `Pipeline`,
  `Target`). Internal only; author-facing text says *declaration* or *root*.
  "Subject-first" named the 0.3.0 migration and is historical.
- **synthesize** — what the compiler does with substrate authors never write:
  web and terminal serving, HTTP routes, SQLite tables, IPC, worker code.
- **engine** — one of the pinned runtimes: **Bun**, **CPython**, **Go**.
- **worker** — one generated process for one engine.
- **supervisor** — the Rust process that builds and keeps workers alive.
- **kernel** — a compiler-owned long-lived runtime for one root: the **loop
  kernel** (Go) and the **real-time kernel** (Babylon on WebGPU, hosted by Bun).
  Always qualify which one.
- **silclm** — the compiler-pinned local language model used by
  `llm::complete`, `ui::chat`, `ui::search_input`, `loop::ask`, and
  `silc assist`.
- **assist** — `silc assist`, the experimental recursive authoring loop.
- **tier 1 / tier 2 routing** — engine selection from declaration kind and
  traits (tier 1) or from operation namespaces (tier 2).
- **IPC** — the Silc shared-buffer ABI (mmap slots plus UDS control frames).

## Retired vocabulary

Do not use these in author-facing text except to say they are gone.

- **portal**, **profile**, **PortalKind** (0.1; removed 0.2.0) — compiler
  modes that took over an application. Replaced by compositions of components
  and resources.
- **`is view`** (removed 0.2.0).
- **`class X is component`** and other `class … is …` spellings (removed
  0.3.0) — replaced by declarations.
- **`sink`**, **`method serve()`**, author-written **`ui::web`** /
  **`ui::terminal`**, **`ipc::*`**, **`store::*`**, **`resource::*`**
  pipelines (removed 0.4.0) — the compiler synthesizes all of them (ADR-009).
- **stdlib / component resolver** — there is no separate component standard
  library; out-of-box capability is the catalog.
- **SIL**, **Semantic Intent Language**, **meta-compiler**, **"first AI-native
  language"** — pre-release naming; the product is **Silc**.
- **lifecycle economics** — 0.1 README framing; the current thesis is "authors
  declare intent; the compiler owns substrate".
- **zero-copy IPC** as a current claim — ABI v1 carries schema-tagged JSON in
  shared memory; typed zero-copy views are future work.
