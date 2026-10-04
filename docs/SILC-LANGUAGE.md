# Silc language surface (0.6.0)

This is the normative description of what a `.silc` program may contain. ADRs
record *why*; this page records *what*. The compiler catalogs in `sil-core`
remain the source of truth for every node, option, and operation; where this
page and the compiler disagree, the compiler is right and this page has a bug.
Vocabulary follows [GLOSSARY.md](GLOSSARY.md). The full per-node option lists
are generated into
[`crates/silc/templates/AGENTS.md`](../crates/silc/templates/AGENTS.md).

## 1. Lexical structure

- Source files use the `.silc` extension only. `.raku` and `.sil` are rejected.
- An optional first line `#!/usr/bin/env silc` is permitted.
- `#` starts a comment to end of line.
- Identifiers: `[A-Za-z_][A-Za-z0-9_]*`. Declared names are `PascalCase`;
  options, fields, handlers, and node names are `snake_case`.
- Sigils: `$name` is a local or parameter; `$.name` is a component-owned
  option, state field, or query binding.
- Strings use double quotes. Numbers may carry a unit suffix
  (`250ms`, `2s`, `512MB`, `1GB`, `90fps`, `8cm`, `3m`, `14deg`, `12px`,
  `100rps`, `50ops`).
- Operators: `==>` (feed), `=>` (route and event target), `->` (loop variable),
  `::` (namespace), `&&`, `||`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `+`, `-`,
  `*`, `/`, unary `!` and `-`.
- Keywords: `subset`, `contract`, `component`, `resource`, `app`, `game`,
  `scene`, `loop`, `service`, `processor`, `has`, `state`, `method`, `query`,
  `mutation`, `seed`, `slot`, `emit`, `route`, `when`, `else`, `for`, `await`,
  `is`, `of`, `where`. `class`, `sink`, and `task` are recognised only to
  produce a migration diagnostic. `scene` and `game` are the same root;
  `game` remains an alias for one release.

## 2. Program structure

```silc
@version("0.6.0")

<declaration>*
```

- `@version("…")` is required and must equal the compiler version exactly.
- Declarations may appear in any order. Names share one namespace: no two
  declarations of any kind may share a name.
- A program is one of a small number of **shapes**, decided by which roots and
  operations it contains (section 10). A program is **runnable** when every
  operation it uses is executable; otherwise it compiles to inspectable stubs.

## 3. Types

- Built-in: `Str`, `UUID`, `Bool`, `Int`, `int32`, `int64`, `num32`, `num64`.
- Named: any `contract` or `subset` name.
- Arrays: `[T]`.
- Fixed vectors: `Vec[num32; N]`.

### Subsets

```silc
subset Uri of Str where { .starts-with("http") }
subset Emb384 of Vec[num32; 384];
```

The `where` predicate language is closed. With a `Str` base it accepts exactly
`.contains("lit")`, `.starts-with("lit")`, and `.ends-with("lit")`. Predicates
are checked at compile time for literals and at resource ingress for
subset-typed fields.

## 4. Contracts

```silc
contract Note {
    has Str $.author;
    has Str $.text;
}
```

A contract is a list of `has Type $.field;` lines. Contracts back resources,
form bindings, `loop::ask` answers (`:into(Contract)`), `doc::extract`
targets, and MCP arguments (`Contract.new(...)`).

## 5. Nodes and options (shared grammar)

Every `ui::`, `game::`, and `loop::` element is a **node**:

```text
ns::name( option* child* )
option  := :name(value) | :name            # the bare form is a flag
child   := ns::name(...) | ComponentName(...)   # component calls only inside ui
```

- Options come from the node's catalog entry; unknown options and unknown
  events are compile errors.
- Option kinds: string, number, identifier, expression, reference to a
  declared name, string template (loop only, section 9).
- **Closed options** accept only the listed identifier values
  (`:variant(primary)`, `:fire_mode(hitscan)`, `:mutation(update)`). The
  spelling is the bare identifier. A quoted string is a compile error with a
  fix-it (`:variant(primary)`, not a string).
- Named slots are options whose value is a node: `:app_bar(ui::app_bar(...))`.
- Children are positional and follow the options. Each catalog entry declares
  its child policy: none, any, or an allow-list.
- Catalog membership is closed. Authors cannot define new nodes in any
  namespace; they compose nodes inside components (`ui::`), scenes (`game::`),
  or flows (`loop::`).

## 6. Components, resources, apps (dual-surface UI)

### Component

