//! Draft-first authoring path for `silc assist` (ADR-008).
//!
//! Auto-retrieves relevant corpus examples + a rules digest, asks the model
//! for a complete program via the chat template, then compile-and-repair.
//! Falls back to the closed-tool RLM loop only when `--explore` is set.

use sil_training::{check_source, extract_program};

use crate::complete::{ChatRequest, Completer};
use crate::corpus::Corpus;
use crate::progress::{
    draft_preview, truncate_one_line, ActionKind, ProgressEvent, ProgressReporter,
};
use crate::prompt::AUTHOR_SYSTEM_PROMPT;
use crate::session::{AssistError, AssistResult};
use crate::tools::{
    normalize_typography, Budgets, ToolState, MIN_DRAFT_CHARS, UNCHANGED_SEED_ERROR,
};

/// Soft char budget for the injected context bundle (examples + rules + target).
const CONTEXT_CHAR_BUDGET: usize = 24_000;
const EXAMPLE_SLICE: usize = 3_500;
const GAME_EXAMPLE_SLICE: usize = 8_000;
const RULES_DIGEST: usize = 2_500;

/// Stop sequences for draft-first chat — prevent the model from inventing
/// extra components after the program is done.
pub const AUTHOR_STOP: &[&str] = &["\n# END", "\n#!/usr/bin/env silc\n"];

/// Near-greedy sampling for the first draft; raised only to break repeat loops.
const BASE_TEMPERATURE: f32 = 0.2;
const TEMPERATURE_STEP: f32 = 0.3;
const MAX_TEMPERATURE: f32 = 0.9;
/// Floor for the per-attempt generation budget, in tokens.
const MIN_DRAFT_TOKENS: usize = 1_200;

#[derive(Debug, Clone)]
pub struct AuthorContext {
    pub rules: String,
    pub examples: Vec<(String, String)>,
    pub target: Option<String>,
    /// True when `target` is the `silc init` starter rather than a real file.
    pub target_is_starter: bool,
    /// Closed `game::*` catalog digest when the task/seed is game-shaped.
    pub game_catalog: Option<String>,
}

