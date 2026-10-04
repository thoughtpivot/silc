<!-- BEGIN SILC_AGENTS_TEMPLATE -->
# Silc project guidance for AI tools

This directory is a **Silc 0.5.0** project. Silc (said like “silk”) is an
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

- ADR index: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-INDEX.md
- Surface syntax: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-002-silc-surface-syntax.md
- Pipeline feeds (`==>`): https://github.com/thoughtpivot/silc/blob/main/docs/ADR-007-pipeline-feeds.md
- Declarative UI: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-003-declarative-ui.md
- Synthesized runtime (0.5.0): https://github.com/thoughtpivot/silc/blob/main/docs/ADR-009-compiler-synthesized-runtime.md
- Local LLM: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-005-local-llm-complete.md
- Scrape: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-006-scrape-namespace.md
- Document extract: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-011-document-extract.md
- Tensor / MiniLM: https://github.com/thoughtpivot/silc/blob/main/docs/ADR-010-tensor-minilm-pipeline.md
- Architecture: https://github.com/thoughtpivot/silc/blob/main/docs/ARCHITECTURE.md
- Examples: https://github.com/thoughtpivot/silc/tree/main/examples

## Workflow

```bash
silc init myapp
cd myapp
silc build main.silc          # compile + validate
silc main.silc                # web by default
silc main.silc --terminal     # also attach OpenTUI (+ telnet fallback)
silc main.silc | pbcopy       # a loop command: run once, result on stdout, exit
```

Treat compiler diagnostics as authoritative. Prefer `silc build` after each
meaningful edit. Stop and report limits instead of inventing substrates.

## Silc 0.5.0 authoring model

| Construct | Role |
| --- | --- |
| `@version("0.5.0")` | Required exact source-version annotation |
| `subset Name of Base where { … }` | Semantic type alias; v1 `where` predicates (Str): `.contains` / `.starts-with` / `.ends-with` (ADR-002) |
| `contract X { has T $.f; }` | **Contract** — typed data schema |
| `component X` | **Component** — options (`has`), `has state`, slots, `emit`, handlers, `render()` |
| `resource X for Contract` | **Resource** — capability CRUD (`query list;`, `mutation create;`, …) |
| `app X` | **App** — `route` table (dual-surface serving is synthesized) |
| `game X` | **Game** — web-only WebGPU scene tree (`game::scene(...)`; ADR-012). Do not mix with `app` / UI routes |
| `loop X` | **Loop** — scheduled, approval-gated, model-assisted work (`loop::flow(...)`; ADR-014). Runs beside an `app`; the compiler adds the `/loops` inbox. With no `app` and only `loop::manual` triggers it is a **command**: `silc main.silc` runs each loop once, prints its notices to stdout, and exits |
| `service X` / `processor X` / `task X` | Optional workflow modules |
| `==>` | Pipeline feed between values and `ns::operation(...)` calls |

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
- Arithmetic / comparison / boolean operators, unary `!` / `-`
- Assignment to component state: `$.field = expr;`
- Lists: `[a, b]`
- `emit event(payload)`, `navigate("/path")`, `await expr`
- Template control: `when expr { … } else { … }`, `for expr -> $item { … }`

## Dual-surface UI (required)

Authors write **one** semantic component tree. The compiler lowers it to:

- `ui::web` → React/Tailwind (compiler-owned)
- `ui::terminal` → OpenTUI (compiler-owned); TCP telnet CLI is a remote fallback

Every UI `app` synthesizes both surfaces automatically (web + terminal).
Authors declare routes only — never `method serve()`, `ui::web`, or `ui::terminal`.
No component may be web-only or terminal-only. Never write HTML, CSS, React,
Tailwind, OpenTUI, ShadCN trees, or bundler config in Silc source.
`silc main.silc` serves web only; pass `--terminal` (or `SILC_TERMINAL=1`) to
attach OpenTUI and the telnet CLI. Override ports with `SILC_HTTP_PORT` /
`SILC_TERMINAL_PORT` when the terminal surface is attached.

### Shared option vocabulary