```silc
component ItemCard {
    has Item $.item;                   # option supplied by the caller
    has state Str $.draft = "";        # local state
    slot actions;                      # named slot
    emit remove(Item);                 # emitted event
    query $.items = Inventory.list();  # resource query binding

    method render() {
        ui::card(
            ui::heading(:text($.item.name)),
            ui::button(:label("Delete"), :variant(destructive), :on(click(on_remove)))
        )
    }

    method on_remove() { emit remove($.item); }
}
```

- `render()` is the only template method. It returns one node tree built from
  `ui::` primitives and component calls `Name(:option(value), …)`.
- Template control flow: `when expr { … } else { … }` and
  `for expr -> $item { … }`.
- Handlers are `method name(Type $param?) { statements }`. Statements:
  `$.field = expr;`, `emit event(payload);`, `navigate("/path");`,
  `await expr;`, `submit();`, `Resource.mutation(args);`,
  `Processor.method();`.
- Expressions: literals, `$name`, `$.field`, member access, calls,
  `Type.new(:field(value))`, lists `[a, b]`, arithmetic, comparison, boolean,
  unary `!`/`-`.
- Event wiring is always an option named `on`: `:on(click(handler))`,
  `:on(submit(handler))`, `:on(select(handler))`, `:on(send(handler))`, and
  forwarding `:on(remove => on_delete)` for author-emitted events.
- Every primitive renders on web and terminal. No component may be
  surface-specific.

### Resource

```silc
resource Articles for Article {
    query list;
    query get;
    mutation create;
    mutation update;
    mutation delete;
    seed Article.new(:id("article-001"), :title("Hello"));
}
```

Capabilities expand to CRUD over SQLite and HTTP (`GET/POST /api/{table}`,
`GET/PUT/DELETE /api/{table}/:id`). Seeds are idempotent and require a stable
`:id`.

### App

```silc
app MyApp {
    route "/" => HomePage;
    route "/admin" => AdminPage;
}
```

An `app` is only a route table. Serving both surfaces, the HTTP routes, the
`/submit`, `/complete`, `/upload`, and `/scrape` endpoints, and the `/loops`
inbox are synthesized.

## 7. Scene (real-time WebGPU world)

```silc
scene Arena {
    scene::scene(:title("Arena"), :renderer(webgpu), children…)
}
```

- The root keyword is `scene`. `game Name` is a one-release alias of the same root.
- Exactly one `scene::scene` node per scene declaration.
- Kernel nodes are `scene::` (entity, mesh, light, camera, zone, asset, and the rest). Gameplay nodes are `game::` (pawn, weapon, encounter, and the rest). Writing a kernel node as `game::` is a compile error that names the `scene::` spelling.
- Nodes follow section 5. References to declared names inside the scene use
  string options today (`:ref("WalkDefault")`, `:prefab("Player")`,
  `:possess("Player")`); see section 12.
- A scene program may not contain `app`, `component`, `resource`, `loop`, or
  pipeline modules.

## 8. Loop (scheduled, approval-gated work)

```silc
loop RfiChase {
    loop::flow(
        loop::schedule(:cron("0 7 * * 1-5"), :tz("America/New_York")),
        loop::find(:as(overdue), :from(Rfis.list), :where($.status == "open" && $.due < $today), :max(50)),
        loop::each(:in($overdue), :as(rfi), :max(50),
            loop::ask(:as(draft), :into(Reminder), :from($rfi), :prompt("…")),
            loop::approve(:by("PM"), :message("Send?"), :show($draft), :within("2d"),
                loop::declined(loop::skip(:reason("declined"))),
                loop::timed_out(loop::skip(:reason("expired")))),
            loop::write(:to(Reminders.create), :value($draft), :key("rfi:{$rfi.id}:{$today}"))
        )
    )
}
```

- `loop::flow` has exactly one trigger first (`schedule`, `manual`,
  `on_mutation`) then steps in order.
- Bounds are mandatory: `:max` on `find`/`each`, `:retry` on `ask`/`read`,
  `:within` on `approve` (at most `30d`).
- Model output (`loop::ask` and anything derived from it) reaches an effect
  only through a `gate`, an `approve`, or an explicit `:unchecked("reason")`.
- Gates fail closed; `otherwise`, `declined`, and `timed_out` blocks end in
  `stop`, `fail`, or `skip` (`skip` only inside `each`).
- Effects (`write`, `notify`) carry a `:key` template with at least one
  placeholder; the kernel performs each key once.
- Reserved bindings: `$today`, `$now`, `$event`, `$calendar`
  (`today`, `weekday`, `last7_start`, `last7_end`, `next7_end`).