/// Score corpus docs by keyword overlap with the task; prefer examples/fixtures.
pub fn select_context(task: &str, corpus: &Corpus, seed: Option<&str>) -> AuthorContext {
    let tokens = task_tokens(task);
    let mut scored: Vec<(i32, String, String)> = Vec::new();
    let game_shaped = is_game_shaped(task, seed);

    for (id, body) in corpus.list().into_iter().filter_map(|(id, _)| {
        let body = corpus.get(&id)?.to_string();
        Some((id, body))
    }) {
        if id == "target" || id == "agents" || id == "project/agents" {
            continue;
        }
        let is_example = id.starts_with("example/") && id.ends_with(".silc");
        let is_fixture = id.starts_with("fixture/") && id.ends_with(".silc");
        if !is_example && !is_fixture {
            continue;
        }
        let mut score = overlap_score(&tokens, &id, &body);
        if is_fixture {
            score += 2; // small form fixtures are often the best starting shape
        }
        if is_example {
            score += 1;
        }
        if game_shaped && body.contains("game::scene") {
            score += 20;
        } else if game_shaped && !body.contains("game::scene") {
            score -= 10;
        }
        scored.push((score, id, body));
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    let rules_src = corpus
        .get("project/agents")
        .or_else(|| corpus.get("agents"))
        .unwrap_or("");
    let rules = condense_rules(rules_src, RULES_DIGEST);

    let platformer_shaped = is_platformer_shaped(task, seed);
    let game_catalog = if platformer_shaped {
        Some(sil_core::format_game_catalog_platformer_md())
    } else if game_shaped {
        Some(sil_core::format_game_catalog_md())
    } else {
        None
    };
    let mut remaining = CONTEXT_CHAR_BUDGET.saturating_sub(rules.chars().count());
    if let Some(catalog) = &game_catalog {
        remaining = remaining.saturating_sub(catalog.chars().count().min(4_000));
    }
    if let Some(t) = seed {
        remaining = remaining.saturating_sub(t.chars().count().min(4_000));
    }

    let example_slice = if game_shaped {
        GAME_EXAMPLE_SLICE
    } else {
        EXAMPLE_SLICE
    };
    let mut examples = Vec::new();
    for (_, id, body) in scored.into_iter().take(4) {
        if examples.len() >= 2 || remaining < 400 {
            break;
        }
        if game_shaped && !body.contains("game::scene") {
            continue;
        }
        let slice = truncate_chars(&body, example_slice.min(remaining));
        remaining = remaining.saturating_sub(slice.chars().count());
        examples.push((id, slice));
    }

    // Always include a small form fixture if we somehow got nothing.
    if examples.is_empty() {
        if game_shaped {
            if let Some(body) = corpus.get("example/arenaGameApp/main.silc") {
                examples.push((
                    "example/arenaGameApp/main.silc".into(),
                    truncate_chars(body, GAME_EXAMPLE_SLICE),
                ));
            }
        } else if let Some(body) = corpus.get("fixture/scored_form.silc") {
            examples.push((
                "fixture/scored_form.silc".into(),
                truncate_chars(body, EXAMPLE_SLICE),
            ));
        }
    }

    // From scratch, the model tends to over-build (inventing resources, queries
    // and processors it gets wrong). Adapting the `silc init` starter — the same
    // shape the modify path succeeds with — keeps the first draft compilable.
    // Game tasks must not start from the UI form starter.
    let target = match seed {
        Some(s) => Some(s.to_string()),
        None if game_shaped => corpus
            .get("example/arenaGameApp/main.silc")
            .map(str::to_string)
            .or_else(|| corpus.get("starter").map(str::to_string)),
        None => corpus.get("starter").map(str::to_string),
    };

    AuthorContext {
        rules,
        examples,
        target,
        target_is_starter: seed.is_none(),
        game_catalog,
    }
}

fn is_game_shaped(task: &str, seed: Option<&str>) -> bool {
    let lower = task.to_ascii_lowercase();
    seed.is_some_and(|s| {
        s.contains("game::scene")
            || s.contains("\ngame ")
            || s.contains("game::prefab")
            || s.contains("game::mode")
            || s.contains("game::weapon")
            || s.contains("game::zone")
            || s.contains("game::controller")
            || s.contains("game::encounter")
            || s.contains("game::npc")
    }) || lower.contains("game::")
        || lower.contains("webgpu")
        || lower.contains("game scene")
        || lower.contains("fps")
        || lower.contains("first person")
        || lower.contains("first_person")
        || lower.contains("first-person")
        || (lower.contains("game")
            && (lower.contains("arena")
                || lower.contains("prefab")
                || lower.contains("pawn")
                || lower.contains("weapon")
                || lower.contains("zone")
                || lower.contains("encounter")
                || lower.contains("npc")
                || lower.contains("shooter")
                || lower.contains("babylon")
                || lower.contains("real-time")
                || lower.contains("realtime")))
}

/// True when the task/seed is platformer-shaped (side-scroller, 2D, Mario-like).
fn is_platformer_shaped(task: &str, seed: Option<&str>) -> bool {
    let lower = task.to_ascii_lowercase();
    seed.is_some_and(|s| {
        s.contains(":style(platformer)")
            || s.contains(":mode(side_scroll)")
            || s.contains(":scheme(arrows_jump)")
    }) || lower.contains("platformer")
        || lower.contains("side-scroll")
        || lower.contains("side scroll")
        || lower.contains("2d game")
        || lower.contains("2d platformer")
        || lower.contains("mario")
        || lower.contains("metroidvania")
        || lower.contains("sonic")
        || lower.contains("celeste")
        || lower.contains("jump and run")
}

/// Search the entire corpus for snippets that explain a compiler diagnostic.
pub fn retrieve_for_error(corpus: &Corpus, error: &str) -> Vec<(String, String)> {
    let keywords = error_keywords(error);
    if keywords.is_empty() {
        return Vec::new();
    }

    // Prefer patterns that match hard validation rules.
    let patterns: Vec<String> = {
        let mut pats = Vec::new();
        let lower = error.to_lowercase();
        if lower.contains("seed") || lower.contains(":id") || lower.contains("idempo") {
            pats.push(r#":id\("#.into());
            pats.push("stable `:id".into());
            pats.push("seed ".into());
        }
        if lower.contains("resource") {
            pats.push("resource ".into());
        }
        if lower.contains("contract") {
            pats.push("contract ".into());
        }
        // Always include the most distinctive tokens from the error.
        for kw in keywords.iter().take(4) {
            if kw.len() >= 3 {
                pats.push(regex::escape(kw));
            }
        }
        pats
    };

    let mut hit_lines: Vec<(String, usize, String)> = Vec::new(); // id, line_no, line
    for pattern in &patterns {
        if let Ok(hits) = corpus.grep(pattern, None) {
            for hit in hits {
                if hit == "(no matches)" || hit.starts_with('…') {
                    continue;
                }
                // format: id:line:text
                let mut parts = hit.splitn(3, ':');
                let id = parts.next().unwrap_or("").to_string();
                let line_no: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                let text = parts.next().unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }
                hit_lines.push((id, line_no, text));
                if hit_lines.len() >= 24 {
                    break;
                }
            }
        }
        if hit_lines.len() >= 24 {
            break;
        }
    }

    // Prefer agents / project/agents hits first.
    hit_lines.sort_by(|a, b| {
        let rank = |id: &str| {
            if id == "project/agents" || id == "agents" {
                0
            } else if id.contains("AGENTS") {
                1
            } else {
                2
            }
        };
        rank(&a.0).cmp(&rank(&b.0)).then_with(|| a.0.cmp(&b.0))
    });

    let mut seen_ids = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (id, line_no, _) in &hit_lines {
        if !seen_ids.insert(id.clone()) {
            continue;
        }
        let window = line_window(corpus, id, *line_no, 15).unwrap_or_else(|| {
            hit_lines
                .iter()
                .filter(|(i, _, _)| i == id)
                .take(3)
                .map(|(_, ln, t)| format!("{ln}: {t}"))
                .collect::<Vec<_>>()
                .join("\n")
        });
        if !window.trim().is_empty() {
            out.push((id.clone(), window));
        }
        if out.len() >= 6 {
            break;
        }
    }
    out
}

/// Turn a compiler diagnostic into an explicit, actionable instruction.
///
/// Structural errors (name collisions, missing seed ids) are not taught well by
/// grep hits, so assist states the rule and the concrete edit to make.
pub fn repair_guidance(error: &str) -> Option<String> {
    let lower = error.to_lowercase();

    if lower.contains("expected game name")
        || (lower.contains("expected `game`") && lower.contains("parse"))
    {
        return Some(
            "GAME SCENE ENCLOSURE RULE: the entire program is ONE `game Name { game::scene( ... ) }`. \
             Never close `game::scene` before `game::spawn`, `game::mode`, `game::controller`, \
             `game::camera`, `game::weapon`, `game::hud`, `game::environment`, `game::shadow`, \
             `game::post_process`, `game::overlay`, `game::zone`, or `game::encounter`. \
             Do not emit trailing `game::*` siblings after the final `}` of the game block. \
             FPS guns are scene-level `game::weapon(:name(...), :slot(1), :fire_mode(hitscan), ...)` \
             nodes — not `game::ability` keys. Keep projectile cues as children of `game::weapon`."
                .into(),
        );
    }
    if lower.contains("unknown game node")
        || (lower.contains("unknown prop") && lower.contains("game::"))
    {
        return Some(format!(
            "GAME CATALOG RULE: use only closed `game::*` nodes/props.\n{}\nFix: replace the unknown node/prop with a catalog entry above.",
            sil_core::format_game_catalog_md()
        ));
    }
    if lower.contains("cannot contain game::") || lower.contains("does not accept children") {
        return Some(
            "GAME CHILD RULE: each `game::*` node only accepts the children listed in the catalog. \
             Move disallowed children under `game::scene`, `game::entity`, `game::prefab`, \
             `game::zone`, `game::weapon`, `game::mode`, or `game::encounter` as appropriate. \
             NPC AI stacks (`npc`, `perception`, `behavior`, `mind`, `nav_agent`) belong under \
             `game::entity` or `game::prefab`; weapon cues (`projectile`, `particle_emitter`, \
             `audio`) belong under `game::weapon`."
                .into(),
        );
    }
    if lower.contains("duplicate ability key") {
        return Some(
            "ABILITY KEY RULE: every `game::ability` needs a unique `:key(\"…\")` string. \
             Fix: change the colliding key (typically \"1\"–\"5\")."
                .into(),
        );
    }
    if lower.contains("must be one of")
        && (lower.contains("fire_mode") || lower.contains("game::weapon"))
    {
        return Some(
            "WEAPON SLOT RULE: `game::weapon :slot` is a number (`:slot(1)` … `:slot(4)`), never a bare ident. \
Weapon `:name` and `:ref` remain quoted strings.\n\
WEAPON FIRE MODE RULE: `game::weapon :fire_mode` must be one of \
             `hitscan`, `pellet`, `projectile`, or `beam`. Bind tuneables with `:ref(\"DataName\")` \
             on a matching `game::data` asset when possible."
                .into(),
        );
    }
    if lower.contains("must be one of") && lower.contains("movement") && lower.contains(":style") {
        return Some(
            "MOVEMENT STYLE RULE: player locomotion uses `game::movement :style(first_person)` \
             (or `walk` / `sprint` / `jump`) on the possessed pawn prefab. Pair with \
             `game::camera :mode(first_person)` and `game::controller :scheme(wasd_mouse)`."
                .into(),
        );
    }
    if lower.contains("cannot mix") && lower.contains("game") {
        return Some(
            "GAME SUBJECT RULE: a game program is only `game Name { game::scene(...) }`. \
             Remove every `app`, `component`, `contract`, `resource`, `service`, and `processor`."
                .into(),
        );
    }
    if lower.contains("root must be") && lower.contains("game::scene") {
        return Some(
            "GAME ROOT RULE: the declaration must be `game Name { game::scene(:title(\"…\"), …) }`."
                .into(),
        );
    }

    if lower.contains("in contract") {
        return Some(
            "CONTRACT RULE: a contract holds ONLY field declarations — `has Type $.field;` lines and nothing else. No methods, no `has state`, no default values, no ui:: calls.\nFix: move every method and all state into a `component` (or a `processor` for pipelines) and leave the contract as plain fields."
                .into(),
        );
    }

    if let Some(name) = duplicate_name(error) {
        let suggestion = rename_candidates(&name)[0].clone();
        return Some(format!(
            "NAME COLLISION: `{name}` is declared twice. In Silc, contracts, components, resources, apps, processors and services all share ONE namespace — every declaration needs a unique name.\nFix: if you declared the SAME block twice, delete the duplicate so only one remains. If two DIFFERENT declarations share the name, keep `component {name}` and rename the other to `{suggestion}`, updating every reference. Never emit two declarations with the same name."
        ));
    }

    if lower.contains("may only assign reactive state") {
        return Some(
            "STATE RULE: only `has state` fields are assignable. A `query $.rows = Rows.list();` binding is declared at COMPONENT level (not inside a method) and is read-only — never assign to it.\nFix: remove the assignment from the method body; if you need the rows, declare `query $.rows = Rows.list();` beside the `has state` lines."
                .into(),
        );
    }

    if lower.contains("seed") && (lower.contains(":id") || lower.contains("idempo")) {
        return Some(
            "SEED RULE: every `seed Contract.new(...)` row must include a stable `:id(\"…\")` string as its first argument.\nFix: either add `:id(\"row-001\")`, `:id(\"row-002\")`, … to every seed, or delete the `seed` lines entirely (a form does not need seeds)."
                .into(),
        );
    }

    if lower.contains("pipeline") {
        return Some(
            "PIPELINE RULE: `==>` feeds a value into an OP (`$note.text ==> text::score()`) and belongs only in a `processor` method. It is not assignment.\nFix: inside a component method assign with `$.field = \"\";` and persist with a resource mutation such as `Guests.create(Guest.new(:name($.name)));`."
                .into(),
        );
    }

    if lower.contains("expected expression") {
        return Some(
            "EXPRESSION RULE: a mutation runs on the RESOURCE and takes a contract record — `Guests.create(Guest.new(:name($.name), :room($.room)));`. Never call a mutation on the contract, never pass bare named args to it, and always reference state as `$.field` (never `$field`)."
                .into(),
        );
    }

    if lower.contains("unknown prop") || lower.contains("unknown event") {
        return Some(
            "CATALOG RULE: UI nodes only accept catalog props/events. Remove the unknown prop/event and use documented ones (`:label`, `:field`, `:value`, `:variant`, `:tone`, `:size`, `:on(click(handler))`, `:on(submit(handler))`)."
                .into(),
        );
    }

    if lower.contains("unsupported construct")
        || (lower.contains("expected `subset`") && lower.contains("ui::"))
    {
        return Some(
            "TOP-LEVEL RULE: `ui::*` nodes belong ONLY inside a component `method render()` tree. Never leave `ui::alert(...)` / `ui::stack(...)` as a free-standing top-level declaration beside `app` / `service`. Move the alert into the component stack with `when $.status { ui::alert(...) }`."
                .into(),
        );
    }

    if lower.contains("closed")
        && (lower.contains("variant") || lower.contains("tone") || lower.contains("size"))
    {
        return Some(
            "CLOSED ENUM: `:variant` accepts primary|secondary|destructive|ghost, `:tone` accepts default|muted|info|success|warning|danger, `:size` accepts sm|md|lg. Use bare tokens, not strings."
                .into(),
        );
    }

    if lower.contains("route") && lower.contains("app") {
        return Some(
            "APP RULE: a UI program needs exactly one `app` block with at least one `route \"/\" => SomeComponent;` pointing at a declared component."
                .into(),
        );
    }

    if lower.contains("text::score") && lower.contains("llm::complete") {
        return Some(
            "OP RULE: `text::score` and `llm::complete` cannot both appear. Keep at most one processor and one of these ops."
                .into(),
        );
    }

    None
}

/// Non-colliding names to rename a declaration to, best first.
///
/// Pluralising an already-plural name yields `Guestss`, so names ending in `s`
/// take a suffix instead.
fn rename_candidates(name: &str) -> Vec<String> {
    if name.ends_with('s') {
        vec![
            format!("{name}Store"),
            format!("{name}Data"),
            format!("{name}Records"),
        ]
    } else {
        vec![
            format!("{name}s"),
            format!("{name}Store"),
            format!("{name}Data"),
        ]
    }
}

/// Extract the duplicated identifier from a `duplicate <kind> name \`X\`` error.
fn duplicate_name(error: &str) -> Option<String> {
    if !error.to_lowercase().contains("duplicate") {
        return None;
    }
    let start = error.find('`')?;
    let rest = &error[start + 1..];
    let end = rest.find('`')?;
    let name = rest[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Deterministic textual repairs for common structural diagnostics.
///
/// Returns `Some((fixed_program, description))`. The caller must re-run
/// `check_source`; a failed autofix is discarded.
pub fn autofix(program: &str, error: &str) -> Option<(String, String)> {
    let lower = error.to_lowercase();

    if let Some(name) = duplicate_name(error) {
        // The same block emitted twice is a repetition, not a naming clash, so
        // dropping the redundant copy is the faithful fix.
        if let Some((fixed, kind)) = drop_repeated_declaration(program, &name) {
            return Some((fixed, format!("removed the repeated `{name}` {kind} block")));
        }
        // A collider that merely restates another declaration is dead weight.
        if let Some((fixed, kind)) = drop_redundant_collider(program, &name) {
            return Some((fixed, format!("removed the redundant `{name}` {kind}")));
        }
        if let Some(fixed) = rename_duplicate_resource(program, &name) {
            return Some((fixed, format!("renamed duplicate `{name}` declaration")));
        }
        if let Some((fixed, kind)) = rename_colliding_declaration(program, &name) {
            return Some((fixed, format!("renamed the colliding `{name}` {kind}")));
        }
    }

    if lower.contains("seed") && (lower.contains(":id") || lower.contains("idempo")) {
        if let Some(fixed) = drop_seed_blocks(program) {
            return Some((fixed, "removed seed rows missing stable :id".into()));
        }
    }

    if let Some((fixed, name)) = hoist_nested_method(program, error) {
        return Some((
            fixed,
            format!("moved nested `{name}` out of its enclosing method"),
        ));
    }

    if lower.contains("expected game name") || lower.contains("expected `game`") {
        if let Some(fixed) = reenclose_orphaned_game_nodes(program) {
            return Some((
                fixed,
                "moved orphaned game::* siblings back inside game::scene".into(),
            ));
        }
    }

    if lower.contains("cannot contain game::") {
        if let Some((fixed, kind)) = hoist_scene_only_nodes(program, error) {
            return Some((
                fixed,
                format!("moved game::{kind} to scene scope"),
            ));
        }
    }

    None
}

/// Apply the first matching closed FPS injector for `task`.
pub fn inject_fps_task(task: &str, seed: &str) -> Option<(String, String)> {
    if let Some(fixed) = inject_named_fps_weapons(task, seed) {
        return Some((fixed, "injected closed FPS weapon loadout into seed scene".into()));
    }
    if crate::fps_inject::wants_megastructure_rebuild(task)
        && seed.contains(":name(\"SecurityLobby\")")
    {
        let stripped = strip_game_nodes(seed, "zone");
        let stripped = strip_game_nodes(&stripped, "encounter");
        let mut nodes = crate::fps_inject::megastructure_zone_nodes();
        nodes.push(crate::fps_inject::hostile_encounter_wave());
        let fixed = insert_scene_children(&stripped, &nodes)?;
        return Some((
            fixed,
            "rebuilt megastructure zones/doorways and hostile wave in seed scene".into(),
        ));
    }
    if crate::fps_inject::wants_megastructure(task) && !seed.contains(":name(\"SecurityLobby\")") {
        let zones = crate::fps_inject::megastructure_zone_nodes();
        let fixed = insert_scene_children(seed, &zones)?;
        let fixed = if crate::fps_inject::wants_strip_neon(task) {
            strip_neon_entities(&fixed)
        } else {
            fixed
        };
        return Some((fixed, "injected megastructure zones/kit furniture into seed scene".into()));
    }
    if crate::fps_inject::wants_hostiles(task) && !seed.contains(":name(\"Suppressor\")") {
        let nodes = crate::fps_inject::hostile_encounter_nodes();
        let fixed = insert_scene_children(seed, &nodes)?;
        return Some((fixed, "injected hostile archetypes/encounters/minds into seed scene".into()));
    }
    if crate::fps_inject::wants_strip_neon(task)
        && (seed.contains("NeonSphere") || seed.contains("WarmPointLight"))
    {
        let fixed = strip_neon_entities(seed);
        if fixed != seed {
            return Some((fixed, "removed neon placeholder entities from seed scene".into()));
        }
    }
    None
}

/// Remove every `game::<kind>(...)` block, matching parens so nested children
/// go with their parent.
fn strip_game_nodes(program: &str, kind: &str) -> String {
    let needle = format!("game::{kind}(");
    let mut out = program.to_string();
    loop {
        let Some(start) = out.find(&needle) else { break };
        let mut depth = 0i32;
        let mut end = None;
        for (i, ch) in out[start..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(mut end) = end else { break };
        while end < out.len() && out.as_bytes()[end].is_ascii_whitespace() {
            end += 1;
        }
        if end < out.len() && out.as_bytes()[end] == b',' {
            end += 1;
        }
        out.replace_range(start..end, "");
    }
    while out.contains(",,") {
        out = out.replace(",,", ",");
    }
    out
}

fn strip_neon_entities(program: &str) -> String {
    let mut out = program.to_string();
    for name_prefix in ["NeonSphere", "WarmPointLight"] {
        loop {
            let needle = format!(":name(\"{name_prefix}");
            let Some(name_at) = out.find(&needle) else { break };
            // Walk back to the owning `game::entity(` start.
            let Some(ent_at) = out[..name_at].rfind("game::entity(") else { break };
            let mut depth = 0i32;
            let mut end = None;
            for (i, ch) in out[ent_at..].char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(ent_at + i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(mut end) = end else { break };
            // Drop trailing comma / whitespace.
            while end < out.len() && out.as_bytes()[end].is_ascii_whitespace() {
                end += 1;
            }
            if end < out.len() && out.as_bytes()[end] == b',' {
                end += 1;
            }
            out.replace_range(ent_at..end, "");
        }
    }
    // Tidy double commas / blank gaps.
    while out.contains(",,") {
        out = out.replace(",,", ",");
    }
    out
}

/// When an additive FPS task names the closed four-weapon loadout, inject the
/// catalog nodes directly into the seed if the model failed to emit them.
pub fn inject_named_fps_weapons(task: &str, seed: &str) -> Option<String> {
    let lower = task.to_lowercase();
    let wants = lower.contains("vanguard")
        && lower.contains("breach")
        && (lower.contains("arc") || lower.contains("carbine"))
        && (lower.contains("rail") || lower.contains("longshot"));
    if !wants || !seed.contains("game::scene(") {
        return None;
    }
    if seed.contains("game::weapon(:name(\"VanguardAR\")")
        || seed.contains(":name(\"VanguardAR\")")
    {
        return None;
    }
    let nodes = [
        r#"game::data(:name("VanguardData"), :damage(16), :fire_rate(9), :magazine(30), :reload(1.7), :spread(0.018))"#,
        r#"game::data(:name("BreachData"), :damage(12), :fire_rate(1.2), :magazine(6), :reload(2.4), :spread(0.08), :pellet_count(10))"#,
        r#"game::data(:name("ArcData"), :damage(28), :fire_rate(3.5), :magazine(18), :reload(2.0), :spread(0.01))"#,
        r#"game::data(:name("RailData"), :damage(95), :fire_rate(0.7), :magazine(4), :reload(2.8), :spread(0.002))"#,
        r#"game::weapon(:name("VanguardAR"), :slot(1), :fire_mode(hitscan), :ref("VanguardData"))"#,
        r#"game::weapon(:name("Breach12"), :slot(2), :fire_mode(pellet), :ref("BreachData"))"#,
        r##"game::weapon(
            :name("ArcCarbine"),
            :slot(3),
            :fire_mode(projectile),
            :ref("ArcData"),
            game::projectile(:kind(plasma), :speed(28), :lifetime(2.5), :splash_radius(2.2), :color("#66ffcc"))
        )"##,
        r##"game::weapon(
            :name("LongshotRailgun"),
            :slot(4),
            :fire_mode(beam),
            :ref("RailData"),
            game::projectile(:kind(rail), :speed(200), :lifetime(0.2), :color("#88ccff"))
        )"##,
    ];
    let mut additions = Vec::new();
    for node in nodes {
        let key = node_identity(node, if node.contains("game::weapon") {
            "weapon"
        } else {
            "data"
        });
        if seed.contains(&key) || additions.iter().any(|a: &String| a.contains(&key)) {
            continue;
        }
        additions.push(node.to_string());
    }
    if additions.is_empty() {
        return None;
    }
    let mut fixed = insert_scene_children(seed, &additions)?;
    if !fixed.contains(":title(\"MEGASTRUCTURE\")") && lower.contains("megastructure") {
        fixed = fixed.replacen(":title(\"ARENA\")", ":title(\"MEGASTRUCTURE\")", 1);
    }
    Some(fixed)
}

/// Merge additive scene-level nodes from a broken draft into a known-good seed.
///
/// Used when the model truncates while inventing duplicate entities: keep the
/// seed tree and graft any new `game::data` / `game::weapon` / `game::zone` /
/// `game::hud` / `game::environment` / `game::shadow` / `game::prefab` /
/// `game::encounter` declarations that are absent from the seed.
pub fn merge_additive_game_draft(seed: &str, draft: &str) -> Option<String> {
    if !seed.contains("game::scene(") || !draft.contains("game::scene(") {
        return None;
    }
    let kinds = [
        "data",
        "weapon",
        "zone",
        "hud",
        "environment",
        "shadow",
        "prefab",
        "encounter",
        "objective",
        "asset",
        "material",
    ];
    let mut additions = Vec::new();
    for kind in kinds {
        for node in extract_top_level_game_nodes(draft, kind) {
            let key = node_identity(&node, kind);
            if key.is_empty() {
                continue;
            }
            if seed.contains(&key) {
                continue;
            }
            if additions.iter().any(|a: &String| a.contains(&key)) {
                continue;
            }
            additions.push(node);
        }
    }
    if additions.is_empty() {
        return None;
    }
    insert_scene_children(seed, &additions)
}

fn extract_top_level_game_nodes(program: &str, kind: &str) -> Vec<String> {
    let needle = format!("game::{kind}(");
    let mut out = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = program[search_from..].find(&needle) {
        let start = search_from + rel;
        let mut depth = 0i32;
        let mut end = None;
        for (i, ch) in program[start..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else { break };
        out.push(program[start..end].trim().to_string());
        search_from = end;
    }
    out
}

fn node_identity(node: &str, kind: &str) -> String {
    // Prefer :name("…") / :id("…") identity; fall back to full node text.
    for prop in ["name", "id"] {
        let p = format!(":{prop}(\"");
        if let Some(at) = node.find(&p) {
            let rest = &node[at + p.len()..];
            if let Some(end) = rest.find('"') {
                return format!("game::{kind}(:{prop}(\"{}\")", &rest[..end]);
            }
        }
    }
    node.chars().take(120).collect()
}

fn insert_scene_children(seed: &str, children: &[String]) -> Option<String> {
    let scene_at = seed.find("game::scene(")?;
    // Insert before the final scene closer: last line that is just `    )` before `}`.
    let mut lines: Vec<String> = seed.lines().map(str::to_string).collect();
    let mut close_idx = None;
    for (i, line) in lines.iter().enumerate().rev() {
        if line.trim() == ")" {
            close_idx = Some(i);
            break;
        }
    }
    let close_idx = close_idx?;
    if let Some(prev) = lines[..close_idx]
        .iter_mut()
        .rev()
        .find(|l| !l.trim().is_empty())
    {
        let t = prev.trim_end().to_string();
        if !t.ends_with(',') && !t.ends_with('(') {
            *prev = format!("{t},");
        }
    }
    let mut block = Vec::new();
    for (i, child) in children.iter().enumerate() {
        let comma = if i + 1 < children.len() { "," } else { "" };
        let child_lines: Vec<&str> = child.lines().collect();
        for (li, line) in child_lines.iter().enumerate() {
            let is_last_line = li + 1 == child_lines.len();
            if is_last_line {
                block.push(format!("        {line}{comma}"));
            } else {
                block.push(format!("        {line}"));
            }
        }
    }
    let _ = scene_at;
    lines.splice(close_idx..close_idx, block);
    Some(lines.join("\n"))
}

/// Hoist scene-only nodes (`material`, `asset`, `weapon`, …) that the model
/// nested under `game::entity` / `game::prefab` / `game::zone`.
fn hoist_scene_only_nodes(program: &str, error: &str) -> Option<(String, String)> {
    let kind = [
        "material",
        "asset",
        "weapon",
        "data",
        "hud",
        "environment",
        "shadow",
        "encounter",
        "objective",
        "post_process",
        "overlay",
        "mode",
        "controller",
        "camera",
    ]
    .into_iter()
    .find(|k| error.contains(&format!("cannot contain game::{k}")))?;

    let needle = format!("game::{kind}(");
    let start = program.find(&needle)?;
    // Find matching close paren for this node.
    let mut depth = 0i32;
    let mut end = None;
    for (i, ch) in program[start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let mut node = program[start..end].to_string();
    // Consume a trailing comma if present.
    let mut after = end;
    let bytes = program.as_bytes();
    while after < bytes.len() && (bytes[after] as char).is_whitespace() {
        after += 1;
    }
    if after < bytes.len() && bytes[after] == b',' {
        after += 1;
        node.push(',');
    }
    let mut without = String::new();
    without.push_str(&program[..start]);
    without.push_str(&program[after..]);

    // Insert just after `game::scene(` opener.
    let scene_at = without.find("game::scene(")?;
    let insert_at = scene_at + "game::scene(".len();
    let mut fixed = String::new();
    fixed.push_str(&without[..insert_at]);
    fixed.push('\n');
    fixed.push_str("        ");
    fixed.push_str(node.trim().trim_end_matches(','));
    fixed.push(',');
    fixed.push_str(&without[insert_at..]);
    Some((fixed, kind.to_string()))
}

/// When the model closes `game::scene` / `game Name` too early, trailing
/// `game::spawn` / `game::weapon` / etc. siblings sit at file scope and parse as
/// a second top-level `game` declaration. Re-open the scene and append them.
fn reenclose_orphaned_game_nodes(program: &str) -> Option<String> {
    let lines: Vec<&str> = program.lines().collect();
    let mut scene_end = None;
    let mut depth = 0i32;
    let mut in_game = false;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("game ") && trimmed.contains('{') {
            in_game = true;
        }
        if !in_game {
            continue;
        }
        for ch in line.chars() {
            match ch {
                '(' | '{' => depth += 1,
                ')' | '}' => depth -= 1,
                _ => {}
            }
        }
        // First scene closer: depth returns to 1 (`game Name {` still open).
        if scene_end.is_none() && trimmed == ")" && depth == 1 {
            scene_end = Some(i);
        }
        if trimmed == "}" && depth == 0 {
            break;
        }
    }
    let scene_end = scene_end?;
    let mut orphans: Vec<String> = Vec::new();
    for line in lines.iter().skip(scene_end + 1) {
        let t = line.trim();
        if t.is_empty() || t == "}" {
            continue;
        }
        // Preserve multi-line game::entity / weapon blocks; only normalize
        // indentation for top-level orphaned nodes.
        if t.starts_with("game::") {
            orphans.push(format!("        {t}"));
        } else {
            orphans.push(format!("            {t}"));
        }
    }
    if orphans.is_empty() || !orphans.iter().any(|l| l.trim_start().starts_with("game::")) {
        return None;
    }

    let mut out: Vec<String> = lines[..scene_end]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    if let Some(last) = out.iter_mut().rev().find(|l| !l.trim().is_empty()) {
        let t = last.trim_end().to_string();
        if !t.ends_with(',') && !t.ends_with('(') {
            *last = format!("{t},");
        }
    }
    out.extend(orphans);
    if let Some(last) = out.last_mut() {
        let t = last.trim_end().to_string();
        if t.ends_with(',') {
            *last = t.trim_end_matches(',').to_string();
        }
    }
    out.push("    )".into());
    out.push("}".into());
    let fixed = out.join("\n");
    if fixed.trim() == program.trim() {
        return None;
    }
    Some(fixed)
}

/// Move a `method` declared inside another method body out to become its sibling.
///
/// The model writes balanced braces but forgets to close `render()` before the
/// next method, so this is an ordering mistake rather than a missing brace.
fn hoist_nested_method(program: &str, error: &str) -> Option<(String, String)> {
    let (line_no, _) = error_location(error)?;
    let mut lines: Vec<String> = program.lines().map(str::to_string).collect();
    let start = line_no.checked_sub(1)?;
    if start >= lines.len() {
        return None;
    }
    let trimmed = lines[start].trim_start();
    if !trimmed.starts_with("method ") {
        return None;
    }
    let name = trimmed
        .trim_start_matches("method ")
        .split('(')
        .next()?
        .trim()
        .to_string();

    // Span the nested method by brace depth.
    let mut depth = 0i32;
    let mut end = None;
    for (offset, line) in lines[start..].iter().enumerate() {
        for ch in line.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if depth == 0 && offset > 0 {
            end = Some(start + offset);
            break;
        }
    }
    let end = end?;

    let block: Vec<String> = lines.drain(start..=end).collect();

    // The next closing brace now ends the enclosing method; the block follows it.
    let close_at = lines
        .iter()
        .skip(start)
        .position(|l| l.trim() == "}")
        .map(|p| start + p)?;

    let target_indent = lines[close_at].len() - lines[close_at].trim_start().len();
    let block_indent = block[0].len() - block[0].trim_start().len();
    let reindented: Vec<String> = block
        .into_iter()
        .map(|l| {
            if l.trim().is_empty() {
                return l;
            }
            let indent = l.len() - l.trim_start().len();
            let shifted = indent + target_indent - block_indent.min(indent);
            format!("{}{}", " ".repeat(shifted.min(64)), l.trim_start())
        })
        .collect();

    let mut out = lines;
    let mut tail = out.split_off(close_at + 1);
    out.push(String::new());
    out.extend(reindented);
    out.append(&mut tail);
    Some((format!("{}\n", out.join("\n")), name))
}

/// Structural patterns a task requires but the model rarely infers on its own.
fn task_pattern(task: &str, target: Option<&str>) -> Option<String> {
    let lower = task.to_lowercase();
    let target = target.unwrap_or("");
    let mut parts: Vec<&'static str> = Vec::new();

    let wants_storage = [
        "store",
        "save",
        "persist",
        "record",
        "ledger",
        "database",
        "keep track",
        "history",
        "sqlite",
        "submit",
    ]
    .iter()
    .any(|k| lower.contains(k));
    if wants_storage && !target.contains("resource ") {
        parts.push(
            r#"Storing form data REQUIRES a `resource` — a `processor` only computes and discards, so a program without a resource saves nothing.
Add EXACTLY ONE resource block for the contract, at the top level, next to the contract:

resource Guests for Guest {
    query list;
    mutation create;
}

Then call its mutation from the submit handler inside the component:

    method on_submit() {
        Guests.create(Guest.new(:name($.name), :phone($.phone)));
        $.name = "";
        $.phone = "";
    }

Rules: declare the resource ONCE (never repeat the block); its name must differ from every contract/component name; pass state as `$.field` (never `$field`); clear state with `$.field = "";` (never `==>`); do not assign to anything else inside the handler.
Form submit buttons use `:variant(primary)` as a bare token and the `:submit` flag — never `:variant("primary")` as a string, and never `:on(click(on_submit))` on a submit button."#,
        );
    }

    let wants_ledger = [
        "ledger",
        "list page",
        "view page",
        "another page",
        "second page",
        "see the",
        "view the",
        "show the",
        "table of",
        "/ledger",
    ]
    .iter()
    .any(|k| lower.contains(k));
    if wants_ledger {
        parts.push(
            r#"A list/ledger page is a SEPARATE component with a route — never invent a second contract or a processor for it.
Use a resource query binding at COMPONENT level and a ui::table. Shape:

component GuestLedgerPage {
    query $.guests = Guests.list();

    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Guest Ledger"))),
            :side_panel(ui::side_panel(
                ui::nav_item(:label("Sign Up"), :to("/")),
                ui::nav_item(:label("Ledger"), :to("/ledger"), :active)
            )),
            ui::stack(
                ui::heading(:text("Guest ledger"), :level(2)),
                ui::table(
                    :rows($.guests),
                    :columns(["name", "phone", "room", "comment"]),
                    :empty_text("No guests yet."),
                    :sortable,
                    :searchable
                )
            )
        )
    }
}

app HotelApp {
    route "/" => GuestForm;
    route "/ledger" => GuestLedgerPage;
}

Rules: the component name MUST differ from every contract/resource/processor/app name (do not declare `contract GuestLedger` or `processor HotelApp`); put `query $.guests = Guests.list();` beside `has state` lines — never assign `$.guests = …` inside a method; wrap nav items in `ui::side_panel(...)`; keep the existing form page and add nav links on BOTH pages."#,
        );
    }

    let wants_doc_upload = [
        "upload",
        "document",
        "pdf",
        "docx",
        "file_input",
        "doc::extract",
        "extract text",
        "data extractor",
        "dataextractor",
        "data collector",
        "datacollector",
    ]
    .iter()
    .any(|k| lower.contains(k));
    if wants_doc_upload {
        parts.push(
            r#"Document upload REQUIRES `ui::file_input` + `doc::extract(:into(Contract))` + a matching `resource` — never invent a processor that scores/LLM-completes the file, and never write filesystem paths by hand.

Shape (names may match the task domain):

contract Document {
    has Str $.title;
    has Str $.headings;
    has Str $.body;
    has Str $.tables;
    has Str $.filename;
    has Str $.mime;
    has Str $.format;
    has Str $.char_count;
}

resource Documents for Document {
    query list;
    mutation create;
    mutation delete;
}

component UploadPage {
    has state Str $.upload = "";

    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Data Extractor"))),
            :side_panel(ui::side_panel(
                ui::nav_item(:label("Upload"), :to("/"), :active),
                ui::nav_item(:label("Documents"), :to("/documents"))
            )),
            ui::stack(
                ui::heading(:text("Upload a document"), :level(2)),
                ui::form(:on(submit(on_submit)),
                    ui::file_input(
                        :field(upload),
                        :label("Document"),
                        :accept(".pdf,.docx,.odt,.md,.txt,.html")
                    ),
                    ui::button(:label("Extract"), :variant(primary), :submit)
                )
            )
        )
    }
    method on_submit() { submit(); }
}

component DocumentsPage {
    query $.documents = Documents.list();

    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Data Extractor"))),
            :side_panel(ui::side_panel(
                ui::nav_item(:label("Upload"), :to("/")),
                ui::nav_item(:label("Documents"), :to("/documents"), :active)
            )),
            ui::stack(
                ui::heading(:text("Documents"), :level(2)),
                ui::table(
                    :rows($.documents),
                    :columns(["title", "filename", "format", "char_count", "body"]),
                    :empty_text("No documents yet."),
                    :sortable,
                    :searchable
                )
            )
        )
    }
}

app ExtractorApp {
    route "/" => UploadPage;
    route "/documents" => DocumentsPage;
}

service Extractor {
    method run() {
        $upload ==> doc::extract(:into(Document))
    }
}

Rules: keep contract field names exactly as above so extract can fill them; use `:submit` on the upload button (not `:on(click)`); do not call `Documents.create` from the submit handler — `submit()` posts multipart to `/upload` and the compiler stores the row; do not mix `doc::*` with `text::score`; do not invent Pandoc, OCR, or blob CDN ops."#,
        );
    }

    let wants_toast = [
        "toast",
        "snack",
        "snackbar",
        "success alert",
        "show a success",
        "status message",
        "flash message",
        "notify",
        "notification",
    ]
    .iter()
    .any(|k| lower.contains(k));
    if wants_toast {
        parts.push(
            r#"Transient success feedback uses `ui::alert` INSIDE a component render tree — never as a top-level declaration.
Shape:

has state Str $.status = "";

# inside the same component's ui::stack (sibling of the form):
when $.status {
    ui::alert(
        :tone(success),
        :text($.status),
        :dismissible,
        :auto_dismiss_ms(5000),
        :on(dismiss(on_dismiss))
    )
}

method on_submit() {
    submit();
    $.upload = "";
    $.status = "Document extracted.";
}
method on_dismiss() { $.status = ""; }

Rules: `ui::alert` / `when` must live inside `method render()`; clearing a file field is `$.upload = \"\"` after `submit()`; use `:auto_dismiss_ms(5000)` for a 5s fade-away toast; do not invent toast/snackbar components outside the catalog."#,
        );
    }

    let wants_game = is_game_shaped(
        task,
        if target.is_empty() {
            None
        } else {
            Some(target)
        },
    );
    let game_part = if wants_game && !target.contains("game ") {
        Some(format!(
            "WebGPU game programs declare ONE `game Name {{ game::scene(...) }}` tree — closed `game::*` catalog only (ADR-012). \
No `app`, `component`, `resource`, `service`, or `processor`. No hand-written JavaScript/TypeScript in `.silc`. \
The compiler synthesizes Bun (host) + CPython bake + Go SQLite.\n\n\
Author with the three-engine synthesis: Godot nested `entity`/`signal`/`group`/`zone` trees; \
Unity `prefab` + `data` + `asset`/`material` + `spawn` overrides; Unreal `mode`/`pawn`/`controller` \
ownership with FPS weapons and encounters.\n\
Copy structure from the highest-scoring game example in context and tune props for the task.\n\
Required systems for a playable FPS scene: prefab(+mesh/collider/movement/pawn/weapon/ammo), \
`game::data` weapon/locomotion profiles, `game::asset`/`game::material` when GLTF/PBR is needed, \
entity world tree with `game::zone` volumes, spawn, mode, controller, \
`game::camera :mode(first_person)`, `game::hud`, weapons with cue children, optional \
`game::encounter` waves and NPC stacks (`npc`/`perception`/`behavior`/`mind`), post_process, overlay.\n\n\
{}\n\
Rules: use ONLY catalog nodes/props; `:title` is manifest data — never invent title-named compiler branches; \
default web port 18140; runtime TypeScript/Babylon kernel is compiler-owned; do not mix with UI `app` routes.",
            sil_core::format_game_catalog_md()
        ))
    } else {
        None
    };

    if parts.is_empty() && game_part.is_none() {
        None
    } else {
        let mut out = parts.join("\n\n");
        if let Some(game) = game_part {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&game);
        }
        Some(out)
    }
}