| Concern | Shape | Closed values / notes |
| --- | --- | --- |
| State bind | `:field(name)` | forms, tabs, filters |
| Display | `:value(expr)` | controlled inputs |
| Role | `:variant(...)` | `primary` \| `secondary` \| `destructive` \| `ghost` |
| Tone | `:tone(...)` | `default` \| `muted` \| `info` \| `success` \| `warning` \| `danger` |
| Size | `:size(...)` | `sm` \| `md` \| `lg` |
| Capability flags | bare flags | `:disabled`, `:sortable`, `:searchable`, `:selectable`, `:dense`, `:active`, `:submit`, `:dismissible`, `:collapsible` |

Unknown closed tokens are compile errors. `:field` stays an option pattern;
`ui::field` is optional chrome around a control.

### Complete UI primitive catalog

Every builtin is dual-surface (`web+terminal`). Lines below are the canonical
API contract (options / events / slots / children).

#### Shell and navigation

- `ui::page` — options: none; events: none; slots: `app_bar`→`app_bar`, `side_panel`→`side_panel`, `footer`→`footer`; children: anyOf(`stack`, `row`, `grid`, `card`, `heading`, `text`, `form`, `text_input`, `textarea`, `file_input`, `radio_group`, `select`, `checkbox`, `switch`, `field`, `button`, `toolbar`, `chat`, `chat_history`, `search_input`, `filter_bar`, `collection`, `list`, `table`, `badge`, `alert`, `divider`, `section`, `description_list`, `tabs`, `dialog`, `loading`, `empty`, `nav_item`); surfaces: web+terminal
- `ui::app_bar` — options: `title` (required); events: none; slots: none; children: none; surfaces: web+terminal
- `ui::side_panel` — options: none; events: none; slots: none; children: anyOf(`nav_item`); surfaces: web+terminal
- `ui::nav_item` — options: `label` (required), `to?`, `active?` (flag); events: `click`; slots: none; children: none; surfaces: web+terminal
- `ui::toolbar` — options: none; events: none; slots: none; children: anyOf(`button`); surfaces: web+terminal
- `ui::footer` — options: none; events: none; slots: none; children: any; surfaces: web+terminal

#### Layout

- `ui::stack` — options: none; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::row` — options: none; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::grid` — options: none; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::card` — options: none; events: none; slots: `actions`→`row`; children: any; surfaces: web+terminal
- `ui::section` — options: `title?`, `description?`; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::divider` — options: `label?`; events: none; slots: none; children: none; surfaces: web+terminal
- `ui::heading` — options: `text` (required), `level?`; events: none; slots: none; children: none; surfaces: web+terminal
- `ui::text` — options: `text` (required); events: none; slots: none; children: none; surfaces: web+terminal

#### Forms

- `ui::form` — options: none; events: `submit`; slots: none; children: anyOf(`stack`, `row`, `grid`, `card`, `heading`, `text`, `text_input`, `textarea`, `file_input`, `radio_group`, `select`, `checkbox`, `switch`, `field`, `button`, `toolbar`, `badge`, `alert`, `divider`, `section`, `loading`, `empty`); surfaces: web+terminal
- `ui::text_input` — options: `field?`, `value?`, `label?`, `placeholder?`, `disabled?` (flag); events: `input`, `change`; slots: none; children: none; surfaces: web+terminal
- `ui::textarea` — options: `field?`, `value?`, `label?`, `disabled?` (flag); events: `input`, `change`; slots: none; children: none; surfaces: web+terminal
- `ui::file_input` — options: `field?`, `label?`, `accept?`, `multiple?` (flag), `disabled?` (flag); events: `change`; slots: none; children: none; surfaces: web+terminal
- `ui::radio_group` — options: `field?`, `value?`, `options` (required), `label?`, `disabled?` (flag); events: `change`; slots: none; children: none; surfaces: web+terminal
- `ui::select` — options: `field?`, `value?`, `options` (required), `label?`, `placeholder?`, `disabled?` (flag); events: `change`; slots: none; children: none; surfaces: web+terminal
- `ui::checkbox` — options: `field?`, `label` (required), `checked?`, `disabled?` (flag); events: `change`; slots: none; children: none; surfaces: web+terminal
- `ui::switch` — options: `field?`, `label` (required), `checked?`, `disabled?` (flag); events: `change`; slots: none; children: none; surfaces: web+terminal
- `ui::field` — options: `label?`, `hint?`, `error?`; events: none; slots: none; children: anyOf(`stack`, `row`, `grid`, `card`, `heading`, `text`, `text_input`, `textarea`, `file_input`, `radio_group`, `select`, `checkbox`, `switch`, `field`, `button`, `toolbar`, `badge`, `alert`, `divider`, `section`, `loading`, `empty`); surfaces: web+terminal
- `ui::button` — options: `label` (required), `variant?`, `size?`, `submit?` (flag), `active?`, `disabled?` (flag); events: `click`; slots: none; children: none; surfaces: web+terminal