- Outside data: `loop::read(:as(x), scrape::page(:url(...)))` or
  `loop::read(:as(x), mcp::call(:server(...), :tool(...),
  :args(Contract.new(...)), :auth_env("VAR"), :select("path")))`. The result
  binds `$x.text` and `$x.data`. The string form `:op("scrape::page")` remains
  a one-release alias.
- A program with no `app`, no `scene` (and no `game` alias), and only `loop::manual` triggers is a
  **loop command**: no surfaces are built; `silc main.silc` runs each loop
  once, prints notices to stdout, and exits. `loop::approve` is a compile
  error in a command.

## 9. String templates

Inside `loop::` string options (`:prompt`, `:text`, `:message`, `:key`,
`:reason`) and inside `ui::` string options, the form `{$binding.path}`
interpolates a bound value. Both namespaces use the same placeholder checker.
A string that contains `{$` and is not a legal placeholder list is a compile error.

## 10. Pipelines and operations

```silc
service ArticleIngress {
    method fetch_article() {
        target_url ==> scrape::page(:js(false)) ==> scrape::extract(:into(ArticlePayload))
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

- `service` and `processor` hold `method` bodies that are feed chains.
  `task` was removed in 0.6.0; the compiler routes by operation.
  A chain starts from a parameter, a field, a bare name, or a contract name and
  threads left to right through operations.
- `is trait(value)` after the declaration name is accepted on these three
  keywords only, for compiler-consumed hints; authors do not declare storage
  or sinks.
- **Executable operations (0.6.0):** `service::http`, `text::score`,
  `llm::complete`, `scrape::page`, `scrape::site`, `scrape::select`,
  `scrape::render`, `scrape::extract`, `doc::extract`, `tensor::tokenize`,
  `tensor::infer`.
- **Stub-only namespaces:** `http`, `html`, `numpy`, `pandas`, `ws`, `sys`,
  `schema`, `payload`, `json`, plus any non-registry name under an executable
  namespace. They parse, route, and emit stubs. Mixing a stub-only operation
  into a runnable graph is a compile error.
- **Synthesized (never authored):** `ui::web`, `ui::terminal`, `resource::*`,
  `ipc::*`, `store::*`, `sink`, `method serve()`.
- Engine assignment comes from the operation registry (ADR-004); authors never name Bun, CPython, or Go.

## 11. Program shapes and compatibility

The compiler currently enforces these exclusions. They are listed here so the
rule set is visible in one place; the refinement plan replaces the scattered
checks with one declared matrix that generates this section.

- A `scene` (or the `game` alias) may not coexist with `app`, `component`, `resource`, `loop`, or
  pipeline modules.
- `text::score` may not coexist with the local model (`llm::complete` or `loop::ask`),
  `scrape::*`, or `doc::*`.
- `text::score` and the local model may not coexist with `tensor::infer`.
- `loop` declarations may not coexist with `scrape::*` or `tensor::*`
  pipelines (use `loop::read` for pages).
- `doc::extract` requires `:into(Contract)` and a matching `resource`.
- An API-only program (`service::http`) may not declare processors.
- UI programs require an `app` with at least one `route`.
- At most one processor operation family per program.

## 12. Resolved in 0.6.0, and what remains

Resolved in 0.6.0:

1. `loop` and `scene` are lexer keywords, like the other roots. `loop` is still
   a legal closed value in `scene::audio(:kind(loop))`.
2. `loop::read` nests the operation (`scrape::page(...)` or `mcp::call(...)`).
   The string `:op("…")` remains a one-release alias. `mcp` is a registered namespace.
3. Closed-enum values are bare identifiers. A string form is a fix-it diagnostic.
4. `{$binding.path}` uses one checker in `loop::` and `ui::` string options.
5. `task` is removed. Routing follows the operation.
6. `llm::complete` and `loop::ask` are one local-model capability in the compatibility matrix.
7. Kernel nodes are `scene::`. Gameplay nodes are `game::`. The root is `scene`; `game` is a one-release alias.
8. Author `sink`, `App.serve`, and discarded component pipelines are gone.

Still irregular:

1. References to declared names are strings in scene nodes (`:ref("X")`) and
   identifiers in `loop::` (`:from(Rfis.list)`) and `ui::` (`:on(click(h))`).
2. `$.field` means "the component's own field" in components and "the current
   row" inside `loop::find(:where(…))`.
3. A source that still says `@version("0.5.0")` is rejected with a migration
   diagnostic that lists each kernel node in that program written `game::`
   instead of `scene::`.