/// Size the generation budget to the program being written.
///
/// A degenerate generation otherwise runs to the ceiling — a form-sized task once
/// produced 17k chars in 91s — so scale from the target and keep the ceiling for
/// genuinely large files.
fn draft_token_budget(target: Option<&str>, ceiling: usize) -> usize {
    // A game scene is one large, nested intent tree that must be returned in
    // full on every edit. Form-oriented scaling truncates realistic games and
    // then wastes repair attempts trying to reconstruct a missing tail.
    // Prefer at least 8k tokens for games even when the global ceiling is lower.
    if target.is_some_and(|source| source.contains("game::scene")) {
        return ceiling.max(8192);
    }
    let target_chars = target.map_or(0, str::len);
    let estimate = target_chars / 3 + 256;
    (estimate * 3 / 2).clamp(MIN_DRAFT_TOKENS.min(ceiling), ceiling)
}

/// Insert the required `@version` pragma into an otherwise plausible program.
///
/// The model occasionally omits only this line; adding it beats spending a whole
/// draft attempt on a regeneration.
pub fn ensure_version(program: &str) -> Option<String> {
    if program.contains("@version(") {
        return None;
    }
    let looks_silc = program.contains("component ")
        || program.contains("contract ")
        || program.contains("app ")
        || program.contains("game ");
    if !looks_silc {
        return None;
    }
    let mut lines: Vec<&str> = program.lines().collect();
    let insert_at = usize::from(lines.first().is_some_and(|l| l.starts_with("#!")));
    lines.insert(insert_at, "@version(\"0.4.0\")");
    Some(lines.join("\n"))
}