#### Chat and search

- `ui::chat` — options: `field?`, `value?`, `label?`, `placeholder?`, `session?`, `loading?`, `error?`, `context?`, `persona?`; events: `send`; slots: none; children: none; surfaces: web+terminal
- `ui::chat_history` — options: `title?`, `items?`, `collapsible?` (flag); events: none; slots: none; children: none; surfaces: web+terminal
- `ui::search_input` — options: `field?`, `value?`, `label?`, `placeholder?`, `context?`, `persona?`; events: `input`, `submit`; slots: none; children: none; surfaces: web+terminal
- `ui::filter_bar` — options: none; events: none; slots: none; children: anyOf(`search_input`, `button`, `text_input`); surfaces: web+terminal

#### Data display

- `ui::collection` — options: `items` (required), `empty_text?`; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::list` — options: `items?`; events: none; slots: none; children: any; surfaces: web+terminal
- `ui::table` — options: `rows` (required), `columns` (required), `empty_text?`, `filter_field?`, `filter_column?`, `filter_all?`, `sortable?` (flag), `searchable?` (flag), `selectable?` (flag), `dense?` (flag); events: `select`; slots: none; children: none; surfaces: web+terminal
- `ui::description_list` — options: `items` (required); events: none; slots: none; children: none; surfaces: web+terminal

#### Feedback and overlays

- `ui::badge` — options: `text` (required), `tone?`; events: none; slots: none; children: none; surfaces: web+terminal
- `ui::alert` — options: `text` (required), `title?`, `tone?`, `dismissible?` (flag), `auto_dismiss_ms?`; events: `dismiss`; slots: none; children: none; surfaces: web+terminal
- `ui::tabs` — options: `field?`, `value?`; events: `change`; slots: none; children: anyOf(`tab`); surfaces: web+terminal
- `ui::tab` — options: `label` (required), `value` (required); events: none; slots: none; children: any; surfaces: web+terminal
- `ui::dialog` — options: `open` (required), `title?`; events: `confirm`, `cancel`; slots: none; children: any; surfaces: web+terminal
- `ui::loading` — options: `text?`; events: none; slots: none; children: none; surfaces: web+terminal
- `ui::empty` — options: `text?`; events: none; slots: none; children: none; surfaces: web+terminal

### Complete game::* catalog (ADR-012)

WebGPU-only. One `game Name { game::scene(...) }` root. Godot tree+signals, Unity prefabs/data/components, Unreal mode/pawn/controller. Do not mix with `app` / `component` / `resource`.

- `game::scene` — options: `title`, `renderer?`, `target_fps?`; children: `entity`, `prefab`, `spawn`, `data`, `asset`, `generate`, `material`, `mode`, `controller`, `camera`, `post_process`, `overlay`, `hud`, `environment`, `shadow`, `zone`, `weapon`, `encounter`, `objective`, `signal`, `group`, `tilemap`, `parallax`, `particle_effect`, `floating_text`
- `game::entity` — options: `name`, `x?`, `y?`, `z?`, `yaw?`, `pitch?`, `roll?`, `sx?`, `sy?`, `sz?`; children: `entity`, `mesh`, `light`, `collider`, `movement`, `attribute`, `pawn`, `ability`, `weapon`, `ammo`, `damage`, `pickup`, `npc`, `perception`, `behavior`, `mind`, `nav_agent`, `door`, `trigger`, `cover`, `audio`, `signal`, `group`, `spawn`, `sprite`, `collectible`, `interactable`, `patrol`, `warp`, `level_end`, `state_machine`, `particle_effect`
- `game::prefab` — options: `name`, `x?`, `y?`, `z?`, `yaw?`, `pitch?`, `roll?`, `sx?`, `sy?`, `sz?`; children: `entity`, `mesh`, `light`, `collider`, `movement`, `attribute`, `pawn`, `ability`, `weapon`, `ammo`, `damage`, `pickup`, `npc`, `perception`, `behavior`, `mind`, `nav_agent`, `door`, `trigger`, `cover`, `audio`, `signal`, `group`, `spawn`, `sprite`, `collectible`, `interactable`, `patrol`, `warp`, `level_end`, `state_machine`, `particle_effect`
- `game::spawn` — options: `prefab`, `x?`, `y?`, `z?`, `as_pawn?` (flag); children: none
- `game::data` — options: `name`, `speed?`, `jump_height?`, `gravity?`, `cooldown?`, `cost?`, `damage?`, `range?`, `fire_rate?`, `magazine?`, `reload?`, `spread?`, `pellet_count?`, `charge_time?`, `splash_radius?`, `cadence_s?`, `persona?`, `aggression?`, `morale?`, `health?`, `armor?`; children: none
- `game::signal` — options: `name`, `on?`; children: none
- `game::group` — options: `name`; children: none
- `game::mesh` — options: `shape?`, `asset?`, `material?`, `size?`, `color?`; children: none
- `game::light` — options: `kind`, `intensity?`, `color?`, `radius_m?`, `cast_shadows?`; children: none
- `game::collider` — options: `shape`, `size?`; children: none
- `game::movement` — options: `style?`, `speed?`, `jump_speed?`, `sprint_mul?`, `ref?`; children: none
- `game::attribute` — options: `name`, `value?`, `max?`; children: none
- `game::mode` — options: `id`, `possess?`; children: `spawn`, `encounter`, `objective`
- `game::pawn` — options: none; children: none
- `game::controller` — options: `scheme?`; children: none
- `game::camera` — options: `mode?`, `distance_m?`, `shoulder_offset_m?`, `follow?`; children: none
- `game::ability` — options: `name`, `key`, `cooldown?`, `cost?`, `cost_attr?`, `ref?`; children: `particle_emitter`, `dynamic_light`, `camera_impulse`, `audio`
- `game::particle_emitter` — options: `kind`, `count?`; children: none
- `game::dynamic_light` — options: `radius_m?`, `intensity?`, `color?`; children: none
- `game::camera_impulse` — options: `strength?`; children: none
- `game::post_process` — options: `stage`, `enabled?`; children: none
- `game::overlay` — options: `toggle`; children: none
- `game::asset` — options: `name`, `path`, `kind`; children: none
- `game::material` — options: `name`, `albedo?`, `normal?`, `roughness?`, `metallic?`, `ao?`, `emissive?`, `tiling?`; children: none
- `game::zone` — options: `name`, `kind`; children: `entity`, `spawn`, `light`, `signal`, `group`
- `game::weapon` — options: `name`, `slot?`, `fire_mode`, `ref?`, `damage?`, `fire_rate?`, `magazine?`, `reload?`, `spread?`; children: `projectile`, `particle_emitter`, `dynamic_light`, `camera_impulse`, `audio`
- `game::projectile` — options: `kind`, `speed?`, `lifetime?`, `splash_radius?`, `color?`, `size?`; children: none
- `game::ammo` — options: `name`, `amount?`, `max?`; children: none
- `game::damage` — options: `amount`, `type_ident`; children: none
- `game::pickup` — options: `kind`, `ref`, `amount?`; children: none
- `game::hud` — options: `show_crosshair?`, `show_ammo?`, `show_health?`, `score_label?`; children: none
- `game::npc` — options: `archetype`, `faction`; children: none
- `game::perception` — options: `sight_m?`, `hear_m?`, `fov_deg?`; children: none
- `game::behavior` — options: `tree`, `default_tactic?`; children: none
- `game::mind` — options: `ref`, `cadence_s?`; children: none
- `game::nav_agent` — options: `radius?`, `height?`, `max_speed?`; children: none
- `game::encounter` — options: `id`, `wave?`; children: `spawn`
- `game::objective` — options: `id`, `kind`, `target?`; children: none
- `game::audio` — options: `kind`, `path?`, `ref?`, `volume?`; children: none
- `game::environment` — options: `fog_density?`, `fog_color?`, `sky_color?`, `exposure?`; children: `clouds`, `stars`
- `game::shadow` — options: `enabled?`, `cascade_count?`; children: none
- `game::clouds` — options: `count?`, `altitude?`, `spread?`, `speed?`, `scale?`, `color?`, `opacity?`; children: none
- `game::stars` — options: `count?`, `altitude?`, `size?`, `color?`, `opacity?`, `twinkle?`; children: none
- `game::door` — options: `state?`, `auto?`; children: none
- `game::trigger` — options: `kind`, `on`; children: none
- `game::cover` — options: `quality`; children: none
- `game::sprite` — options: `atlas`, `frame?`, `width?`, `height?`, `animation?`, `flip_x?`, `billboard?`; children: none
- `game::tilemap` — options: `asset`, `tileset`, `tile_size?`, `collision_layer?`; children: none
- `game::collectible` — options: `kind`, `value?`, `on_collect?`, `respawn?`; children: none
- `game::interactable` — options: `kind`, `contents?`, `health?`, `on_interact?`; children: none
- `game::patrol` — options: `behavior`, `speed?`, `bounds?`, `on_stomp?`, `on_touch?`; children: none
- `game::warp` — options: `target`, `direction?`, `on_warp?`; children: none
- `game::level_end` — options: `on_complete?`, `next_level?`; children: none
- `game::state_machine` — options: `initial`, `on_stomp_state?`, `on_hit_state?`, `on_touch_state?`, `death_delay?`, `on_state_change?`; children: none
- `game::parallax` — options: `texture`, `depth`, `y?`, `scale?`, `repeat_x?`, `tint?`; children: none
- `game::particle_effect` — options: `id`, `preset?`, `count?`, `speed?`, `spread?`, `lifetime?`, `gravity?`, `color?`, `on_trigger?`; children: none
- `game::floating_text` — options: `on_trigger`, `prefix?`, `color?`, `duration?`, `rise_speed?`; children: none
- `game::generate` — options: `type`, `name`, `preset?`, `style?`, `frame_size?`, `palette?`, `animations?`, `export?`; children: none

Closed enums: `game::scene` `:renderer(webgpu)`; `game::mesh` `:shape(plane|box|capsule|sphere)`; `game::light` `:kind(directional|point|spot)`; `game::collider` `:shape(box|capsule|plane)`; `game::movement` `:style(walk|first_person|sprint|jump|platformer)`; `game::controller` `:scheme(wasd_mouse|arrows_jump)`; `game::camera` `:mode(third_person|first_person|side_scroll)`; `game::particle_emitter` `:kind(burst|spark|smoke)`; `game::post_process` `:stage(taa|ssao|ssr|dof|bloom|tonemap|grain|sharpen)`; `game::asset` `:kind(gltf|texture|audio|navmesh)`; `game::zone` `:kind(room|walkway|outdoor)`; `game::weapon` `:fire_mode(hitscan|pellet|projectile|beam)`; `game::projectile` `:kind(tracer|shell|plasma|rail)`; `game::damage` `:type_ident(bullet|pellet|plasma|rail|melee)`; `game::pickup` `:kind(weapon|ammo|health)`; `game::npc` `:archetype(suppressor|flanker|breacher)`; `game::npc` `:faction(hostile|neutral)`; `game::behavior` `:tree(patrol_combat|guard)`; `game::behavior` `:default_tactic(suppress|flank|push|retreat)`; `game::objective` `:kind(clear_hostiles|reach)`; `game::audio` `:kind(oneshot|loop)`; `game::door` `:state(open|closed)`; `game::trigger` `:kind(enter|exit)`; `game::cover` `:quality(low|med|high)`; `game::collectible` `:kind(coin|gem|health|powerup|key|custom)`; `game::interactable` `:kind(breakable|bumpable|switchable|container)`; `game::patrol` `:behavior(walk_reverse|walk_fall|stationary|follow|flee)`; `game::warp` `:direction(down|up|left|right)`; `game::particle_effect` `:preset(burst|sparkle|debris|dust|trail)`; `game::generate` `:type(sprite|texture|material)`; `game::generate` `:preset(character|enemy|item|tile|effect)`; `game::generate` `:style(pixel_8|pixel_16|pixel_32|flat|outline)`. `game::mesh` takes `:asset` XOR `:shape`.

### Complete loop::* catalog (ADR-014)

Declare `loop Name { loop::flow(trigger, steps...) }`. Exactly one trigger
(`schedule`, `manual`, or `on_mutation`) comes first; steps run in order. The
compiler adds a `/loops` inbox (approvals, Run now, runs, notices) on both
surfaces and a Go loop kernel that records every outside input and model answer
so a resumed run replays instead of redoing work. A program with no `app`, no
`game`, and only `loop::manual` triggers is a **loop command**: no inbox and no
surfaces are built; `silc main.silc` runs every loop once, narrates each step on
stderr, prints each run's notices to stdout (one per line), and exits non-zero
if any run failed.

- `loop::flow` — options: none; children: one trigger, then steps
- `loop::schedule` — options: `cron`, `tz`, `catch_up?`; children: none
- `loop::manual` — options: none; children: none
- `loop::on_mutation` — options: `resource`, `mutation`; children: none
- `loop::let` — options: `as`, `value`; children: none
- `loop::find` — options: `as`, `from`, `where?`, `order?`, `desc?` (flag), `max?`, `one?` (flag); children: none
- `loop::read` — options: `as`, `op`, `url?`, `server?`, `tool?`, `args?`, `auth_env?`, `select?`, `retry?`; children: none
- `loop::ask` — options: `as`, `into`, `prompt`, `from?`, `retry?`; children: `otherwise`
- `loop::gate` — options: `that`, `reason`; children: `otherwise`
- `loop::branch` — options: none; children: `when`, `otherwise`
- `loop::when` — options: `that`; children: steps
- `loop::otherwise` — options: none; children: steps
- `loop::each` — options: `in`, `as`, `max`; children: steps
- `loop::write` — options: `to`, `value`, `key`, `unchecked?`; children: none
- `loop::notify` — options: `to`, `text`, `key`, `unchecked?`; children: none
- `loop::approve` — options: `by`, `message`, `show?`, `within`, `as?`; children: `declined`, `timed_out`
- `loop::declined` — options: none; children: steps
- `loop::timed_out` — options: none; children: steps
- `loop::stop` — options: `reason?`; children: none
- `loop::fail` — options: `reason`; children: none
- `loop::skip` — options: `reason`; children: none

Rules the compiler enforces:

- Every `find` and `each` has `:max`; every `write` and `notify` has a `:key` with at least one placeholder; every `approve` has `:within` (max `30d`).
- `loop::ask` output (and anything built from it) must be referenced by a `gate`, shown by an `approve`, or the effect must say `:unchecked("why")`.
- Gates fail closed: a missing field makes a condition unknown, and unknown never passes.
- The `otherwise` of `gate`/`ask` and the `declined`/`timed_out` blocks end in `stop`, `fail`, or `skip`; `skip` only inside `each`.
- Reserved bindings: `$today` and `$now` (loop time zone), `$event` (trigger data), `$calendar` (`today`, `weekday`, `last7_start`, `last7_end`, `next7_end`).
- `loop::read(:op("mcp::call"), :server("https://…/mcp"), :tool("name"), :args(Contract.new(...)), :auth_env("TOKEN_VAR"), :select("records.parsed"))` calls one MCP tool; the result is `$x.text` and `$x.data`. Tokens come from the environment, never from source.
- Run now in `/loops` starts `manual` and `schedule` loops. A loop never overlaps itself.
- A loop command cannot use `loop::approve` (nobody is there to answer); use `loop::gate`, or add an `app` to get the `/loops` inbox.
- `silc build` prints worst-case model calls, effects, approvals, and reads per run.

## Valid patterns

### Component with state and events

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
```

### Author component composition + emit forwarding

```silc
component ItemCard {
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
component BrowsePage {
    query $.items = InventoryItems.list();
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

### App with routes (dual-surface synthesized)

```silc
app MyApp {
    route "/" => HomePage;
    route "/admin" => AdminPage;
}
```

### Resource CRUD

```silc
resource Products for Product {
    query list;
    query get;
    mutation create;
    mutation update;
    mutation delete;
}
```

Derived HTTP (compiler-owned): `GET/POST /api/{table}`,
`GET/PUT/DELETE /api/{table}/:id`.

### Resource seeds (idempotent)

```silc
resource Articles for Article {
    query list;
    mutation create;
    mutation update;
    mutation delete;
    seed Article.new(
        :id("article-001"),
        :title("Hello"),
        :body("Short body."),
        :author("Ada"),
        :published_at("2026-01-15"),
        :year("2026"),
        :month("January")
    );
}
```

Seeds are compiler-owned `INSERT OR IGNORE` rows. Every seed must construct the
resource contract and include a stable `:id("…")` string so restarts never
overwrite admin edits.

### Table row select

```silc
ui::table(
    :rows($.articles),
    :columns(["title", "author"]),
    :selectable,
    :on(select(on_select))
)

method on_select(Article $article) {
    $.selected_id = $article.id;
}
```

`:on(select(…))` passes the clicked/activated row object to the handler on both
web and terminal surfaces.

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

### Feed filter with silclm

```silc
has state Str $.filter_query = "";
has state Str $.match_ids = "*";       # "*" = show all; "__none__" = empty
has state Bool $.filtering = false;