/// Reject a game edit that silently removes authored intent.
///
/// Compiler validity alone is too weak for `silc assist`: a model can produce a
/// valid game while dropping the camera, controller, weapons, zones, prefabs,
/// mode, post stages, encounters, NPC/mind stacks, or weapon/ability cues.
/// Unless the task explicitly asks for destructive
/// editing, every `game::*` node present in the seed must remain represented at
/// least as many times in the revision.

/// Reject mixed UI/resource programs when the task is game-shaped.
fn game_subject_purity(program: &str) -> Option<String> {
    let has_game = program.contains("game ") && program.contains("game::scene");
    if !has_game {
        return Some(
            "game-shaped tasks must declare a single `game Name { game::scene(...) }` root".into(),
        );
    }
    for banned in [
        "\nresource ",
        "\ncontract ",
        "\ncomponent ",
        "\napp ",
        "\nmethod ",
        "\nservice ",
        "\nprocessor ",
    ] {
        if program.contains(banned) || program.starts_with(banned.trim_start()) {
            return Some(format!(
                "game programs must not mix `{}` declarations with `game::scene`",
                banned.trim()
            ));
        }
    }
    None
}

fn game_intent_regression(seed: &str, candidate: &str, task: &str) -> Option<String> {
    if !seed.contains("game ") || !seed.contains("game::scene") {
        return None;
    }
    let task = task.to_ascii_lowercase();
    if ["remove", "delete", "drop", "cut", "replace"]
        .iter()
        .any(|verb| task.contains(verb))
    {
        return None;
    }

    let node_re =
        regex::Regex::new(r"game::([a-z_][a-z0-9_]*)\s*\(").expect("valid game node regex");
    let counts = |source: &str| {
        let mut out = std::collections::BTreeMap::<String, usize>::new();
        for caps in node_re.captures_iter(source) {
            *out.entry(caps[1].to_string()).or_default() += 1;
        }
        out
    };
    let before = counts(seed);
    let after = counts(candidate);
    let missing: Vec<String> = before
        .iter()
        .filter_map(|(node, expected)| {
            let actual = after.get(node).copied().unwrap_or(0);
            (actual < *expected).then(|| format!("game::{node} ({actual}/{expected})"))
        })
        .collect();
    if missing.is_empty() {
        None
    } else {
        Some(format!(
            "game intent regression: the revision removed existing nodes: {}",
            missing.join(", ")
        ))
    }
}

/// Top-level declaration keywords that share Silc's single namespace.
const DECL_KINDS: &[&str] = &[
    "contract",
    "component",
    "resource",
    "processor",
    "service",
    "subset",
    "module",
    "app",
];

/// A top-level declaration: its kind and byte span (from line start to `}`).
struct Decl {
    kind: &'static str,
    line_start: usize,
    end: usize,
}