ui::search_input(
    :field(filter_query),
    :context($.articles),
    :persona("Return ONLY a JSON array of matching article id strings."),
    :on(submit(on_filter))
)

method on_filter() { Assistant.complete(); }

# in the feed:
when $.match_ids.contains($article.id) { ArticleCard(:article($article)) }
```

`ui::search_input` with `:context` + `:persona` lowers to an AI filter that
parses a JSON id array from silclm and stores it in `$.match_ids`.

### Processor (score or LLM)

```silc
processor Assistant {
    method complete(ChatRecord $record) {
        $record.prompt ==> llm::complete()
    }
}
```

`text::score` and `llm::complete` cannot both appear in one program. Each needs
exactly one processor. SQLite persistence is synthesized by the compiler — do
**not** declare `sink`, `ipc::*`, or `store::*`.

### API-only service

```silc
service Api {
    method create(Note $note) {
        $note ==> service::http(:port(8080), :route("/notes"), :method(POST))
    }
}
```

API-only programs must not declare processor modules.

Wire handlers with `:on(click(handler))`, `:on(submit(handler))`, navigation
with `ui::nav_item(:to("/path"))`, collections with `for $.items -> $item { … }`,
and conditionals with `when expr { … }`.

## Runnable operations (0.5.0)

Author-facing executable operations today (registry in `sil-core`):

`service::http`, `text::score`, `llm::complete`,
`scrape::page`, `scrape::site`, `scrape::select`, `scrape::render`,
`scrape::extract`, `doc::extract`, `tensor::tokenize`, `tensor::infer`.

Compiler-synthesized (do **not** write in `.silc`): dual-surface `ui::web` /
`ui::terminal` serving, `resource::*` CRUD pipelines, and `ipc`/`store` sink
persistence.

Local LLM chat uses **silclm** (default catalog id). Prefer
`llm::complete(:model("silclm"))` or omit `:model`. Do not invent Ollama,
OpenAI, or ad-hoc GGUF paths in `.silc`. Legacy alias `llama3.2-1b` resolves to
`silclm` for one release.

Scraping uses **`scrape::*`** (ADR-006). Authors never name Bun, Colly, or
Playwright. Prefer `scrape::site` for crawls and `scrape::page` /
`scrape::select` for single pages. Do **not** use stub `http::get` /
`html::extract_body` in runnable programs — migrate to `scrape::*`.

Document upload + extract uses **`doc::extract`** with **`ui::file_input`**
(ADR-011). Declare `$upload ==> doc::extract(:into(Document))` plus a
`resource … for Document`. The compiler synthesizes multipart `POST /upload`,
Python-native extract (PDF/DOCX/ODT/MD/TXT/HTML — no Pandoc), and SQLite rows.
Original file bytes are discarded after extract. Do not mix `doc::*` with
`text::score`.

Pipeline-only programs use `scrape::page ==> scrape::extract`, then
`tensor::tokenize(:model("minilm-l6-v2")) ==> tensor::infer(:prefer(CPU))`.
Persistence is synthesized. Their contract must carry `raw_content` and
`vector_embedding: Emb384`, where `subset Emb384 of Vec[num32; 384]`. Run them
with `silc run main.silc --input-json '{"url":"https://…"}'`. CUDA and
arbitrary tensor models/shapes are not executable in 0.5.0.

Stub-only namespaces (parse/route/emit, do not run): `http`, `html`,
`numpy`, `pandas`, `ws`, `sys`, `schema`, `payload`, `json`, plus non-registry
operations under runnable namespaces. Mixing stub-only operations into a runnable graph is a
**compile error**.

## Generated runtime surfaces

Compiler-owned (do not invent alternatives):

- `POST /submit` — form `submit()` handlers (also scrape jobs when `scrape::*` is present)
- `POST /scrape` — explicit scrape ingest when `scrape::*` is present
- `POST /upload` — multipart file upload when `doc::extract` is present
- `POST /complete` — chat / `*.complete()` processors
- `GET|POST|PUT|DELETE /api/{table}` — resource queries/mutations
- Web: React app served by Bun (`silc main.silc`)
- Terminal: OpenTUI + telnet CLI when run with `--terminal` / `SILC_TERMINAL=1`

## Validation constraints agents must respect

1. UI apps require an `app` declaration with non-empty `route`s; dual-surface
   web/terminal serving is synthesized (default ports 18088 / 18023). Runtime
   attaches the terminal surface only with `--terminal` (or `SILC_TERMINAL=1`).
2. Every builtin UI node must use catalog options/events; unknown options/events fail.
3. Closed enums (`:variant`, `:tone`, `:size`) reject unknown tokens.
4. Resource `query` bindings must reference real resource query methods.
5. Do not mix `text::score` and `llm::complete`.
6. Do not mix `scrape::*` with `text::score`. Scrape pipelines may use
   `llm::complete` for grounded SilcLM summaries.
7. Do not mix `doc::*` with `text::score`. Document extract needs
   `:into(Contract)` plus a matching `resource`.
8. Do not mix executable and stub-only operations in one runnable graph.
9. Default ports: web `18088`, terminal `18023`, API `8080`. Override with
   `SILC_HTTP_PORT` / `SILC_TERMINAL_PORT` / service `:port` as needed.
10. Tensor pipelines require MiniLM, CPU, and exactly 384 normalized `num32`
   values in `vector_embedding`.

## Rules for agents

1. Edit only `.silc` source (`.raku` / `.sil` are not accepted).
2. Prefer author-defined `component` + `app` declarations over inventing profiles.
3. Do not write `method serve()`, `ui::web`, `ui::terminal`, `sink`, `ipc::*`,
   `store::*`, or `resource::*` pipelines — the compiler owns those mechanics.
4. Use Contracts + `resource Name for Contract` capabilities for persistence.
5. Do not create a `stdlib/` directory or escape into React/OpenTUI/CSS.
6. Do not invent new compiler portal kinds to make an app run.
7. Stay inside the UI catalog and runnable operation set above.
8. Validate with `silc build`; report errors instead of patching `.runtime/`.
<!-- END SILC_AGENTS_TEMPLATE -->

## App-specific notes (dataExtractorApp)

- Routes: `/` upload form with `ui::file_input`; `/documents` ledger table.
- Resource: `Documents for Document` (title, headings, body, tables, filename, mime, format, char_count).
- Service: `$upload ==> doc::extract(:into(Document))` — Bun multipart `/upload` + Python extract; originals discarded.
- Do not call `Documents.create` from submit — `submit()` posts multipart and the compiler stores the row.
- Do not mix `doc::*` with `text::score`; never invent Pandoc/OCR paths in `.silc`.