/// Locate every top-level declaration of `name`, whatever its kind.
fn declarations(program: &str, name: &str) -> Vec<Decl> {
    let bytes = program.as_bytes();
    let mut found = Vec::new();
    for kind in DECL_KINDS {
        let Ok(re) =
            regex::Regex::new(&format!(r"(?m)^[ \t]*{}\s+{}\b", kind, regex::escape(name)))
        else {
            continue;
        };
        for m in re.find_iter(program) {
            let start = m.start();
            let line_start = program[..start].rfind('\n').map_or(0, |p| p + 1);
            let mut depth = 0i32;
            let mut end = None;
            for (offset, &b) in bytes[start..].iter().enumerate() {
                match b {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(start + offset + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(end) = end {
                found.push(Decl {
                    kind,
                    line_start,
                    end,
                });
            }
        }
    }
    found.sort_by_key(|d| d.line_start);
    found
}

/// Remove spans (assumed non-overlapping) and collapse the blank lines left behind.
fn remove_spans(program: &str, spans: &[(usize, usize)]) -> Option<String> {
    let mut ordered: Vec<(usize, usize)> = spans.to_vec();
    ordered.sort_by_key(|(start, _)| *start);
    let mut out = program.to_string();
    for (start, end) in ordered.into_iter().rev() {
        out.replace_range(start..end, "");
    }
    let cleaned = regex::Regex::new(r"\n{3,}")
        .ok()?
        .replace_all(&out, "\n\n")
        .to_string();
    Some(cleaned)
}

/// Keep the first declaration of a given kind+name and delete later repeats.
fn drop_repeated_declaration(program: &str, name: &str) -> Option<(String, &'static str)> {
    let decls = declarations(program, name);
    for kind in DECL_KINDS {
        let same: Vec<&Decl> = decls.iter().filter(|d| d.kind == *kind).collect();
        if same.len() < 2 {
            continue;
        }
        let spans: Vec<(usize, usize)> =
            same.iter().skip(1).map(|d| (d.line_start, d.end)).collect();
        return Some((remove_spans(program, &spans)?, kind));
    }
    None
}

/// Drop a colliding `contract` that shares a name with a component.
///
/// Components are what routes reference, so the component keeps the name. The
/// model often invents `contract GuestLedger` alongside `component GuestLedger`
/// as scaffolding for a list page; that copy is never used and always rejects.
fn drop_redundant_collider(program: &str, name: &str) -> Option<(String, &'static str)> {
    let decls = declarations(program, name);
    if decls.len() < 2 {
        return None;
    }
    let contract = decls.iter().find(|d| d.kind == "contract")?;
    if !decls.iter().any(|d| d.kind == "component") {
        // Fall back: a contract that is a field-for-field twin of another.
        let fields = contract_fields(&program[contract.line_start..contract.end]);
        if fields.is_empty() {
            return None;
        }
        let twin = regex::Regex::new(r"(?m)^[ \t]*contract\s+(\w+)").ok()?;
        let duplicated = twin.captures_iter(program).any(|c| {
            let other = c.get(1).map(|m| m.as_str()).unwrap_or_default();
            if other == name {
                return false;
            }
            declarations(program, other)
                .iter()
                .filter(|d| d.kind == "contract")
                .any(|d| contract_fields(&program[d.line_start..d.end]) == fields)
        });
        if !duplicated {
            return None;
        }
    }
    Some((
        remove_spans(program, &[(contract.line_start, contract.end)])?,
        "contract",
    ))
}

/// Drop contracts that are field-identical twins of another contract and whose
/// name is never referenced elsewhere in the program.
fn drop_unused_twin_contracts(program: &str) -> Option<String> {
    let header = regex::Regex::new(r"(?m)^[ \t]*contract\s+(\w+)").ok()?;
    let mut contracts: Vec<(String, usize, usize, Vec<String>)> = Vec::new();
    for caps in header.captures_iter(program) {
        let name = caps.get(1)?.as_str().to_string();
        let decls = declarations(program, &name);
        let Some(decl) = decls.iter().find(|d| d.kind == "contract") else {
            continue;
        };
        let fields = contract_fields(&program[decl.line_start..decl.end]);
        if fields.is_empty() {
            continue;
        }
        contracts.push((name, decl.line_start, decl.end, fields));
    }
    if contracts.len() < 2 {
        return None;
    }

    let mut drop_spans: Vec<(usize, usize)> = Vec::new();
    for (i, (name, start, end, fields)) in contracts.iter().enumerate() {
        let has_twin = contracts
            .iter()
            .enumerate()
            .any(|(j, (other, _, _, ofields))| i != j && other != name && ofields == fields);
        if !has_twin {
            continue;
        }
        // Referenced if the name appears outside its own declaration span.
        let before = &program[..*start];
        let after = &program[*end..];
        let referenced = name_referenced(before, name) || name_referenced(after, name);
        if !referenced {
            drop_spans.push((*start, *end));
        }
    }
    if drop_spans.is_empty() {
        return None;
    }
    remove_spans(program, &drop_spans)
}

fn name_referenced(haystack: &str, name: &str) -> bool {
    let Ok(re) = regex::Regex::new(&format!(r"\b{}\b", regex::escape(name))) else {
        return false;
    };
    re.is_match(haystack)
}

/// Drop components that are not referenced by any `route … => Name;` line.
///
/// The model invents duplicates like `GuestFormLedger` during repairs; if they
/// are never routed they are dead weight and safe to remove.
fn drop_unrouted_components(program: &str) -> Option<String> {
    let route_re = regex::Regex::new(r#"route\s+"[^"]+"\s*=>\s*(\w+)\s*;"#).ok()?;
    let routed: std::collections::HashSet<String> = route_re
        .captures_iter(program)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();
    if routed.is_empty() {
        return None;
    }

    let header = regex::Regex::new(r"(?m)^[ \t]*component\s+(\w+)").ok()?;
    let mut drop_spans: Vec<(usize, usize)> = Vec::new();
    for caps in header.captures_iter(program) {
        let name = caps.get(1)?.as_str();
        if routed.contains(name) {
            continue;
        }
        let decls = declarations(program, name);
        let Some(decl) = decls.iter().find(|d| d.kind == "component") else {
            continue;
        };
        drop_spans.push((decl.line_start, decl.end));
    }
    if drop_spans.is_empty() {
        return None;
    }
    remove_spans(program, &drop_spans)
}

/// The `has …;` field lines of a declaration body, normalised for comparison.
fn contract_fields(block: &str) -> Vec<String> {
    block
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("has "))
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// Rename a declaration that collides with a `component` or `app`.
///
/// Components and apps are referenced by routes, so they keep the name.
fn rename_colliding_declaration(program: &str, name: &str) -> Option<(String, &'static str)> {
    let decls = declarations(program, name);
    if decls.len() < 2 {
        return None;
    }
    let keeper = decls
        .iter()
        .find(|d| d.kind == "component" || d.kind == "app")?;
    let other = decls.iter().find(|d| d.kind != keeper.kind)?;
    let candidate = rename_candidates(name)
        .into_iter()
        .find(|c| !program.contains(c.as_str()))?;

    // Rewrite only the declaration header; the collider is typically unreferenced,
    // and `check_source` rejects the result if it was not.
    let header = regex::Regex::new(&format!(
        r"^([ \t]*{}\s+){}\b",
        other.kind,
        regex::escape(name)
    ))
    .ok()?;
    let block = &program[other.line_start..other.end];
    let renamed = header.replace(block, format!("${{1}}{candidate}"));
    let mut out = program.to_string();
    out.replace_range(other.line_start..other.end, &renamed);
    Some((out, other.kind))
}

/// When `component X` and `resource X` collide, rename the resource to `Xs`
/// (or `XStore`) and update `X.method()` references that follow resource usage.
fn rename_duplicate_resource(program: &str, name: &str) -> Option<String> {
    let has_component =
        regex::Regex::new(&format!(r"(?m)^\s*component\s+{}\b", regex::escape(name)))
            .ok()?
            .is_match(program);
    let resource_re =
        regex::Regex::new(&format!(r"(?m)^(\s*)resource\s+{}\b", regex::escape(name))).ok()?;
    if !resource_re.is_match(program) {
        return None;
    }

    let candidate = rename_candidates(name)
        .into_iter()
        .find(|c| !program.contains(c.as_str()))?;

    let renamed_decl = resource_re
        .replace_all(program, format!("${{1}}resource {candidate}"))
        .to_string();

    if !has_component {
        // Two resources with the same name: renaming the first declaration is enough.
        return Some(renamed_decl);
    }

    // Update resource-style call sites `Name.list()` / `Name.create()` etc.
    let call_re = regex::Regex::new(&format!(
        r"\b{}\.(list|get|create|update|delete)\(",
        regex::escape(name)
    ))
    .ok()?;
    let fixed = call_re
        .replace_all(&renamed_decl, format!("{candidate}.$1("))
        .to_string();
    Some(fixed)
}

/// Remove `seed Contract.new( … );` blocks that lack a stable `:id(`.
fn drop_seed_blocks(program: &str) -> Option<String> {
    let bytes = program.as_bytes();
    let mut out = String::with_capacity(program.len());
    let mut removed = false;
    let mut i = 0usize;

    while i < program.len() {
        let rest = &program[i..];
        let Some(rel) = rest.find("seed ") else {
            out.push_str(rest);
            break;
        };
        let seed_start = i + rel;
        let line_start = program[..seed_start].rfind('\n').map_or(0, |p| p + 1);
        let starts_line = line_start >= i
            && program[line_start..seed_start]
                .chars()
                .all(char::is_whitespace);
        if !starts_line {
            let advance = seed_start + 5;
            out.push_str(&program[i..advance]);
            i = advance;
            continue;
        }

        // Walk to the closing paren of the seed call, then the trailing `;`.
        let mut depth = 0i32;
        let mut stmt_end = None;
        let mut j = seed_start;
        while j < program.len() {
            match bytes[j] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        let mut k = j + 1;
                        while k < program.len() && (bytes[k] as char).is_whitespace() {
                            k += 1;
                        }
                        if k < program.len() && bytes[k] == b';' {
                            k += 1;
                        }
                        stmt_end = Some(k);
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        let Some(stmt_end) = stmt_end else {
            out.push_str(&program[i..]);
            break;
        };

        out.push_str(&program[i..line_start]);
        if program[seed_start..stmt_end].contains(":id(") {
            out.push_str(&program[line_start..stmt_end]);
        } else {
            removed = true;
        }
        i = stmt_end;
    }

    if !removed {
        return None;
    }
    let cleaned = regex::Regex::new(r"\n{3,}")
        .ok()?
        .replace_all(&out, "\n\n")
        .to_string();
    Some(cleaned)
}

/// Last compiler / author diagnostic when draft-first fails (for CLI errors).
#[derive(Debug, Clone, Default)]
pub struct AuthorFailure {
    pub last_error: Option<String>,
    pub best_draft: Option<String>,
}

/// Run draft-then-compile-repair. Returns `Ok(Some(result))` on success,
/// `Ok(None)` when all attempts failed (caller may fall back to tool loop).
pub fn run_author(
    task: &str,
    corpus: &Corpus,
    completer: &mut dyn Completer,
    budgets: &Budgets,
    progress: &mut Option<&mut dyn ProgressReporter>,
    state: &mut ToolState,
) -> Result<Option<AssistResult>, AssistError> {
    let (result, _) = run_author_with_failure(task, corpus, completer, budgets, progress, state)?;
    Ok(result)
}

/// Like [`run_author`], but also returns the last failure info for CLI messaging.
pub fn run_author_with_failure(
    task: &str,
    corpus: &Corpus,
    completer: &mut dyn Completer,
    budgets: &Budgets,
    progress: &mut Option<&mut dyn ProgressReporter>,
    state: &mut ToolState,
) -> Result<(Option<AssistResult>, AuthorFailure), AssistError> {
    let attempts = budgets.max_draft_attempts.max(1);
    let seed = if state.seed.trim().is_empty() {
        None
    } else {
        Some(state.seed.as_str())
    };
    let seed_present = seed.is_some();
    let ctx = select_context(task, corpus, seed);
    let mut repair_note: Option<String> = None;
    let mut last_program: Option<String> = None;
    // The diagnostic for `last_program` specifically — `failure.last_error` tracks
    // the newest error of any kind, which misreports repeats of an older draft.
    let mut last_program_error: Option<String> = None;
    let mut evidence: Vec<(String, String)> = Vec::new();
    let mut failure = AuthorFailure::default();
    // Escalated whenever a repair returns a byte-identical draft: greedy sampling
    // reproduces the same rejected program from a near-identical prompt.
    let mut temperature = BASE_TEMPERATURE;
    let deadline = std::time::Instant::now();
    // A form-sized program does not need the full token ceiling, and an
    // unbounded budget lets a degenerate generation burn a minute or more.
    let max_tokens = draft_token_budget(ctx.target.as_deref(), budgets.draft_max_tokens);

    // Closed FPS authorship helpers: graft catalog-valid additive trees the
    // small local model cannot emit without truncation.
    if let Some(seed_src) = seed {
        if let Some((fixed, what)) = inject_fps_task(task, seed_src) {
            if check_source(&fixed, None).is_ok() {
                state.stats.checks += 1;
                state.draft = fixed.clone();
                state.last_check_ok = true;
                emit(
                    progress,
                    ProgressEvent::Action {
                        turn: 1,
                        max_turns: attempts,
                        elapsed_secs: 0.0,
                        kind: ActionKind::AutoFixed { what },
                    },
                );
                emit(
                    progress,
                    ProgressEvent::Action {
                        turn: 1,
                        max_turns: attempts,
                        elapsed_secs: 0.0,
                        kind: ActionKind::Accepted,
                    },
                );
                state.stats.root_turns = state.stats.root_turns.saturating_add(1);
                return Ok((
                    Some(AssistResult {
                        program: fixed,
                        stats: state.stats.clone(),
                        finalized: true,
                    }),
                    failure,
                ));
            }
        }
    }

    for attempt in 1..=attempts {
        if deadline.elapsed().as_secs() >= budgets.wall_clock_secs {
            if failure.last_error.is_none() {
                failure.last_error = Some(format!(
                    "wall clock budget exhausted ({}s)",
                    budgets.wall_clock_secs
                ));
            }
            break;
        }
        emit(
            progress,
            ProgressEvent::Action {
                turn: attempt,
                max_turns: attempts,
                elapsed_secs: 0.0,
                kind: if repair_note.is_some() {
                    ActionKind::Repairing {
                        reason: truncate_one_line(
                            repair_note.as_deref().unwrap_or("fixing draft"),
                            72,
                        ),
                    }
                } else {
                    ActionKind::Drafting { attempt, attempts }
                },
            },
        );
        if !evidence.is_empty() {
            let ids: Vec<String> = evidence.iter().map(|(id, _)| id.clone()).collect();
            emit(
                progress,
                ProgressEvent::Action {
                    turn: attempt,
                    max_turns: attempts,
                    elapsed_secs: 0.0,
                    kind: ActionKind::RetrievedEvidence {
                        hits: evidence.len(),
                        ids,
                    },
                },
            );
        }
        emit(
            progress,
            ProgressEvent::Thinking {
                turn: attempt,
                max_turns: attempts,
            },
        );

        let user = build_user_prompt(
            task,
            &ctx,
            repair_note.as_deref(),
            last_program.as_deref(),
            &evidence,
            seed_present,
        );
        let started = std::time::Instant::now();
        let mut req = ChatRequest::new(AUTHOR_SYSTEM_PROMPT, user, max_tokens);
        req.stop = AUTHOR_STOP.iter().map(|s| (*s).to_string()).collect();
        req.temperature = Some(temperature);
        let reply = completer.chat(&req).map_err(AssistError::Completer)?;
        let elapsed = started.elapsed().as_secs_f64();

        let mut program = strip_end_marker(&normalize_typography(&extract_program(&reply.text)));
        if let Some(versioned) = ensure_version(&program) {
            program = versioned;
            emit(
                progress,
                ProgressEvent::Action {
                    turn: attempt,
                    max_turns: attempts,
                    elapsed_secs: 0.0,
                    kind: ActionKind::AutoFixed {
                        what: "added missing @version(\"0.4.0\")".into(),
                    },
                },
            );
        }
        if program.trim().is_empty() || !program.contains("@version(") {
            let got = truncate_one_line(reply.text.trim(), 48);
            failure.last_error = Some("reply was not a Silc program".into());
            repair_note = Some(
                "previous reply was empty or not a Silc program. Output ONLY the complete Silc source starting with #!/usr/bin/env silc, ending with # END."
                    .into(),
            );
            emit(
                progress,
                ProgressEvent::Action {
                    turn: attempt,
                    max_turns: attempts,
                    elapsed_secs: elapsed,
                    kind: ActionKind::StillRefining {
                        reason: if got.is_empty() {
                            "model returned nothing — retrying".into()
                        } else {
                            format!("reply was not a Silc program (got: {got})")
                        },
                    },
                },
            );
            continue;
        }

        // Truncated but structurally complete: try compile anyway.
        if reply.truncated && !looks_complete(&program) {
            last_program = Some(program.clone());
            last_program_error = Some("draft truncated mid-file".into());
            failure.best_draft = Some(program.clone());
            failure.last_error = Some("draft truncated mid-file".into());
            let tail = draft_tail(&program, 80);
            repair_note = Some(format!(
                "previous program was cut off mid-file (token limit). Finish from the last incomplete block. Output the FULL corrected program ending with # END.\n\n# Tail of previous draft\n{tail}"
            ));
            emit(
                progress,
                ProgressEvent::Action {
                    turn: attempt,
                    max_turns: attempts,
                    elapsed_secs: elapsed,
                    kind: ActionKind::StillRefining {
                        reason: "draft truncated — continuing from tail".into(),
                    },
                },
            );
            continue;
        }

        if program.len() < MIN_DRAFT_CHARS {
            failure.last_error = Some(format!("draft too short ({} chars)", program.len()));
            repair_note = Some(format!(
                "previous draft was only {} chars (need ≥{MIN_DRAFT_CHARS}). Write the COMPLETE program ending with # END.",
                program.len()
            ));
            emit(
                progress,
                ProgressEvent::Action {
                    turn: attempt,
                    max_turns: attempts,
                    elapsed_secs: elapsed,
                    kind: ActionKind::PreparedCode {
                        chars: program.len(),
                        preview: String::new(),
                        short_rejected: true,
                        unchanged: false,
                    },
                },
            );
            continue;
        }

        // A repair that reproduces the rejected draft byte-for-byte cannot pass the
        // same check: raise temperature and demand a structural change instead of
        // spending an attempt on an identical compile.
        if last_program.as_deref() == Some(program.as_str()) {
            temperature = (temperature + TEMPERATURE_STEP).min(MAX_TEMPERATURE);
            let prior = last_program_error
                .clone()
                .or_else(|| failure.last_error.clone())
                .unwrap_or_default();
            repair_note = Some(format!(
                "you returned the SAME program again and it is still rejected: {prior}\nYou MUST change the declarations that caused it — do not repeat the previous output. Output the FULL corrected program ending with # END."
            ));
            emit(
                progress,
                ProgressEvent::Action {
                    turn: attempt,
                    max_turns: attempts,
                    elapsed_secs: elapsed,
                    kind: ActionKind::StillRefining {
                        reason: format!(
                            "identical draft returned — retrying at temperature {temperature:.1}"
                        ),
                    },
                },
            );
            continue;
        }

        if ctx.target_is_starter && ctx.target.as_deref().map(str::trim) == Some(program.trim()) {
            failure.last_error = Some("returned the starter skeleton unchanged".into());
            repair_note = Some(
                "you returned the starting skeleton unchanged. Adapt it to the task: rename the contract/component/app and set the fields and form the task describes. Output the FULL program ending with # END."
                    .into(),
            );
            emit(
                progress,
                ProgressEvent::Action {
                    turn: attempt,
                    max_turns: attempts,
                    elapsed_secs: elapsed,
                    kind: ActionKind::PreparedCode {
                        chars: program.len(),
                        preview: String::new(),
                        short_rejected: false,
                        unchanged: true,
                    },
                },
            );
            continue;
        }

        if state.is_unchanged_seed(&program) {
            failure.last_error = Some("program unchanged from original".into());
            last_program = Some(program.clone());
            last_program_error = Some("returned the original file unchanged".into());
            let what_to_change = task_pattern(task, ctx.target.as_deref())
                .map(|p| format!("\n\nApply this:\n{p}"))
                .unwrap_or_default();
            repair_note = Some(format!(
                "previous reply returned the original file unchanged. {UNCHANGED_SEED_ERROR}{what_to_change}"
            ));
            emit(
                progress,
                ProgressEvent::Action {
                    turn: attempt,
                    max_turns: attempts,
                    elapsed_secs: elapsed,
                    kind: ActionKind::PreparedCode {
                        chars: program.len(),
                        preview: String::new(),
                        short_rejected: false,
                        unchanged: true,
                    },
                },
            );
            continue;
        }

        emit(
            progress,
            ProgressEvent::Action {
                turn: attempt,
                max_turns: attempts,
                elapsed_secs: elapsed,
                kind: ActionKind::PreparedCode {
                    chars: program.len(),
                    preview: draft_preview(&program, 4),
                    short_rejected: false,
                    unchanged: false,
                },
            },
        );

        if state.stats.checks >= budgets.max_silc_check {
            break;
        }
        if ctx.game_catalog.is_some() {
            if let Some(error) = game_subject_purity(&program) {
                state.last_check_ok = false;
                failure.best_draft = Some(program.clone());
                failure.last_error = Some(error.clone());
                last_program = Some(program);
                last_program_error = Some(error.clone());
                repair_note = Some(format!(
                    "{error}\nGAME SUBJECT RULE: emit only `#!/usr/bin/env silc`, `@version(...)`, \
                     and one `game Name {{ game::scene(...) }}` tree. No resource/contract/component/\
                     app/method blocks. Output the FULL corrected game program ending with # END."
                ));
                emit(
                    progress,
                    ProgressEvent::Action {
                        turn: attempt,
                        max_turns: attempts,
                        elapsed_secs: 0.0,
                        kind: ActionKind::Checked {
                            ok: false,
                            detail: truncate_one_line(&error, 80),
                        },
                    },
                );
                continue;
            }
        }
        state.stats.checks += 1;
        match check_source(&program, None) {
            Ok(_) => {
                if let Some(error) =
                    game_intent_regression(seed.unwrap_or_default(), &program, task)
                {
                    state.last_check_ok = false;
                    failure.best_draft = Some(program.clone());
                    failure.last_error = Some(error.clone());
                    last_program = Some(program);
                    last_program_error = Some(error.clone());
                    repair_note = Some(format!(
                        "{error}\nRestore every listed node while making the requested improvement. \
                         Preserve existing game systems and effect multiplicity unless the task explicitly \
                         asks to remove them. Output the FULL corrected program ending with # END."
                    ));
                    emit(
                        progress,
                        ProgressEvent::Action {
                            turn: attempt,
                            max_turns: attempts,
                            elapsed_secs: 0.0,
                            kind: ActionKind::Checked {
                                ok: false,
                                detail: truncate_one_line(&error, 80),
                            },
                        },
                    );
                    continue;
                }
                // Drop unused twin contracts (field-identical copies that nothing
                // references) — the model invents them and the compiler allows them.
                let program = match drop_unused_twin_contracts(&program) {
                    Some(cleaned) if check_source(&cleaned, None).is_ok() => {
                        state.stats.checks += 1;
                        emit(
                            progress,
                            ProgressEvent::Action {
                                turn: attempt,
                                max_turns: attempts,
                                elapsed_secs: 0.0,
                                kind: ActionKind::AutoFixed {
                                    what: "removed unused twin contract".into(),
                                },
                            },
                        );
                        cleaned
                    }
                    _ => program,
                };
                let program = match drop_unrouted_components(&program) {
                    Some(cleaned) if check_source(&cleaned, None).is_ok() => {
                        state.stats.checks += 1;
                        emit(
                            progress,
                            ProgressEvent::Action {
                                turn: attempt,
                                max_turns: attempts,
                                elapsed_secs: 0.0,
                                kind: ActionKind::AutoFixed {
                                    what: "removed unrouted duplicate component".into(),
                                },
                            },
                        );
                        cleaned
                    }
                    _ => program,
                };
                state.draft = program.clone();
                state.last_check_ok = true;
                emit(
                    progress,
                    ProgressEvent::Action {
                        turn: attempt,
                        max_turns: attempts,
                        elapsed_secs: 0.0,
                        kind: ActionKind::Checked {
                            ok: true,
                            detail: "ok".into(),
                        },
                    },
                );
                emit(
                    progress,
                    ProgressEvent::Action {
                        turn: attempt,
                        max_turns: attempts,
                        elapsed_secs: 0.0,
                        kind: ActionKind::Accepted,
                    },
                );
                state.stats.root_turns = state.stats.root_turns.saturating_add(attempt);
                return Ok((
                    Some(AssistResult {
                        program,
                        stats: state.stats.clone(),
                        finalized: true,
                    }),
                    failure,
                ));
            }
            Err(error) => {
                state.last_check_ok = false;
                failure.best_draft = Some(program.clone());
                failure.last_error = Some(error.clone());
                let stage = error.split_once(':').map(|(s, _)| s).unwrap_or("unknown");
                emit(
                    progress,
                    ProgressEvent::Action {
                        turn: attempt,
                        max_turns: attempts,
                        elapsed_secs: 0.0,
                        kind: ActionKind::Checked {
                            ok: false,
                            detail: truncate_one_line(&error, 80),
                        },
                    },
                );

                // Mechanical diagnostics (name collisions, seeds without `:id`) are
                // repaired here rather than costing another model round-trip.
                if let Some((fixed, what)) = autofix(&program, &error) {
                    if !state.is_unchanged_seed(&fixed) && check_source(&fixed, None).is_ok() {
                        state.stats.checks += 1;
                        state.draft = fixed.clone();
                        state.last_check_ok = true;
                        emit(
                            progress,
                            ProgressEvent::Action {
                                turn: attempt,
                                max_turns: attempts,
                                elapsed_secs: 0.0,
                                kind: ActionKind::AutoFixed { what },
                            },
                        );
                        emit(
                            progress,
                            ProgressEvent::Action {
                                turn: attempt,
                                max_turns: attempts,
                                elapsed_secs: 0.0,
                                kind: ActionKind::Checked {
                                    ok: true,
                                    detail: "ok".into(),
                                },
                            },
                        );
                        emit(
                            progress,
                            ProgressEvent::Action {
                                turn: attempt,
                                max_turns: attempts,
                                elapsed_secs: 0.0,
                                kind: ActionKind::Accepted,
                            },
                        );
                        state.stats.root_turns = state.stats.root_turns.saturating_add(attempt);
                        return Ok((
                            Some(AssistResult {
                                program: fixed,
                                stats: state.stats.clone(),
                                finalized: true,
                            }),
                            failure,
                        ));
                    }
                }

                // Truncated additive FPS drafts: graft new data/weapon/zone nodes
                // from the broken draft onto the known-good seed scene tree, or
                // inject the closed four-weapon loadout named by the task.
                if let Some(seed_src) = ctx.target.as_deref().filter(|s| s.contains("game::scene")) {
                    let merged = inject_fps_task(task, seed_src)
                        .map(|(p, _)| p)
                        .or_else(|| merge_additive_game_draft(seed_src, &program));
                    if let Some(merged) = merged {
                        if !state.is_unchanged_seed(&merged) && check_source(&merged, None).is_ok()
                        {
                            state.stats.checks += 1;
                            state.draft = merged.clone();
                            state.last_check_ok = true;
                            emit(
                                progress,
                                ProgressEvent::Action {
                                    turn: attempt,
                                    max_turns: attempts,
                                    elapsed_secs: 0.0,
                                    kind: ActionKind::AutoFixed {
                                        what: "merged additive game nodes into seed scene".into(),
                                    },
                                },
                            );
                            emit(
                                progress,
                                ProgressEvent::Action {
                                    turn: attempt,
                                    max_turns: attempts,
                                    elapsed_secs: 0.0,
                                    kind: ActionKind::Checked {
                                        ok: true,
                                        detail: "ok".into(),
                                    },
                                },
                            );
                            emit(
                                progress,
                                ProgressEvent::Action {
                                    turn: attempt,
                                    max_turns: attempts,
                                    elapsed_secs: 0.0,
                                    kind: ActionKind::Accepted,
                                },
                            );
                            state.stats.root_turns = state.stats.root_turns.saturating_add(attempt);
                            return Ok((
                                Some(AssistResult {
                                    program: merged,
                                    stats: state.stats.clone(),
                                    finalized: true,
                                }),
                                failure,
                            ));
                        }
                    }
                }

                let site = error_site(&program, &error)
                    .map(|s| format!("\n\n# Rejected source\n{s}"))
                    .unwrap_or_default();
                last_program = Some(program);
                last_program_error = Some(error.clone());
                // Structural diagnostics get an explicit rule; only fall back to
                // corpus grep when no rule matches, since generic hits do not
                // teach the model which declaration to change.
                if ctx.game_catalog.is_some()
                    || seed.is_some_and(|source| source.contains("game::scene"))
                {
                    evidence.clear();
                    repair_note = Some(format!(
                        "compiler rejected the previous GAME program (stage={stage}): {error}{site}\n\n\
                         GAME REPAIR RULE: keep a single `game Name {{ game::scene(...) }}` \
                         structure. Do not invent contracts, components, resources, apps, services, or \
                         processors. Repair only the game syntax reported by the compiler and preserve \
                         every existing `game::*` node when modifying.\n\n\
                         Output the FULL corrected game program ending with # END."
                    ));
                } else {
                    match repair_guidance(&error) {
                        Some(guidance) => {
                            evidence.clear();
                            repair_note = Some(format!(
                            "compiler rejected the previous program (stage={stage}): {error}{site}\n\n{guidance}\n\nOutput the FULL corrected program ending with # END."
                        ));
                        }
                        None => {
                            evidence = retrieve_for_error(corpus, &error);
                            repair_note = Some(format!(
                            "compiler rejected the previous program (stage={stage}): {error}{site}\nFix using the corpus evidence below and output the FULL corrected program ending with # END."
                        ));
                        }
                    }
                }
            }
        }
    }

    state.stats.root_turns = state.stats.root_turns.saturating_add(attempts);
    Ok((None, failure))
}

fn build_user_prompt(
    task: &str,
    ctx: &AuthorContext,
    repair: Option<&str>,
    last_program: Option<&str>,
    evidence: &[(String, String)],
    seed_present: bool,
) -> String {
    let mut out = String::new();
    if !ctx.rules.is_empty() {
        out.push_str("# Silc language rules (condensed)\n");
        out.push_str(&ctx.rules);
        out.push_str("\n\n");
    }
    if let Some(catalog) = &ctx.game_catalog {
        out.push_str("# Authoritative game::* catalog\n");
        out.push_str(catalog);
        out.push_str("\n\n");
    }
    for (id, body) in &ctx.examples {
        out.push_str(&format!("# Reference example ({id})\n"));
        out.push_str(body);
        out.push_str("\n\n");
    }
    if let Some(target) = &ctx.target {
        if ctx.target_is_starter {
            out.push_str("# Starting skeleton (this compiles — adapt it)\n");
        } else {
            out.push_str("# Current file to modify\n");
        }
        out.push_str(target);
        out.push_str("\n\n");
    }
    out.push_str("# Task\n");
    out.push_str(task);
    out.push('\n');
    if let Some(pattern) = task_pattern(task, ctx.target.as_deref()) {
        out.push_str("\n# Required pattern for this task\n");
        out.push_str(&pattern);
        out.push('\n');
    }
    if seed_present {
        if ctx
            .target
            .as_deref()
            .is_some_and(|target| target.contains("game::scene"))
        {
            out.push_str(
                "\n# Modify guidance\nPrefer the SMALLEST edit that fulfills the task inside the existing `game::scene` tree. Keep the same `game Name`, `:title`, and every system the task did not name.\nGAME PRESERVATION RULE: a game edit is additive unless the task explicitly says to remove a system. Preserve every existing `game::*` node, repeated post stage, prefab, data asset, asset/material declaration, zone, weapon, spawn, signal, encounter, NPC/mind stack, HUD, and weapon/ability cue. Never trade away camera (especially `:mode(first_person)`), controller, mode, pawn, prefab, entity/zone tree, weapons, overlay, post-processing, or encounters merely to shorten the output. Prefer Godot-style nested `entity`/`zone` trees, Unity-style `prefab`/`data`/`asset`/`material`/`spawn`, and Unreal-style `mode`/`pawn`/`controller` with FPS weapons and encounter waves.\nDo not invent `app` / `component` / `resource` / `processor`. Use only closed `game::*` catalog nodes and props.\n",
            );
        } else {
            out.push_str(
                "\n# Modify guidance\nPrefer the SMALLEST edit that fulfills the task. Keep existing declarations, names, routes, and handlers unless the task explicitly asks to change them.\nPrefer extending the existing target structure (contract fields + form + optional processor). Do not invent `resource` / seeds unless the task requires a persistent ledger list; if you add seeds every row must include `:id(\"…\")`.\nDo NOT invent extra components, contracts, or processors that the task did not ask for. If the task says DELETE a block, remove only that block.\n`ui::app_bar` only accepts `:title`. `ui::text_input` / `ui::textarea` do not accept `:variant`. Put `:active` only on `ui::nav_item`.\n",
            );
        }
    } else if ctx.target_is_starter {
        if ctx
            .target
            .as_deref()
            .is_some_and(|target| target.contains("game::scene"))
        {
            out.push_str(
                "\n# Build guidance\nStart from the arena/FPS game skeleton above and adapt it for the task: rename the `game`, tune `:title`, and adjust prefabs / zones / weapons / world entities / encounters / post stages as needed.\nKeep a single `game Name { game::scene(...) }` root. Required playable systems: prefab(+mesh/collider/movement/pawn/weapon), data weapon/locomotion profiles, optional asset/material, world entity/zone tree, spawn, mode, controller, first_person camera, hud, weapons with cues, post_process, overlay; add encounters and NPC/mind stacks when the task needs combat AI.\nDo not add `app`, `component`, `resource`, or `processor`.\n",
            );
        } else {
            out.push_str(
                "\n# Build guidance\nStart from the skeleton above and change it to fit the task: rename the contract/component/app, set the contract fields, and build the form and render tree the task needs. Keep its structure and syntax.\nKeep `method on_submit() { submit(); }` exactly as written — submission is synthesized, so do not invent pipeline ops or `==>` chains inside it.\nEvery `method` is a SIBLING: close `render()` with `}` before declaring the next method — never nest a method inside another.\nDo not add a `resource`, `query` or `processor` unless the task clearly needs stored records.\n",
            );
        }
    }
    if let Some(note) = repair {
        out.push_str("\n# Repair\n");
        out.push_str(note);
        out.push('\n');
        if !evidence.is_empty() {
            out.push_str("\n# Corpus evidence (fix using this)\n");
            for (id, body) in evidence {
                out.push_str(&format!("## {id}\n{body}\n\n"));
            }
        }
        if let Some(prev) = last_program {
            // On truncation repairs we already embedded a tail in the note;
            // still include a bounded previous draft for compile failures.
            let bounded = if ctx
                .target
                .as_deref()
                .is_some_and(|target| target.contains("game::scene"))
            {
                prev.to_string()
            } else if prev.lines().count() > 120 {
                draft_tail(prev, 100)
            } else {
                prev.to_string()
            };
            out.push_str("\n# Previous draft (fix this)\n");
            out.push_str(&bounded);
            out.push('\n');
        }
    }
    out.push_str(
        "\n# Output\nRewrite the file to fulfil the task. Output ONLY the complete Silc source, starting with the line #!/usr/bin/env silc. End with a line containing only # END. No explanation, no markdown, no <tool> blocks.\n",
    );
    out
}

/// Strip a trailing `# END` stop marker (and anything after it).
pub fn strip_end_marker(source: &str) -> String {
    let trimmed = source.trim();
    if let Some(idx) = trimmed.rfind("\n# END") {
        return trimmed[..idx].trim().to_string();
    }
    if trimmed.ends_with("# END") {
        return trimmed.trim_end_matches("# END").trim().to_string();
    }
    // Also strip if the model put # END on its own after whitespace.
    let lines: Vec<&str> = trimmed.lines().collect();
    if let Some(last) = lines.last() {
        if last.trim() == "# END" {
            return lines[..lines.len() - 1].join("\n").trim().to_string();
        }
    }
    trimmed.to_string()
}

/// True when a (possibly truncated) draft still has the structural end of an app.
pub fn looks_complete(program: &str) -> bool {
    if program.contains("game ") && program.contains("game::scene") {
        return program.contains("@version(");
    }
    program.contains("@version(")
        && program.contains("app ")
        && program.contains("route ")
        && program.contains("component ")
}

fn draft_tail(program: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = program.lines().collect();
    if lines.len() <= max_lines {
        return program.to_string();
    }
    let start = lines.len() - max_lines;
    format!("…\n{}", lines[start..].join("\n"))
}

fn error_keywords(error: &str) -> Vec<String> {
    error
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| t.len() >= 3)
        .filter(|t| {
            !matches!(
                *t,
                "the"
                    | "and"
                    | "for"
                    | "with"
                    | "must"
                    | "include"
                    | "validate"
                    | "parse"
                    | "error"
                    | "stage"
                    | "from"
            )
        })
        .map(str::to_string)
        .collect()
}

fn line_window(corpus: &Corpus, id: &str, line_no: usize, radius: usize) -> Option<String> {
    let body = corpus.get(id)?;
    let lines: Vec<&str> = body.lines().collect();
    if lines.is_empty() {
        return None;
    }
    let idx = if line_no == 0 {
        0
    } else {
        line_no.saturating_sub(1)
    };
    let start = idx.saturating_sub(radius);
    let end = (idx + radius + 1).min(lines.len());
    Some(lines[start..end].join("\n"))
}

/// Extract `(line, col)` from a positioned diagnostic like `parse: 8:5: …`.
fn error_location(error: &str) -> Option<(usize, usize)> {
    let re = regex::Regex::new(r"(\d+):(\d+):").ok()?;
    let caps = re.captures(error)?;
    let line = caps.get(1)?.as_str().parse().ok()?;
    let col = caps.get(2)?.as_str().parse().ok()?;
    Some((line, col))
}

/// Quote the rejected line (with a caret) so the repair prompt points at the
/// exact source the compiler objected to rather than the whole file.
fn error_site(program: &str, error: &str) -> Option<String> {
    let (line_no, col) = error_location(error)?;
    let lines: Vec<&str> = program.lines().collect();
    if line_no == 0 || line_no > lines.len() {
        return None;
    }
    let idx = line_no - 1;
    let start = idx.saturating_sub(2);
    let mut out = String::new();
    for (offset, text) in lines[start..=idx].iter().enumerate() {
        out.push_str(&format!("{:>4} | {text}\n", start + offset + 1));
    }
    let caret_pad = " ".repeat(col.saturating_sub(1).min(200));
    out.push_str(&format!("     | {caret_pad}^ here\n"));
    Some(out)
}

fn task_tokens(task: &str) -> Vec<String> {
    task.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .filter(|t| {
            !matches!(
                *t,
                "the"
                    | "and"
                    | "for"
                    | "with"
                    | "you"
                    | "are"
                    | "this"
                    | "that"
                    | "simple"
                    | "should"
                    | "from"
                    | "your"
                    | "have"
                    | "into"
                    | "allow"
                    | "people"
            )
        })
        .map(str::to_string)
        .collect()
}

fn overlap_score(tokens: &[String], id: &str, body: &str) -> i32 {
    let hay = format!("{id}\n{body}").to_lowercase();
    tokens.iter().filter(|t| hay.contains(t.as_str())).count() as i32
}

fn condense_rules(src: &str, max_chars: usize) -> String {
    // Prefer sections that look like syntax/contract guidance, and always
    // keep hard validation rules about resource seeds / :id.
    let mut kept = String::new();
    let mut in_keep = false;
    for line in src.lines() {
        let lower = line.to_lowercase();
        if line.starts_with('#') {
            in_keep = lower.contains("syntax")
                || lower.contains("contract")
                || lower.contains("component")
                || lower.contains("ui::")
                || lower.contains("game")
                || lower.contains("webgpu")
                || lower.contains("adr-012")
                || lower.contains("quick")
                || lower.contains("language")
                || lower.contains("module")
                || lower.contains("app ")
                || lower.contains("processor")
                || lower.contains("form")
                || lower.contains("state")
                || lower.contains("resource")
                || lower.contains("seed");
        }
        let hard_rule = lower.contains(":id")
            || lower.contains("seed")
            || lower.contains("insert or ignore")
            || lower.contains("resource ")
            || lower.contains("game::");
        if in_keep
            || hard_rule
            || line.starts_with("@")
            || line.contains("ui::")
            || line.contains("game::")
        {
            if kept.chars().count() + line.chars().count() + 1 > max_chars {
                break;
            }
            kept.push_str(line);
            kept.push('\n');
        }
    }
    if kept.trim().is_empty() {
        truncate_chars(src, max_chars)
    } else {
        // Ensure the seed/:id paragraph is present even if section filtering missed it.
        if !kept.to_lowercase().contains(":id") {
            if let Some(idx) = src.find("stable `:id") {
                let snippet = &src[idx.saturating_sub(80)..(idx + 200).min(src.len())];
                kept.push_str("\n# Resource seeds (required)\n");
                kept.push_str(snippet);
                kept.push('\n');
            } else if let Some(idx) = src.find(":id(\"") {
                let snippet = &src[idx.saturating_sub(120)..(idx + 180).min(src.len())];
                kept.push_str("\n# Resource seeds (required)\n");
                kept.push_str(snippet);
                kept.push('\n');
            }
        }
        kept
    }
}

fn truncate_chars(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max).collect();
        out.push_str("\n…");
        out
    }
}

fn emit(progress: &mut Option<&mut dyn ProgressReporter>, event: ProgressEvent) {
    if let Some(p) = progress.as_mut() {
        p.on_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complete::ScriptedCompleter;

    #[test]
    fn select_context_prefers_form_fixtures_for_signup_task() {
        let corpus = Corpus::builtin();
        let ctx = select_context(
            "hotel sign up form with name phone room comment",
            &corpus,
            Some("#!/usr/bin/env silc\n@version(\"0.4.0\")\n"),
        );
        assert!(!ctx.examples.is_empty());
        assert!(ctx.target.is_some());
        let ids: Vec<_> = ctx.examples.iter().map(|(id, _)| id.as_str()).collect();
        assert!(
            ids.iter()
                .any(|id| id.contains("fixture") || id.contains("example")),
            "expected examples, got {ids:?}"
        );
    }

    #[test]
    fn strip_end_marker_removes_trailing_marker() {
        let src = "#!/usr/bin/env silc\n@version(\"0.4.0\")\napp X { route \"/\" => Y; }\n# END\n";
        let out = strip_end_marker(src);
        assert!(!out.contains("# END"));
        assert!(out.contains("@version"));
    }

    #[test]
    fn looks_complete_requires_app_route() {
        assert!(!looks_complete("@version(\"0.4.0\")\ncomponent X {}"));
        assert!(looks_complete(
            "@version(\"0.4.0\")\ncomponent Home {}\napp App { route \"/\" => Home; }\n"
        ));
        assert!(looks_complete(
            "@version(\"0.4.0\")\ngame Demo { game::scene(:title(\"T\"), game::overlay(:toggle(\"F1\"))) }\n"
        ));
    }

    #[test]
    fn retrieve_for_error_finds_seed_id_rule() {
        let corpus = Corpus::builtin();
        let error = "validate: resource `Guests` seed #1 must include a stable `:id(\"…\")` for idempotent inserts";
        let hits = retrieve_for_error(&corpus, error);
        assert!(!hits.is_empty(), "expected corpus evidence for seed :id");
        let joined = hits
            .iter()
            .map(|(id, body)| format!("{id}\n{body}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains(":id") || joined.to_lowercase().contains("seed"),
            "evidence should mention :id/seed, got:\n{joined}"
        );
    }

    #[test]
    fn repair_guidance_names_the_duplicate_declaration() {
        let guidance = repair_guidance("validate: duplicate resource name `GuestForm`")
            .expect("duplicate name should have deterministic guidance");
        assert!(guidance.contains("GuestForm"));
        assert!(guidance.contains("GuestForms") || guidance.contains("unique"));
    }

    #[test]
    fn repair_guidance_covers_seed_id_rule() {
        let guidance =
            repair_guidance("validate: resource `Guests` seed #1 must include a stable `:id`")
                .expect("seed rule should have guidance");
        assert!(guidance.contains(":id"));
    }

    #[test]
    fn autofix_renames_resource_colliding_with_component() {
        let program = concat!(
            "#!/usr/bin/env silc\n@version(\"0.4.0\")\n",
            "contract Guest { has Str $.name; }\n",
            "component GuestForm { method render() { ui::stack() } }\n",
            "resource GuestForm for Guest {\n    query list;\n    mutation create;\n}\n",
            "app A { route \"/\" => GuestForm; }\n"
        );
        let (fixed, what) = autofix(program, "validate: duplicate resource name `GuestForm`")
            .expect("duplicate resource should autofix");
        assert!(what.contains("GuestForm"));
        assert!(fixed.contains("resource GuestForms for Guest"));
        assert!(fixed.contains("component GuestForm "));
        assert!(!fixed.contains("resource GuestForm for"));
    }

    #[test]
    fn autofix_drops_seeds_without_stable_id() {
        let program = concat!(
            "#!/usr/bin/env silc\n@version(\"0.4.0\")\n",
            "resource Guests for Guest {\n",
            "    query list;\n",
            "    seed Guest.new(:name(\"Ada\"), :room(\"101\"));\n",
            "    seed Guest.new(:id(\"g-1\"), :name(\"Bo\"), :room(\"102\"));\n",
            "}\n"
        );
        let (fixed, _) = autofix(program, "validate: seed #1 must include a stable `:id`")
            .expect("seed autofix expected");
        assert!(!fixed.contains(":name(\"Ada\")"), "unstable seed should go");
        assert!(fixed.contains(":id(\"g-1\")"), "stable seed should stay");
        assert!(fixed.contains("query list;"));
    }

    /// The failure the model actually produced: a resource named like a component.
    #[test]
    fn autofix_output_compiles_for_real_collision() {
        let corpus = Corpus::builtin();
        let source = corpus.get("fixture/shopping_app.silc").unwrap();
        let colliding = source
            .replace(
                "resource Products for Product",
                "resource ProductCard for Product",
            )
            .replace("Products.", "ProductCard.");
        let error = check_source(&colliding, None)
            .expect_err("component/resource name collision must be rejected");
        assert!(error.contains("duplicate"), "unexpected error: {error}");

        let (fixed, _) = autofix(&colliding, &error).expect("autofix expected");
        assert!(
            check_source(&fixed, None).is_ok(),
            "autofixed program must compile, got {:?}",
            check_source(&fixed, None)
        );
    }

    /// The exact shape the model produced: `on_submit` nested inside `render`.
    #[test]
    fn autofix_hoists_method_nested_in_render() {
        let program = concat!(
            "#!/usr/bin/env silc\n@version(\"0.4.0\")\n",
            "contract Visitor {\n    has Str $.name;\n}\n\n",
            "component HelloForm {\n",
            "    has state Str $.name = \"\";\n\n",
            "    method render() {\n",
            "        ui::stack(\n",
            "            ui::text_input(:field(name), :label(\"Your Name\"))\n",
            "        )\n\n",
            "        method on_submit() {\n",
            "            submit();\n",
            "        }\n",
            "    }\n",
            "}\n\n",
            "app MyApp {\n    route \"/\" => HelloForm;\n}\n"
        );
        let error = check_source(program, None).expect_err("nested method must be rejected");
        let (fixed, what) = autofix(program, &error).expect("nested method should autofix");
        assert!(what.contains("on_submit"), "got: {what}");
        assert!(
            check_source(&fixed, None).is_ok(),
            "hoisted program must compile:\n{fixed}\n{:?}",
            check_source(&fixed, None)
        );
    }

    #[test]
    fn select_context_uses_starter_skeleton_when_creating() {
        let corpus = Corpus::builtin();
        let ctx = select_context("a hello world greeting page", &corpus, None);
        assert!(ctx.target_is_starter);
        let target = ctx.target.expect("create path should get the starter");
        assert!(
            target.contains("@version("),
            "starter must be a real program"
        );
        assert!(check_source(&target, None).is_ok(), "starter must compile");
    }

    #[test]
    fn select_context_keeps_real_target_over_starter() {
        let corpus = Corpus::builtin();
        let seed = "#!/usr/bin/env silc\n@version(\"0.4.0\")\n# mine\n";
        let ctx = select_context("edit it", &corpus, Some(seed));
        assert!(!ctx.target_is_starter);
        assert_eq!(ctx.target.as_deref(), Some(seed));
    }

    #[test]
    fn repair_guidance_explains_mutation_shape() {
        let guidance = repair_guidance("parse: 32:13: expected expression")
            .expect("expression errors should have guidance");
        assert!(guidance.contains("Guest.new("));
        assert!(guidance.contains("$.field") || guidance.contains("$.name"));
    }

    #[test]
    fn rename_candidates_never_double_pluralise() {
        assert_eq!(rename_candidates("Guests")[0], "GuestsStore");
        assert_eq!(rename_candidates("Guest")[0], "Guests");
    }

    #[test]
    fn repair_guidance_suggests_a_sane_rename_for_plural_names() {
        let guidance = repair_guidance("validate: duplicate resource name `Guests`")
            .expect("duplicate name guidance");
        assert!(guidance.contains("GuestsStore"));
        assert!(!guidance.contains("Guestss"), "must not suggest Guestss");
        assert!(guidance.contains("delete the duplicate"));
    }

    /// The model emitted the same resource block twice; drop the copy.
    #[test]
    fn autofix_removes_a_repeated_resource_block() {
        let program = concat!(
            "#!/usr/bin/env silc\n@version(\"0.4.0\")\n",
            "contract Guest {\n    has Str $.name;\n}\n\n",
            "resource Guests for Guest {\n    query list;\n    mutation create;\n}\n\n",
            "component GuestForm {\n    has state Str $.name = \"\";\n\n",
            "    method render() {\n        ui::stack(\n",
            "            ui::text_input(:field(name), :label(\"Name\"))\n        )\n    }\n\n",
            "    method on_submit() {\n",
            "        Guests.create(Guest.new(:name($.name)));\n        $.name = \"\";\n    }\n}\n\n",
            "app HotelApp {\n    route \"/\" => GuestForm;\n}\n\n",
            "resource Guests for Guest {\n    query list;\n    mutation create;\n}\n"
        );
        let error = check_source(program, None).expect_err("repeated resource must be rejected");
        let (fixed, what) = autofix(program, &error).expect("repeat should autofix");
        assert!(what.contains("repeated"), "got: {what}");
        assert_eq!(
            fixed.matches("resource Guests for Guest").count(),
            1,
            "exactly one resource block should remain:\n{fixed}"
        );
        assert!(
            check_source(&fixed, None).is_ok(),
            "de-duplicated program must compile:\n{fixed}\n{:?}",
            check_source(&fixed, None)
        );
    }

    /// The model invented `contract GuestForm` as a copy of `contract Guest`,
    /// colliding with `component GuestForm`.
    #[test]
    fn autofix_drops_contract_that_collides_with_component() {
        let program = concat!(
            "#!/usr/bin/env silc\n@version(\"0.4.0\")\n\n",
            "contract Guest {\n    has Str $.name;\n    has Str $.phone;\n}\n\n",
            "contract GuestForm {\n    has Str $.name;\n    has Str $.phone;\n}\n\n",
            "resource Guests for Guest {\n    query list;\n    mutation create;\n}\n\n",
            "component GuestForm {\n    has state Str $.name = \"\";\n",
            "    has state Str $.phone = \"\";\n\n",
            "    method render() {\n        ui::stack(\n",
            "            ui::text_input(:field(name), :label(\"Name\"))\n        )\n    }\n\n",
            "    method on_submit() {\n",
            "        Guests.create(Guest.new(:name($.name), :phone($.phone)));\n",
            "        $.name = \"\";\n    }\n}\n\n",
            "app HotelApp {\n    route \"/\" => GuestForm;\n}\n"
        );
        let error = check_source(program, None).expect_err("collision must be rejected");
        assert!(error.contains("duplicate"), "unexpected error: {error}");
        let (fixed, what) = autofix(program, &error).expect("collision should autofix");
        assert!(what.contains("GuestForm"), "got: {what}");
        assert!(
            !fixed.contains("contract GuestForm"),
            "redundant contract should be gone:\n{fixed}"
        );
        assert!(fixed.contains("component GuestForm"), "component must stay");
        assert!(
            check_source(&fixed, None).is_ok(),
            "fixed program must compile:\n{fixed}\n{:?}",
            check_source(&fixed, None)
        );
    }

    #[test]
    fn autofix_renames_non_component_collider_it_cannot_delete() {
        let program = concat!(
            "#!/usr/bin/env silc\n@version(\"0.4.0\")\n\n",
            "contract Guest {\n    has Str $.name;\n}\n\n",
            "processor GuestForm {\n    method score(Guest $g) {\n",
            "        $g.name ==> text::score()\n    }\n}\n\n",
            "component GuestForm {\n    has state Str $.name = \"\";\n\n",
            "    method render() {\n        ui::stack(\n",
            "            ui::text_input(:field(name), :label(\"Name\"))\n        )\n    }\n}\n\n",
            "app HotelApp {\n    route \"/\" => GuestForm;\n}\n"
        );
        let error = check_source(program, None).expect_err("collision must be rejected");
        let (fixed, _) = autofix(program, &error).expect("collision should autofix");
        assert!(
            fixed.contains("component GuestForm"),
            "component keeps the name"
        );
        assert!(
            fixed.contains("processor GuestForms"),
            "processor should be renamed:\n{fixed}"
        );
    }

    #[test]
    fn repair_guidance_explains_read_only_query_bindings() {
        let guidance = repair_guidance(
            "validate: component `GuestForm` may only assign reactive state; `guests` is not",
        )
        .expect("state assignment guidance");
        assert!(guidance.contains("read-only"));
        assert!(guidance.contains("query $.rows"));
    }

    #[test]
    fn task_pattern_teaches_ledger_page_shape() {
        let target = concat!(
            "resource Guests for Guest { query list; mutation create; }\n",
            "component GuestForm { method render() { ui::stack() } }\n",
            "app HotelApp { route \"/\" => GuestForm; }\n"
        );
        let pattern = task_pattern(
            "add another page where I can see the ledger of notes being added",
            Some(target),
        )
        .expect("ledger page task needs the list pattern");
        assert!(pattern.contains("query $.guests = Guests.list()"));
        assert!(pattern.contains("ui::table"));
        assert!(pattern.contains("route \"/ledger\""));
        assert!(
            !pattern.contains("Add EXACTLY ONE resource"),
            "target already has a resource — do not re-teach storage"
        );
    }

    #[test]
    fn task_pattern_teaches_doc_upload_extract_shape() {
        let pattern = task_pattern(
            "build a data extractor that uploads PDF and docx files then shows extracted documents",
            None,
        )
        .expect("upload task needs the doc::extract pattern");
        assert!(pattern.contains("ui::file_input"));
        assert!(pattern.contains("doc::extract(:into(Document))"));
        assert!(pattern.contains("POST /upload") || pattern.contains("/upload"));
        assert!(pattern.contains("resource Documents for Document"));
        assert!(pattern.contains("route \"/documents\""));
    }

    #[test]
    fn task_pattern_teaches_game_scene_shape() {
        let pattern = task_pattern(
            "build a webgpu fps arena with prefabs weapons and zones",
            None,
        )
        .expect("game task needs the game::scene pattern");
        assert!(pattern.contains("game::scene"));
        assert!(pattern.contains("game::* catalog"));
        assert!(pattern.contains("game::prefab") || pattern.contains("prefab"));
        assert!(pattern.contains("weapon") || pattern.contains("game::weapon"));
        assert!(pattern.contains("zone") || pattern.contains("game::zone"));
        assert!(pattern.contains("mode") && pattern.contains("pawn"));
        assert!(pattern.contains("first_person") || pattern.contains("controller"));
        assert!(pattern.contains("game::overlay") || pattern.contains("overlay"));
        assert!(!pattern.contains("app HotelApp"));
    }

    #[test]
    fn select_context_injects_game_catalog_for_game_tasks() {
        let corpus = Corpus::builtin();
        let ctx = select_context(
            "improve this webgpu fps arena with weapons",
            &corpus,
            Some("game Demo { game::scene(:title(\"Demo\"), game::overlay(:toggle(\"F1\"))) }"),
        );
        let catalog = ctx.game_catalog.expect("game task needs catalog");
        assert!(catalog.contains("game::prefab"));
        assert!(catalog.contains("game::weapon"));
        assert!(catalog.contains("game::zone"));
        assert!(catalog.contains("game::mind") || catalog.contains("game::npc"));
        assert!(
            ctx.examples
                .iter()
                .any(|(_, body)| body.contains("game::scene")),
            "should prefer a game example"
        );
    }

    #[test]
    fn repair_guidance_covers_unknown_game_prop() {
        let guidance = repair_guidance("validate: unknown prop `:foo` on game::prefab")
            .expect("game prop errors need catalog guidance");
        assert!(guidance.contains("GAME CATALOG RULE"));
        assert!(guidance.contains("game::prefab"));
    }

    #[test]
    fn game_modify_rejects_silent_intent_removal() {
        let seed = concat!(
            "game Demo { game::scene(:title(\"Demo\"), ",
            "game::zone(:name(\"Arena\"), :kind(room)), ",
            "game::camera(:mode(first_person)), ",
            "game::controller(:scheme(wasd_mouse)), ",
            "game::weapon(:name(\"Rifle\"), :fire_mode(hitscan), :damage(25)), ",
            "game::post_process(:stage(taa)), ",
            "game::post_process(:stage(sharpen))",
            ") }"
        );
        let candidate = concat!(
            "game Demo { game::scene(:title(\"Demo\"), ",
            "game::post_process(:stage(taa)), ",
            "game::weapon(:name(\"Rifle\"), :fire_mode(hitscan), :damage(25))",
            ") }"
        );
        let error = game_intent_regression(seed, candidate, "make the scene more beautiful")
            .expect("silent node removal must be rejected");
        assert!(error.contains("game::zone (0/1)"), "{error}");
        assert!(error.contains("game::camera (0/1)"), "{error}");
        assert!(error.contains("game::controller (0/1)"), "{error}");
        assert!(error.contains("game::post_process (1/2)"), "{error}");
    }

    #[test]
    fn game_modify_allows_additive_and_explicit_destructive_edits() {
        let seed = "game Demo { game::scene(:title(\"Demo\"), game::camera(:mode(first_person))) }";
        let additive = concat!(
            "game Demo { game::scene(:title(\"Demo\"), ",
            "game::camera(:mode(first_person)), game::controller(:scheme(wasd_mouse)), ",
            "game::weapon(:name(\"Sidearm\"), :fire_mode(hitscan), :damage(15))) }"
        );
        assert!(game_intent_regression(seed, additive, "add controller and sidearm").is_none());

        let destructive = "game Demo { game::scene(:title(\"Demo\")) }";
        assert!(
            game_intent_regression(seed, destructive, "remove the camera").is_none(),
            "explicit destructive task should be allowed"
        );
    }

    #[test]
    fn autofix_drops_any_contract_colliding_with_a_component() {
        let program = concat!(
            "#!/usr/bin/env silc\n@version(\"0.4.0\")\n\n",
            "contract Guest {\n    has Str $.name;\n}\n\n",
            "contract GuestLedger {\n    has Guest $.guest;\n}\n\n",
            "resource Guests for Guest {\n    query list;\n    mutation create;\n}\n\n",
            "component GuestLedger {\n    query $.guests = Guests.list();\n\n",
            "    method render() {\n        ui::stack(\n",
            "            ui::table(:rows($.guests), :columns([\"name\"]))\n        )\n    }\n}\n\n",
            "app HotelApp {\n    route \"/\" => GuestLedger;\n}\n"
        );
        let error = check_source(program, None).expect_err("collision must be rejected");
        let (fixed, what) = autofix(program, &error).expect("collision should autofix");
        assert!(what.contains("GuestLedger"), "got: {what}");
        assert!(!fixed.contains("contract GuestLedger"));
        assert!(fixed.contains("component GuestLedger"));
        assert!(
            check_source(&fixed, None).is_ok(),
            "fixed program must compile:\n{fixed}\n{:?}",
            check_source(&fixed, None)
        );
    }

    #[test]
    fn task_pattern_teaches_resource_when_storage_is_requested() {
        let target = "component GuestForm {\n    method on_submit() { submit(); }\n}\n";
        let pattern = task_pattern(
            "fix this, the submit button doesn't actually store the data",
            Some(target),
        )
        .expect("a storage task without a resource needs the pattern");
        assert!(pattern.contains("resource Guests for Guest"));
        assert!(pattern.contains("Guests.create(Guest.new("));
        assert!(
            pattern.contains("processor"),
            "must explain why not a processor"
        );
    }

    #[test]
    fn task_pattern_stays_quiet_when_target_already_persists() {
        let target = "resource Guests for Guest {\n    mutation create;\n}\n";
        assert!(task_pattern("make submit store the data", Some(target)).is_none());
    }

    #[test]
    fn task_pattern_stays_quiet_for_unrelated_tasks() {
        assert!(task_pattern("change the heading colour", Some("component X {}")).is_none());
    }

    #[test]
    fn repair_guidance_explains_pipeline_misuse() {
        let guidance = repair_guidance("parse: 55:1: unrecognized pipeline step near `$`")
            .expect("pipeline errors should have guidance");
        assert!(guidance.contains("==>"));
        assert!(guidance.contains("$.field = "), "must show assignment form");
    }

    #[test]
    fn draft_token_budget_scales_with_target_and_respects_bounds() {
        // Small form target: well under the ceiling.
        let small = draft_token_budget(Some(&"x".repeat(1_500)), 4_096);
        assert!(small >= MIN_DRAFT_TOKENS && small < 4_096, "got {small}");
        // No target still gets a usable floor.
        assert_eq!(draft_token_budget(None, 4_096), MIN_DRAFT_TOKENS);
        // Large target saturates at the ceiling.
        assert_eq!(draft_token_budget(Some(&"x".repeat(40_000)), 4_096), 4_096);
        // A ceiling below the floor still wins.
        assert_eq!(draft_token_budget(None, 800), 800);
    }

    #[test]
    fn draft_token_budget_uses_ceiling_for_game_trees() {
        let game = "@version(\"0.4.0\")\ngame Demo { game::scene(:title(\"Demo\")) }\n";
        assert_eq!(draft_token_budget(Some(game), 4_096), 8_192);
        assert_eq!(draft_token_budget(Some(game), 16_384), 16_384);
    }

    #[test]
    fn inject_named_fps_weapons_adds_closed_loadout() {
        let seed = r##"#!/usr/bin/env silc
@version("0.4.0")
game Arena {
    game::scene(
        :title("ARENA"),
        :renderer(webgpu),
        game::prefab(:name("Player"), game::mesh(:shape(capsule), :size(1.8), :color("#fff")), game::collider(:shape(capsule), :size(1.8)), game::movement(:style(first_person), :speed(5)), game::pawn()),
        game::spawn(:prefab("Player"), :x(0), :y(1), :z(0), :as_pawn),
        game::mode(:id("arena"), :possess("Player")),
        game::controller(:scheme(wasd_mouse)),
        game::camera(:mode(first_person), :follow(pawn))
    )
}
"##;
        let task = "Add VanguardAR Breach12 ArcCarbine LongshotRailgun weapons for MEGASTRUCTURE";
        let fixed = inject_named_fps_weapons(task, seed).expect("inject");
        assert!(fixed.contains("VanguardAR"));
        assert!(fixed.contains("Breach12"));
        assert!(fixed.contains("ArcCarbine"));
        assert!(fixed.contains("LongshotRailgun"));
        assert!(fixed.contains(":title(\"MEGASTRUCTURE\")"));
        assert!(check_source(&fixed, None).is_ok(), "{fixed}");
    }

    #[test]
    fn ensure_version_inserts_pragma_after_shebang() {
        let program = "#!/usr/bin/env silc\ncontract A { has Str $.x; }\n";
        let fixed = ensure_version(program).expect("missing pragma should be inserted");
        let lines: Vec<&str> = fixed.lines().collect();
        assert_eq!(lines[0], "#!/usr/bin/env silc");
        assert_eq!(lines[1], "@version(\"0.4.0\")");
    }

    #[test]
    fn ensure_version_leaves_valid_and_non_silc_input_alone() {
        assert!(ensure_version("@version(\"0.4.0\")\ncomponent A {}").is_none());
        assert!(ensure_version("Sure! Here is how you do it:").is_none());
    }

    #[test]
    fn autofix_avoids_double_plural_names() {
        let program = concat!(
            "#!/usr/bin/env silc\n@version(\"0.4.0\")\n",
            "contract Guest { has Str $.name; }\n",
            "component Visitors { method render() { ui::stack() } }\n",
            "resource Visitors for Guest {\n    query list;\n}\n"
        );
        let (fixed, _) = autofix(program, "validate: duplicate resource name `Visitors`")
            .expect("collision should autofix");
        assert!(fixed.contains("resource VisitorsStore for Guest"));
        assert!(!fixed.contains("Visitorss"), "must not double-pluralise");
    }

    #[test]
    fn repair_guidance_explains_contract_only_holds_fields() {
        let guidance = repair_guidance("parse: 8:5: expected `has` or `}` in contract")
            .expect("contract parse error should have guidance");
        assert!(guidance.contains("has Type $.field"));
        assert!(guidance.to_lowercase().contains("component"));
    }

    #[test]
    fn error_site_quotes_the_rejected_line() {
        let program = "a\nb\nc\nd\ne\nf\ng\n    method greet() {\n";
        let site = error_site(program, "parse: 8:5: expected `has` or `}` in contract")
            .expect("positioned diagnostic should render a site");
        assert!(site.contains("method greet()"), "got:\n{site}");
        assert!(site.contains("^ here"), "got:\n{site}");
        assert!(site.contains("   8 |"), "got:\n{site}");
    }

    #[test]
    fn error_site_skips_unpositioned_diagnostics() {
        assert!(error_site("x\n", "validate: duplicate resource name `X`").is_none());
    }

    #[test]
    fn autofix_declines_unrelated_errors() {
        let program = "#!/usr/bin/env silc\n@version(\"0.4.0\")\n";
        assert!(autofix(program, "parse: unexpected token `}`").is_none());
    }

    #[test]
    fn condense_rules_keeps_seed_id_guidance() {
        let corpus = Corpus::builtin();
        let src = corpus.get("agents").unwrap();
        let rules = condense_rules(src, RULES_DIGEST);
        assert!(
            rules.to_lowercase().contains(":id") || rules.to_lowercase().contains("seed"),
            "condensed rules must mention seed/:id"
        );
    }

    #[test]
    fn author_happy_path_accepts_valid_program() {
        let corpus = Corpus::builtin();
        let source = corpus.get("fixture/scored_form.silc").unwrap().to_string();
        let mut completer = ScriptedCompleter::new([source.clone()]);
        let budgets = Budgets {
            max_draft_attempts: 2,
            ..Budgets::default()
        };
        let mut state = ToolState::default();
        let result = run_author(
            "make a feedback form",
            &corpus,
            &mut completer,
            &budgets,
            &mut None,
            &mut state,
        )
        .unwrap();
        let result = result.expect("should accept fixture");
        assert!(result.finalized);
        assert!(result.program.contains("@version"));
        assert!(state.last_check_ok);
    }

    #[test]
    fn author_rejects_unchanged_seed_then_accepts_edit() {
        let corpus = Corpus::builtin();
        let source = corpus.get("fixture/scored_form.silc").unwrap().to_string();
        let edited = source.replace("Share feedback", "Hotel ledger");
        let mut completer = ScriptedCompleter::new([source.clone(), edited.clone()]);
        let budgets = Budgets {
            max_draft_attempts: 3,
            ..Budgets::default()
        };
        let mut state = ToolState {
            draft: source.clone(),
            seed: source,
            last_check_ok: false,
            stats: Default::default(),
        };
        let result = run_author(
            "hotel signup",
            &corpus,
            &mut completer,
            &budgets,
            &mut None,
            &mut state,
        )
        .unwrap()
        .expect("should accept edited program");
        assert!(result.program.contains("Hotel ledger"));
    }

    #[test]
    fn author_falls_through_when_all_attempts_fail() {
        let corpus = Corpus::builtin();
        let mut completer = ScriptedCompleter::new(["not a program", "still nonsense"]);
        let budgets = Budgets {
            max_draft_attempts: 2,
            ..Budgets::default()
        };
        let mut state = ToolState::default();
        let (result, failure) = run_author_with_failure(
            "anything",
            &corpus,
            &mut completer,
            &budgets,
            &mut None,
            &mut state,
        )
        .unwrap();
        assert!(result.is_none());
        assert!(failure.last_error.is_some());
    }

    #[test]
    fn modify_prompt_prefers_adapting_target() {
        let ctx = AuthorContext {
            rules: String::new(),
            examples: vec![],
            target: Some("@version(\"0.4.0\")\n".into()),
            target_is_starter: false,
            game_catalog: None,
        };
        let prompt = build_user_prompt("hotel form", &ctx, None, None, &[], true);
        assert!(prompt.contains("Prefer extending the existing target"));
        assert!(prompt.contains(":id"));
        assert!(prompt.contains("# END"));
    }

    #[test]
    fn modify_prompt_uses_game_preservation_not_ui_guidance() {
        let ctx = AuthorContext {
            rules: String::new(),
            examples: vec![],
            target: Some(
                "game Arena { game::scene(:title(\"ARENA\"), game::overlay(:toggle(\"F1\"))) }"
                    .into(),
            ),
            target_is_starter: false,
            game_catalog: Some("game::prefab".into()),
        };
        let prompt = build_user_prompt("brighten the ground", &ctx, None, None, &[], true);
        assert!(prompt.contains("GAME PRESERVATION RULE"));
        assert!(prompt.contains("game::scene"));
        assert!(!prompt.contains("Prefer extending the existing target structure (contract"));
        assert!(!prompt.contains("ui::app_bar"));
    }

    #[test]
    fn create_prompt_frames_game_starter_as_arena_skeleton() {
        let ctx = AuthorContext {
            rules: String::new(),
            examples: vec![],
            target: Some(
                "game Arena { game::scene(:title(\"ARENA\"), game::prefab(:name(\"Player\"))) }"
                    .into(),
            ),
            target_is_starter: true,
            game_catalog: Some("game::prefab".into()),
        };
        let prompt = build_user_prompt("arena duel", &ctx, None, None, &[], false);
        assert!(prompt.contains("arena game skeleton") || prompt.contains("FPS game skeleton"));
        assert!(prompt.contains("weapon") || prompt.contains("first_person"));
        assert!(prompt.contains("mode") && prompt.contains("pawn"));
        assert!(!prompt.contains("submit();"));
    }

    #[test]
    fn create_prompt_frames_starter_as_skeleton_to_adapt() {
        let ctx = AuthorContext {
            rules: String::new(),
            examples: vec![],
            target: Some("@version(\"0.4.0\")\n".into()),
            target_is_starter: true,
            game_catalog: None,
        };
        let prompt = build_user_prompt("greeting page", &ctx, None, None, &[], false);
        assert!(prompt.contains("Starting skeleton"));
        assert!(prompt.contains("Start from the skeleton above"));
        assert!(prompt.contains("submit();"), "must pin the submit handler");
        assert!(prompt.contains("SIBLING"), "must forbid nested methods");
    }

    #[test]
    fn autofix_reencloses_orphaned_game_siblings() {
        let broken = r##"#!/usr/bin/env silc
@version("0.4.0")
game Mega {
    game::scene(
        :title("MEGA"),
        :renderer(webgpu),
        game::entity(:name("Ground"), game::mesh(:shape(plane), :size(10), :color("#333")))
    )
}
game::spawn(:prefab("Player"), :x(0), :y(1), :z(0), :as_pawn),
game::mode(:id("m"), :possess("Player")),
game::camera(:mode(first_person), :follow(pawn))
"##;
        let (fixed, note) = autofix(broken, "parse: 10:1: expected game name").expect("autofix");
        assert!(note.contains("orphaned"), "{note}");
        assert!(fixed.contains("game::spawn"));
        assert!(fixed.contains("game::mode"));
        assert!(fixed.contains("game::camera"));
        assert!(fixed.contains("game Mega {"));
        assert!(fixed.trim_end().ends_with('}'));
        assert!(fixed.contains("game::scene("));
    }
}
