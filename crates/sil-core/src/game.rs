//! Game subject: real-time WebGPU scene trees (ADR-012).
//!
//! Authors declare `game Name { game::scene(...) }`. The catalog synthesizes
//! Godot-style trees/signals, Unity-style prefabs/data/components, and
//! Unreal-style mode/pawn/controller ownership. Compiler-owned Babylon/WebGPU
//! templates implement the nodes. Distinct from dual-surface `ui::*` / `app`.

use crate::expr::Expr;
use crate::types::Span;

/// Render / runtime surface for game programs (web-only, WebGPU).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameSurface {
    WebGpu,
}

impl GameSurface {
    pub fn as_str(self) -> &'static str {
        match self {
            GameSurface::WebGpu => "webgpu",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePropKind {
    String,
    Bool,
    Ident,
    Number,
    Flag,
    Expr,
    Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GamePropSpec {
    pub name: &'static str,
    pub kind: GamePropKind,
    pub required: bool,
    /// One or two sentences teaching what the prop controls.
    pub description: &'static str,
    /// Closed ident tokens when `kind` is Ident (empty = open / unvalidated).
    pub closed_values: &'static [&'static str],
}

const fn gp(
    name: &'static str,
    kind: GamePropKind,
    required: bool,
    description: &'static str,
) -> GamePropSpec {
    GamePropSpec {
        name,
        kind,
        required,
        description,
        closed_values: &[],
    }
}

const fn gp_closed(
    name: &'static str,
    kind: GamePropKind,
    required: bool,
    description: &'static str,
    closed_values: &'static [&'static str],
) -> GamePropSpec {
    GamePropSpec {
        name,
        kind,
        required,
        description,
        closed_values,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameChildPolicy {
    None,
    AnyOf(&'static [&'static str]),
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameNodeSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub props: &'static [GamePropSpec],
    pub children: GameChildPolicy,
}

const SCENE_CHILDREN: &[&str] = &[
    "entity",
    "prefab",
    "spawn",
    "data",
    "asset",
    "material",
    "mode",
    "controller",
    "camera",
    "post_process",
    "overlay",
    "hud",
    "environment",
    "shadow",
    "zone",
    "weapon",
    "encounter",
    "objective",
    "signal",
    "group",
];

const ENTITY_CHILDREN: &[&str] = &[
    "entity",
    "mesh",
    "light",
    "collider",
    "movement",
    "attribute",
    "pawn",
    "ability",
    "weapon",
    "ammo",
    "damage",
    "pickup",
    "npc",
    "perception",
    "behavior",
    "mind",
    "nav_agent",
    "door",
    "trigger",
    "cover",
    "audio",
    "signal",
    "group",
    "spawn",
];

const PREFAB_CHILDREN: &[&str] = ENTITY_CHILDREN;

const ABILITY_CHILDREN: &[&str] = &["particle_emitter", "dynamic_light", "camera_impulse", "audio"];

const WEAPON_CHILDREN: &[&str] = &[
    "projectile",
    "particle_emitter",
    "dynamic_light",
    "camera_impulse",
    "audio",
];

const ZONE_CHILDREN: &[&str] = &["entity", "spawn", "light", "signal", "group"];

const ENCOUNTER_CHILDREN: &[&str] = &["spawn"];

const MODE_CHILDREN: &[&str] = &["spawn", "encounter", "objective"];

pub const GAME_NODE_CATALOG: &[GameNodeSpec] = &[
    GameNodeSpec {
        name: "scene",
        description: "Root WebGPU game scene (Godot main scene). Hosts the entity tree, prefabs, data assets, mode, controller, camera, and post-process.",
        props: &[
            gp("title", GamePropKind::String, true, "Window / overlay title shown for the game program. Prefer a short product name."),
            gp_closed("renderer", GamePropKind::Ident, false, "Graphics backend token. Only `webgpu` is legal in Silc 0.4.0.", &["webgpu"]),
            gp("target_fps", GamePropKind::Number, false, "Preferred frame rate for the render loop (for example `90`). The runtime caps to display capability."),
        ],
        children: GameChildPolicy::AnyOf(SCENE_CHILDREN),
    },
    GameNodeSpec {
        name: "entity",
        description: "Named node in the scene tree (Godot-style). Parent/child links own transform hierarchy; attach component children such as mesh, collider, or ability.",
        props: &[
            gp("name", GamePropKind::String, true, "Unique name within the parent scope used for find, signals, and spawn targets."),
            gp("x", GamePropKind::Number, false, "Local X translation in meters."),
            gp("y", GamePropKind::Number, false, "Local Y translation in meters."),
            gp("z", GamePropKind::Number, false, "Local Z translation in meters."),
            gp("yaw", GamePropKind::Number, false, "Local yaw rotation in degrees around the Y axis."),
            gp("pitch", GamePropKind::Number, false, "Local pitch rotation in degrees around the X axis."),
            gp("roll", GamePropKind::Number, false, "Local roll rotation in degrees around the Z axis."),
            gp("sx", GamePropKind::Number, false, "Local X scale multiplier applied to mesh children."),
            gp("sy", GamePropKind::Number, false, "Local Y scale multiplier applied to mesh children."),
            gp("sz", GamePropKind::Number, false, "Local Z scale multiplier applied to mesh children."),
        ],
        children: GameChildPolicy::AnyOf(ENTITY_CHILDREN),
    },
    GameNodeSpec {
        name: "prefab",
        description: "Named reusable entity template (Unity prefab / Godot packed scene). Instantiate with `game::spawn` and optional property overrides.",
        props: &[
            gp("name", GamePropKind::String, true, "Prefab identity used by `game::spawn :prefab(...)`."),
            gp("x", GamePropKind::Number, false, "Default root X translation in meters when instantiated."),
            gp("y", GamePropKind::Number, false, "Default root Y translation in meters when instantiated."),
            gp("z", GamePropKind::Number, false, "Default root Z translation in meters when instantiated."),
            gp("yaw", GamePropKind::Number, false, "Default root yaw rotation in degrees around the Y axis."),
            gp("pitch", GamePropKind::Number, false, "Default root pitch rotation in degrees around the X axis."),
            gp("roll", GamePropKind::Number, false, "Default root roll rotation in degrees around the Z axis."),
            gp("sx", GamePropKind::Number, false, "Default root X scale multiplier for mesh children."),
            gp("sy", GamePropKind::Number, false, "Default root Y scale multiplier for mesh children."),
            gp("sz", GamePropKind::Number, false, "Default root Z scale multiplier for mesh children."),
        ],
        children: GameChildPolicy::AnyOf(PREFAB_CHILDREN),
    },
    GameNodeSpec {
        name: "spawn",
        description: "Instantiate a prefab into the scene (Unity instantiate). Supports transform overrides and component `:ref` data bindings at spawn time.",
        props: &[
            gp("prefab", GamePropKind::String, true, "Name of the `game::prefab` to instantiate."),
            gp("x", GamePropKind::Number, false, "World X override for the spawned root in meters."),
            gp("y", GamePropKind::Number, false, "World Y override for the spawned root in meters."),
            gp("z", GamePropKind::Number, false, "World Z override for the spawned root in meters."),
            gp("as_pawn", GamePropKind::Flag, false, "When set, marks this spawn as the possessable pawn for the active mode."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "data",
        description: "Named data asset (Unity ScriptableObject-like). Shared tuneables referenced by components via `:ref(Name)`.",
        props: &[
            gp("name", GamePropKind::String, true, "Data asset identity used by `:ref` on movement, weapons, and AI components."),
            gp("speed", GamePropKind::Number, false, "Optional movement speed (m/s) when this asset is a locomotion profile."),
            gp("cooldown", GamePropKind::Number, false, "Optional ability cooldown seconds when used as an ability profile."),
            gp("cost", GamePropKind::Number, false, "Optional attribute cost when used as an ability profile."),
            gp("damage", GamePropKind::Number, false, "Optional base damage per hit when used as a weapon or projectile profile."),
            gp("range", GamePropKind::Number, false, "Optional effective range in meters for hitscan and projectile weapons."),
            gp("fire_rate", GamePropKind::Number, false, "Optional shots per second for automatic or semi-auto weapons."),
            gp("magazine", GamePropKind::Number, false, "Optional rounds per magazine before reload is required."),
            gp("reload", GamePropKind::Number, false, "Optional reload duration in seconds to refill the magazine."),
            gp("spread", GamePropKind::Number, false, "Optional aim cone spread in degrees for pellet or hitscan weapons."),
            gp("pellet_count", GamePropKind::Number, false, "Optional number of pellets fired per shot for shotgun-style weapons."),
            gp("charge_time", GamePropKind::Number, false, "Optional seconds to fully charge beam or rail weapons before firing."),
            gp("splash_radius", GamePropKind::Number, false, "Optional splash damage radius in meters for explosive projectiles."),
            gp("cadence_s", GamePropKind::Number, false, "Optional AI decision cadence in seconds between tactic re-evaluations."),
            gp("persona", GamePropKind::String, false, "Optional AI persona label referenced by `game::mind :ref` for behavior flavor."),
            gp("aggression", GamePropKind::Number, false, "Optional AI aggression bias (0–1) weighting push and flank tactics."),
            gp("morale", GamePropKind::Number, false, "Optional AI morale baseline (0–1) influencing retreat thresholds."),
            gp("health", GamePropKind::Number, false, "Optional max health when used as a character or NPC profile."),
            gp("armor", GamePropKind::Number, false, "Optional armor rating subtracted from incoming damage."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "signal",
        description: "Declared event on an entity (Godot signal). Wire listeners with `:on(signal_name => handler)` on the receiving node.",
        props: &[
            gp("name", GamePropKind::Ident, true, "Signal identity token (for example `ability_cast` or `landed`)."),
            gp("on", GamePropKind::Expr, false, "Optional connection expression wiring this signal to a handler id."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "group",
        description: "Membership tag for entity queries (Godot groups). Systems find entities with `world.group(\"enemies\")`.",
        props: &[
            gp("name", GamePropKind::String, true, "Group name string shared by all members."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "mesh",
        description: "Renderable mesh component (Unity MeshFilter-like). Use a closed primitive `:shape` or a GLTF `:asset`; one is required at runtime.",
        props: &[
            gp_closed("shape", GamePropKind::Ident, false, "Closed primitive mesh shape. Omit when `:asset` supplies a GLTF model.", &["plane", "box", "capsule", "sphere"]),
            gp("asset", GamePropKind::String, false, "Optional `game::asset` name; when set the runtime loads GLTF instead of a primitive shape."),
            gp("material", GamePropKind::String, false, "Optional `game::material` name applied to the mesh surface."),
            gp("size", GamePropKind::Number, false, "Uniform scale / size in meters for the primitive."),
            gp("color", GamePropKind::String, false, "CSS-like hex color for the unlit/albedo tint (for example `\"#4a7c59\"`)."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "light",
        description: "Light component attached to an entity (directional sun, point lamp, or spot cone).",
        props: &[
            gp_closed("kind", GamePropKind::Ident, true, "Closed light kind selecting directional sun, omnidirectional point, or spot cone.", &["directional", "point", "spot"]),
            gp("intensity", GamePropKind::Number, false, "Light intensity multiplier."),
            gp("color", GamePropKind::String, false, "CSS-like hex color for the light."),
            gp("radius_m", GamePropKind::Number, false, "Point-light radius in meters (ignored for directional)."),
            gp("cast_shadows", GamePropKind::Bool, false, "When true, this light casts shadow maps for nearby receivers."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "collider",
        description: "Thin physics collider used by the kernel physics system (ground planes and capsule bodies).",
        props: &[
            gp_closed("shape", GamePropKind::Ident, true, "Closed collider shape matching the gameplay body.", &["box", "capsule", "plane"]),
            gp("size", GamePropKind::Number, false, "Collider half-extent or radius scale in meters."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "movement",
        description: "Locomotion component for walk, first-person, sprint, and jump styles driven by the possessed controller.",
        props: &[
            gp_closed("style", GamePropKind::Ident, false, "Closed locomotion style token.", &["walk", "first_person", "sprint", "jump"]),
            gp("speed", GamePropKind::Number, false, "Move speed in meters per second when no `:ref` data asset is bound."),
            gp("jump_speed", GamePropKind::Number, false, "Initial upward velocity in m/s applied when the jump input is pressed."),
            gp("sprint_mul", GamePropKind::Number, false, "Speed multiplier applied while sprint is held (for example `1.5`)."),
            gp("ref", GamePropKind::String, false, "Optional `game::data` asset name providing speed and related tuneables."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "attribute",
        description: "Named float channel on an entity (health, stamina) used by abilities for cost and GAS-lite state.",
        props: &[
            gp("name", GamePropKind::String, true, "Attribute channel name (for example `\"stamina\"`)."),
            gp("value", GamePropKind::Number, false, "Initial attribute value."),
            gp("max", GamePropKind::Number, false, "Optional maximum clamp for the attribute."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "mode",
        description: "Unreal-style game mode: owns default spawns and which pawn the controller possesses.",
        props: &[
            gp("id", GamePropKind::String, true, "Mode identity string (for example `\"arena\"`)."),
            gp("possess", GamePropKind::String, false, "Prefab or entity name the controller should possess at start."),
        ],
        children: GameChildPolicy::AnyOf(MODE_CHILDREN),
    },
    GameNodeSpec {
        name: "pawn",
        description: "Marks an entity or prefab as a possessable Unreal-style pawn body for the active controller.",
        props: &[],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "controller",
        description: "Input owner that drives the possessed pawn (WASD + mouse). Does not move meshes directly.",
        props: &[
            gp_closed("scheme", GamePropKind::Ident, false, "Closed input scheme token.", &["wasd_mouse"]),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "camera",
        description: "Follow or first-person camera bound to the possessed pawn.",
        props: &[
            gp_closed("mode", GamePropKind::Ident, false, "Camera rig mode token.", &["third_person", "first_person"]),
            gp("distance_m", GamePropKind::Number, false, "Follow distance from the pawn pivot in meters (third-person only)."),
            gp("shoulder_offset_m", GamePropKind::Number, false, "Lateral shoulder offset in meters for over-the-shoulder framing."),
            gp("follow", GamePropKind::Ident, false, "Follow target token; use `pawn` to track the possessed body."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "ability",
        description: "Keyed player ability with optional cooldown/cost and cue children (particles, lights, camera impulse).",
        props: &[
            gp("name", GamePropKind::String, true, "Human-readable ability label shown in HUD / assist chrome."),
            gp("key", GamePropKind::String, true, "Keyboard binding that casts the ability (for example `\"1\"`). Keys must be unique in the scene."),
            gp("cooldown", GamePropKind::Number, false, "Seconds before the ability can cast again."),
            gp("cost", GamePropKind::Number, false, "Attribute cost deducted on cast when `:cost_attr` is set."),
            gp("cost_attr", GamePropKind::String, false, "Attribute name consumed by `:cost` (for example `\"stamina\"`)."),
            gp("ref", GamePropKind::String, false, "Optional `game::data` asset supplying cooldown/cost defaults."),
        ],
        children: GameChildPolicy::AnyOf(ABILITY_CHILDREN),
    },
    GameNodeSpec {
        name: "particle_emitter",
        description: "Pooled GPU particle burst cue fired by an ability cast.",
        props: &[
            gp_closed("kind", GamePropKind::Ident, true, "Closed particle preset.", &["burst", "spark", "smoke"]),
            gp("count", GamePropKind::Number, false, "Particle budget for this emitter; larger counts look denser and cost more GPU."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "dynamic_light",
        description: "Tight-radius dynamic light cue for ability illumination.",
        props: &[
            gp("radius_m", GamePropKind::Number, false, "Light radius in meters."),
            gp("intensity", GamePropKind::Number, false, "Light intensity multiplier."),
            gp("color", GamePropKind::String, false, "CSS-like hex color string for the light (for example `\"#a8d4ff\"`)."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "camera_impulse",
        description: "Subtle camera shake impulse fired when an ability casts, scaled by strength.",
        props: &[
            gp("strength", GamePropKind::Number, false, "Impulse magnitude; keep small (near `0.15`–`0.25`) for readable framing."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "post_process",
        description: "One post-process stage in the ordered screen chain (TAA, bloom, tonemap, …). Repeat the node once per stage.",
        props: &[
            gp_closed("stage", GamePropKind::Ident, true, "Closed post-process stage identity in the filmic chain.", &["taa", "ssao", "ssr", "dof", "bloom", "tonemap", "grain", "sharpen"]),
            gp("enabled", GamePropKind::Bool, false, "Whether this stage is active. Disable expensive stages (SSR, DOF) for performance."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "overlay",
        description: "Hidden-by-default settings and debug overlay toggled from the keyboard (F1 style). Can trigger save/load.",
        props: &[
            gp("toggle", GamePropKind::String, true, "Key binding that shows or hides the overlay (for example `\"F1\"`)."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "asset",
        description: "Named external asset reference (GLTF model, texture, audio clip, or navmesh bake) declared at scene scope.",
        props: &[
            gp("name", GamePropKind::String, true, "Asset identity referenced by `:asset` on mesh, `:path`/`ref` on audio, and similar bindings."),
            gp("path", GamePropKind::String, true, "Relative or absolute path to the asset file on disk or in the baked bundle."),
            gp_closed("kind", GamePropKind::Ident, true, "Closed asset kind selecting how the runtime loads and caches the file.", &["gltf", "texture", "audio", "navmesh"]),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "material",
        description: "Named PBR material asset with optional texture maps and tiling. Referenced by `:material` on mesh components.",
        props: &[
            gp("name", GamePropKind::String, true, "Material identity used by `:material` on mesh nodes."),
            gp("albedo", GamePropKind::String, false, "Albedo / base-color map path or hex tint (for example `\"#808080\"`)."),
            gp("normal", GamePropKind::String, false, "Normal map texture path for surface detail."),
            gp("roughness", GamePropKind::Number, false, "Roughness scalar (0 = mirror, 1 = fully diffuse)."),
            gp("metallic", GamePropKind::Number, false, "Metallic scalar (0 = dielectric, 1 = metal)."),
            gp("ao", GamePropKind::String, false, "Ambient-occlusion texture path darkening creases."),
            gp("emissive", GamePropKind::String, false, "Emissive color hex or texture path for glowing surfaces."),
            gp("tiling", GamePropKind::Number, false, "UV tiling multiplier applied to all texture maps."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "zone",
        description: "Spatial volume grouping entities, spawns, lights, and signals (room, walkway, or outdoor area).",
        props: &[
            gp("name", GamePropKind::String, true, "Zone identity used by encounter and objective targeting."),
            gp_closed("kind", GamePropKind::Ident, true, "Closed zone topology hint for lighting and AI nav hints.", &["room", "walkway", "outdoor"]),
        ],
        children: GameChildPolicy::AnyOf(ZONE_CHILDREN),
    },
    GameNodeSpec {
        name: "weapon",
        description: "Weapon definition with fire mode, ballistics tuneables, and cue children (projectile, particles, audio).",
        props: &[
            gp("name", GamePropKind::String, true, "Weapon identity referenced by `:ref` on equipped weapon components and pickups."),
            gp("slot", GamePropKind::Number, false, "Hotbar slot index (1–4) for player loadouts."),
            gp_closed("fire_mode", GamePropKind::Ident, true, "Closed fire delivery mode selecting hitscan, pellet spread, projectile, or beam.", &["hitscan", "pellet", "projectile", "beam"]),
            gp("ref", GamePropKind::String, false, "Optional `game::data` asset supplying damage, fire_rate, magazine, and spread defaults."),
            gp("damage", GamePropKind::Number, false, "Base damage per hit when no `:ref` data asset is bound."),
            gp("fire_rate", GamePropKind::Number, false, "Shots per second for automatic or semi-auto fire."),
            gp("magazine", GamePropKind::Number, false, "Rounds per magazine before reload is required."),
            gp("reload", GamePropKind::Number, false, "Reload duration in seconds to refill the magazine."),
            gp("spread", GamePropKind::Number, false, "Aim cone spread in degrees for pellet or hitscan weapons."),
        ],
        children: GameChildPolicy::AnyOf(WEAPON_CHILDREN),
    },
    GameNodeSpec {
        name: "projectile",
        description: "Projectile or tracer cue spawned by a weapon fire event with speed, lifetime, and splash tuning.",
        props: &[
            gp_closed("kind", GamePropKind::Ident, true, "Closed projectile visual and physics preset.", &["tracer", "shell", "plasma", "rail"]),
            gp("speed", GamePropKind::Number, false, "Projectile travel speed in meters per second."),
            gp("lifetime", GamePropKind::Number, false, "Maximum flight time in seconds before the projectile despawns."),
            gp("splash_radius", GamePropKind::Number, false, "Splash damage radius in meters for explosive shells."),
            gp("color", GamePropKind::String, false, "CSS-like hex color for the projectile trail or glow."),
            gp("size", GamePropKind::Number, false, "Visual size multiplier for the projectile mesh or billboard."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "ammo",
        description: "Ammo pool component tracking current and maximum rounds for a weapon slot on an entity.",
        props: &[
            gp("name", GamePropKind::String, true, "Ammo type identity matching the weapon or pickup `:ref`."),
            gp("amount", GamePropKind::Number, false, "Current round count in the pool."),
            gp("max", GamePropKind::Number, false, "Maximum rounds this pool can hold."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "damage",
        description: "Damage payload component attached to projectiles or melee hitboxes describing amount and type.",
        props: &[
            gp("amount", GamePropKind::Number, true, "Damage points applied on a successful hit."),
            gp_closed("type_ident", GamePropKind::Ident, true, "Closed damage type for armor and VFX routing.", &["bullet", "pellet", "plasma", "rail", "melee"]),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "pickup",
        description: "World pickup trigger granting weapons, ammo, or health when the player overlaps the collider.",
        props: &[
            gp_closed("kind", GamePropKind::Ident, true, "Closed pickup category selecting what the player receives.", &["weapon", "ammo", "health"]),
            gp("ref", GamePropKind::String, true, "Weapon name, ammo type, or health profile referenced by this pickup."),
            gp("amount", GamePropKind::Number, false, "Quantity granted (ammo rounds or health points; ignored for weapon pickups)."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "hud",
        description: "Heads-up display toggles for crosshair, ammo counter, and health bar rendered over the 3D view.",
        props: &[
            gp("show_crosshair", GamePropKind::Bool, false, "When true, draw a centered aim reticle for FPS weapons."),
            gp("show_ammo", GamePropKind::Bool, false, "When true, display current magazine and reserve ammo counts."),
            gp("show_health", GamePropKind::Bool, false, "When true, display the possessed pawn health bar."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "npc",
        description: "Non-player character archetype and faction tag driving AI squad roles and hostility checks.",
        props: &[
            gp_closed("archetype", GamePropKind::Ident, true, "Closed combat role selecting default tactic weights.", &["suppressor", "flanker", "breacher"]),
            gp_closed("faction", GamePropKind::Ident, true, "Closed faction token for targeting and friendly-fire rules.", &["hostile", "neutral"]),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "perception",
        description: "Sensory range component defining how far an NPC can see and hear the player.",
        props: &[
            gp("sight_m", GamePropKind::Number, false, "Maximum sight range in meters before the target is lost."),
            gp("hear_m", GamePropKind::Number, false, "Maximum hearing range in meters for gunfire and footsteps."),
            gp("fov_deg", GamePropKind::Number, false, "Horizontal field-of-view cone in degrees for line-of-sight checks."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "behavior",
        description: "Behavior-tree selector and default combat tactic for an NPC squad member.",
        props: &[
            gp_closed("tree", GamePropKind::Ident, true, "Closed behavior tree preset for patrol and combat transitions.", &["patrol_combat", "guard"]),
            gp_closed("default_tactic", GamePropKind::Ident, false, "Closed default combat tactic when engaged.", &["suppress", "flank", "push", "retreat"]),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "mind",
        description: "AI mind component referencing a persona data asset and decision cadence for tactical re-evaluation.",
        props: &[
            gp("ref", GamePropKind::String, true, "Name of a `game::data` asset supplying persona, aggression, and morale tuneables."),
            gp("cadence_s", GamePropKind::Number, false, "Seconds between AI tactic re-evaluations when in combat."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "nav_agent",
        description: "Navigation agent capsule describing radius, height, and max speed for pathfinding on navmesh.",
        props: &[
            gp("radius", GamePropKind::Number, false, "Agent capsule radius in meters for navmesh clearance."),
            gp("height", GamePropKind::Number, false, "Agent capsule height in meters for doorway checks."),
            gp("max_speed", GamePropKind::Number, false, "Maximum travel speed in m/s along navmesh paths."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "encounter",
        description: "Combat encounter wave container that spawns prefabs when the mode or trigger activates it.",
        props: &[
            gp("id", GamePropKind::String, true, "Encounter identity referenced by objectives and mode scripting."),
            gp("wave", GamePropKind::Number, false, "Wave index within a multi-wave encounter sequence."),
        ],
        children: GameChildPolicy::AnyOf(ENCOUNTER_CHILDREN),
    },
    GameNodeSpec {
        name: "objective",
        description: "Mission objective node declaring win conditions such as clearing hostiles or reaching a target zone.",
        props: &[
            gp("id", GamePropKind::String, true, "Objective identity shown in HUD and mode completion checks."),
            gp_closed("kind", GamePropKind::Ident, true, "Closed objective completion rule.", &["clear_hostiles", "reach"]),
            gp("target", GamePropKind::String, false, "Zone name, encounter id, or entity name required to complete the objective."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "audio",
        description: "Spatial or UI audio source playing a one-shot or looping clip from an asset path or ref.",
        props: &[
            gp_closed("kind", GamePropKind::Ident, true, "Closed playback mode selecting one-shot or looping audio.", &["oneshot", "loop"]),
            gp("path", GamePropKind::String, false, "Direct path to an audio file when no `:ref` asset is bound."),
            gp("ref", GamePropKind::String, false, "Optional `game::asset` name of kind `audio` for baked clip lookup."),
            gp("volume", GamePropKind::Number, false, "Playback volume multiplier (0 = silent, 1 = full)."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "environment",
        description: "Scene-wide atmosphere tuning for fog, sky color, and exposure applied before the post chain.",
        props: &[
            gp("fog_density", GamePropKind::Number, false, "Exponential fog density scalar (0 = clear, higher = thicker)."),
            gp("fog_color", GamePropKind::String, false, "CSS-like hex color for distance fog."),
            gp("sky_color", GamePropKind::String, false, "CSS-like hex color for the clear-sky gradient."),
            gp("exposure", GamePropKind::Number, false, "Global exposure multiplier before tonemap (1 = neutral)."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "shadow",
        description: "Scene-wide shadow map configuration controlling cascade count and enable state.",
        props: &[
            gp("enabled", GamePropKind::Bool, false, "When false, all shadow casting is disabled for performance."),
            gp("cascade_count", GamePropKind::Number, false, "Number of cascaded shadow splits for directional sun (1–4)."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "door",
        description: "Animated door component with open/closed state and optional auto-close behavior.",
        props: &[
            gp_closed("state", GamePropKind::Ident, false, "Closed initial door state.", &["open", "closed"]),
            gp("auto", GamePropKind::Bool, false, "When true, the door opens on player proximity and closes after a delay."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "trigger",
        description: "Volume trigger firing a handler expression when the player enters or exits the collider.",
        props: &[
            gp_closed("kind", GamePropKind::Ident, true, "Closed trigger edge selecting enter or exit activation.", &["enter", "exit"]),
            gp("on", GamePropKind::Expr, true, "Handler expression or signal id invoked when the trigger fires."),
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "cover",
        description: "Cover point quality tag used by AI flank and suppress tactics to rank defensive positions.",
        props: &[
            gp_closed("quality", GamePropKind::Ident, true, "Closed cover quality rating for AI position scoring.", &["low", "med", "high"]),
        ],
        children: GameChildPolicy::None,
    },
];

pub fn lookup_game_node(name: &str) -> Option<&'static GameNodeSpec> {
    GAME_NODE_CATALOG.iter().find(|n| n.name == name)
}

/// Documentation for a prop on a catalogued `game::*` node.
pub fn game_prop_doc(node: &str, prop: &str) -> Option<&'static str> {
    lookup_game_node(node).and_then(|spec| {
        spec.props
            .iter()
            .find(|p| p.name == prop)
            .map(|p| p.description)
    })
}

/// Documentation for a closed enum / ident token used by a game prop.
pub fn game_closed_value_doc(value: &str) -> Option<&'static str> {
    Some(match value {
        "webgpu" => {
            "WebGPU renderer backend. The only legal `:renderer` token for `game::scene` in Silc 0.4.0."
        }
        "plane" => "Flat ground or wall primitive / collider.",
        "box" => "Axis-aligned box mesh or collider.",
        "capsule" => "Capsule mesh or character collider.",
        "sphere" => "Sphere mesh primitive.",
        "directional" => "Infinite directional light (sun).",
        "point" => "Omnidirectional point light with finite radius.",
        "spot" => "Spot cone light with direction and falloff angle.",
        "walk" => "Grounded walk locomotion style driven by WASD.",
        "first_person" => "First-person locomotion or camera rig bound to the pawn eye socket.",
        "sprint" => "Sprint locomotion style applying a speed multiplier while held.",
        "jump" => "Jump-capable locomotion style with vertical impulse on input.",
        "wasd_mouse" => "Keyboard WASD move plus mouse look input scheme.",
        "third_person" => "Spring-arm third-person camera following the pawn.",
        "gltf" => "GLTF 3D model asset loaded at runtime for mesh rendering.",
        "texture" => "2D texture asset for material map bindings.",
        "navmesh" => "Baked navigation mesh asset for AI pathfinding.",
        "room" => "Enclosed indoor zone with walls and a ceiling.",
        "walkway" => "Linear corridor or bridge zone connecting rooms.",
        "outdoor" => "Open-air zone with sky lighting and no ceiling.",
        "hitscan" => "Instant raycast weapon fire with no visible projectile.",
        "pellet" => "Multi-ray shotgun-style fire with spread cone.",
        "projectile" => "Spawns a visible projectile with travel time.",
        "beam" => "Continuous beam weapon requiring charge time.",
        "tracer" => "Fast visible bullet tracer projectile preset.",
        "shell" => "Explosive shell projectile with splash radius.",
        "plasma" => "Energy plasma bolt projectile preset.",
        "rail" => "High-speed rail slug projectile preset.",
        "bullet" => "Standard bullet damage type for hitscan weapons.",
        "melee" => "Melee damage type for close-range strikes.",
        "weapon" => "Pickup grants a weapon referenced by `:ref`.",
        "health" => "Pickup restores health points to the player.",
        "suppressor" => "NPC archetype favoring suppress-and-advance tactics.",
        "flanker" => "NPC archetype favoring lateral flanking routes.",
        "breacher" => "NPC archetype favoring aggressive push-and-clear.",
        "hostile" => "Faction treated as an enemy by player targeting.",
        "neutral" => "Faction ignored by auto-targeting unless provoked.",
        "patrol_combat" => "Behavior tree alternating patrol and combat states.",
        "guard" => "Behavior tree holding a fixed guard position.",
        "suppress" => "Default tactic: lay down covering fire from cover.",
        "flank" => "Default tactic: move to a lateral angle on the target.",
        "push" => "Default tactic: advance aggressively toward the target.",
        "retreat" => "Default tactic: fall back to safer cover or rally point.",
        "clear_hostiles" => "Objective completes when all hostiles in the target are eliminated.",
        "reach" => "Objective completes when the player enters the target zone.",
        "oneshot" => "Audio clip plays once and then stops.",
        "loop" => "Audio clip repeats continuously until stopped.",
        "open" => "Door starts in the open position.",
        "closed" => "Door starts in the closed position.",
        "enter" => "Trigger fires when a body enters the volume.",
        "exit" => "Trigger fires when a body exits the volume.",
        "low" => "Low-quality cover providing minimal protection.",
        "med" => "Medium-quality cover blocking most incoming fire.",
        "high" => "High-quality cover providing strong protection.",
        "pawn" => "Follow or possess the active Unreal-style pawn.",
        "burst" => "Short particle burst cue for ability impacts.",
        "spark" => "Spark particle cue for ability hits.",
        "smoke" => "Smoke particle cue for ability trails.",
        "taa" => "Temporal anti-aliasing stage that stabilizes shimmering edges across frames.",
        "ssao" => "Screen-space ambient occlusion stage that darkens creases and under-overhangs.",
        "ssr" => "Screen-space reflections stage; expensive — disable when targeting lower GPUs.",
        "dof" => "Depth-of-field blur stage that softens distant (or near) regions.",
        "bloom" => "Bloom stage that softens bright highlights into a glow.",
        "tonemap" => "Tonemap stage that maps HDR scene luminance into displayable range.",
        "grain" => "Film-grain stage that adds subtle temporal noise for texture.",
        "sharpen" => "Sharpen stage that restores edge contrast after TAA / bloom.",
        "ability_cast" | "landed" => "Common signal identity used by entities and mode wiring.",
        "audio" => "Audio clip asset for spatial or UI playback.",
        "ammo" => "Pickup grants ammo rounds referenced by `:ref`.",
        _ => return None,
    })
}

/// Look up which game node/prop (if any) claims a closed value token.
pub fn game_closed_value_owners(value: &str) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    for node in GAME_NODE_CATALOG {
        for prop in node.props {
            if prop.closed_values.contains(&value) {
                out.push((node.name, prop.name));
            }
        }
    }
    out
}

pub fn catalog_game_node_names() -> Vec<&'static str> {
    GAME_NODE_CATALOG.iter().map(|n| n.name).collect()
}

/// Markdown digest of the closed `game::*` catalog for assist / docs.
pub fn format_game_catalog_md() -> String {
    let mut out = String::from(
        "# game::* catalog (ADR-012)\n\n\
         WebGPU programs declare one `game Name { game::scene(...) }` tree. \
         Godot tree+signals, Unity prefabs/data/components, Unreal mode/pawn/controller. \
         Use only these nodes/props. No `app` / `component` / `resource` mix.\n\n",
    );
    for node in GAME_NODE_CATALOG {
        out.push_str(&format!("## game::{}\n{}\n", node.name, node.description));
        if !node.props.is_empty() {
            out.push_str("\nProps:\n");
            for prop in node.props {
                let req = if prop.required {
                    "required"
                } else {
                    "optional"
                };
                let closed = if prop.closed_values.is_empty() {
                    String::new()
                } else {
                    format!("; closed: {}", prop.closed_values.join("|"))
                };
                out.push_str(&format!(
                    "- `:{}` ({:?}, {req}{closed}): {}\n",
                    prop.name, prop.kind, prop.description
                ));
            }
        }
        match node.children {
            GameChildPolicy::None => out.push_str("\nChildren: none\n"),
            GameChildPolicy::Any => out.push_str("\nChildren: any game::* node\n"),
            GameChildPolicy::AnyOf(allowed) => {
                out.push_str("\nChildren: ");
                out.push_str(
                    &allowed
                        .iter()
                        .map(|n| format!("game::{n}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                out.push('\n');
            }
        }
        out.push('\n');
    }
    out.push_str(
        "Closed enums: `:renderer(webgpu)`; mesh/collider `:shape(plane|box|capsule|sphere)`; \
         mesh `:asset` XOR `:shape`; light `:kind(directional|point|spot)`; \
         movement `:style(walk|first_person|sprint|jump)`; controller `:scheme(wasd_mouse)`; \
         camera `:mode(third_person|first_person)`; asset `:kind(gltf|texture|audio|navmesh)`; \
         zone `:kind(room|walkway|outdoor)`; weapon `:fire_mode(hitscan|pellet|projectile|beam)`; \
         projectile `:kind(tracer|shell|plasma|rail)`; damage `:type_ident(bullet|pellet|plasma|rail|melee)`; \
         pickup `:kind(weapon|ammo|health)`; npc `:archetype(suppressor|flanker|breacher)` `:faction(hostile|neutral)`; \
         behavior `:tree(patrol_combat|guard)` `:default_tactic(suppress|flank|push|retreat)`; \
         objective `:kind(clear_hostiles|reach)`; audio `:kind(oneshot|loop)`; door `:state(open|closed)`; \
         trigger `:kind(enter|exit)`; cover `:quality(low|med|high)`; \
         particle_emitter `:kind(burst|spark|smoke)`; \
         post_process `:stage(taa|ssao|ssr|dof|bloom|tonemap|grain|sharpen)`.\n",
    );
    out
}

/// One-line AGENTS-style catalog entry for a game node.
pub fn format_game_catalog_line(spec: &GameNodeSpec) -> String {
    let props: Vec<String> = spec
        .props
        .iter()
        .map(|p| {
            let mut item = if p.required {
                format!("`{}`", p.name)
            } else {
                format!("`{}?`", p.name)
            };
            if matches!(p.kind, GamePropKind::Flag) {
                item.push_str(" (flag)");
            }
            item
        })
        .collect();
    let children = match spec.children {
        GameChildPolicy::None => "none".to_string(),
        GameChildPolicy::Any => "any".to_string(),
        GameChildPolicy::AnyOf(allowed) => allowed
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", "),
    };
    format!(
        "- `game::{}` — props: {}; children: {}",
        spec.name,
        if props.is_empty() {
            "none".into()
        } else {
            props.join(", ")
        },
        children
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameNode {
    pub name: String,
    pub name_span: Span,
    pub props: Vec<(String, Expr)>,
    pub prop_spans: Vec<Span>,
    pub children: Vec<GameNode>,
    pub span: Span,
}

impl GameNode {
    pub fn prop(&self, name: &str) -> Option<&Expr> {
        self.props.iter().find(|(n, _)| n == name).map(|(_, e)| e)
    }

    pub fn contains_node(&self, name: &str) -> bool {
        self.name == name || self.children.iter().any(|c| c.contains_node(name))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub name: String,
    pub root: GameNode,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameCapabilities {
    pub webgpu: bool,
    pub title: String,
    pub target_fps: u32,
    pub http_port: u16,
}

pub const DEFAULT_GAME_PORT: u16 = 18140;
pub const DEFAULT_GAME_FPS: u32 = 90;

fn validate_closed_ident(node: &str, prop: &str, value: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "game::{node} :{prop} must be one of {}",
            allowed.join("|")
        ))
    }
}

pub fn validate_game_node(node: &GameNode) -> Result<(), String> {
    let spec = lookup_game_node(&node.name).ok_or_else(|| {
        format!(
            "unknown game node `game::{}`; known: {}",
            node.name,
            catalog_game_node_names().join(", ")
        )
    })?;

    for prop in spec.props.iter().filter(|p| p.required) {
        if node.prop(prop.name).is_none() {
            return Err(format!(
                "game::{} requires prop `:{}`",
                node.name, prop.name
            ));
        }
    }
    for (pname, _) in &node.props {
        if !spec.props.iter().any(|p| p.name == *pname) {
            return Err(format!("unknown prop `:{}` on game::{}", pname, node.name));
        }
    }

    for prop in spec.props {
        if prop.closed_values.is_empty() {
            continue;
        }
        if let Some(Expr::Ident(v)) = node.prop(prop.name) {
            validate_closed_ident(&node.name, prop.name, v, prop.closed_values)?;
        }
    }

    match spec.children {
        GameChildPolicy::None if !node.children.is_empty() => {
            return Err(format!("game::{} does not accept children", node.name));
        }
        GameChildPolicy::AnyOf(allowed) => {
            for child in &node.children {
                if !allowed.contains(&child.name.as_str()) {
                    return Err(format!(
                        "game::{} cannot contain game::{}; allowed: {}",
                        node.name,
                        child.name,
                        allowed.join(", ")
                    ));
                }
                validate_game_node(child)?;
            }
        }
        GameChildPolicy::Any => {
            for child in &node.children {
                validate_game_node(child)?;
            }
        }
        GameChildPolicy::None => {}
    }

    if node.name == "scene" {
        if let Some(Expr::Ident(r)) = node.prop("renderer") {
            if r != "webgpu" {
                return Err("game::scene :renderer must be `webgpu`".into());
            }
        }
        let mut keys = std::collections::HashSet::new();
        collect_ability_keys(node, &mut keys)?;
    }
    Ok(())
}

fn collect_ability_keys(
    node: &GameNode,
    keys: &mut std::collections::HashSet<String>,
) -> Result<(), String> {
    if node.name == "ability" {
        if let Some(Expr::String(k)) = node.prop("key") {
            if !keys.insert(k.clone()) {
                return Err(format!("duplicate ability key `{k}`"));
            }
        }
    }
    for child in &node.children {
        collect_ability_keys(child, keys)?;
    }
    Ok(())
}

pub fn validate_game(game: &Game) -> Result<(), String> {
    if game.root.name != "scene" {
        return Err(format!(
            "game `{}` root must be `game::scene`, found `game::{}`",
            game.name, game.root.name
        ));
    }
    validate_game_node(&game.root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;

    #[test]
    fn catalog_has_scene_root() {
        assert!(lookup_game_node("scene").is_some());
        assert!(lookup_game_node("entity").is_some());
        assert!(lookup_game_node("prefab").is_some());
        assert!(lookup_game_node("mode").is_some());
        assert!(GAME_NODE_CATALOG.len() >= 40);
    }

    #[test]
    fn validates_minimal_scene() {
        let game = Game {
            name: "Demo".into(),
            root: GameNode {
                name: "scene".into(),
                name_span: Span::default(),
                props: vec![("title".into(), Expr::String("Demo".into()))],
                prop_spans: vec![Span::default()],
                children: vec![],
                span: Span::default(),
            },
            span: Span::default(),
        };
        validate_game(&game).unwrap();
    }

    #[test]
    fn every_game_prop_has_description() {
        for node in GAME_NODE_CATALOG {
            assert!(
                node.description.len() > 40,
                "game::{} description too short",
                node.name
            );
            for prop in node.props {
                assert!(
                    !prop.description.is_empty() && prop.description.len() > 20,
                    "game::{} :{} needs a teaching description",
                    node.name,
                    prop.name
                );
                for value in prop.closed_values {
                    assert!(
                        game_closed_value_doc(value).is_some(),
                        "missing game_closed_value_doc for `{value}` on game::{} :{}",
                        node.name,
                        prop.name
                    );
                }
            }
        }
    }

    #[test]
    fn format_game_catalog_lists_closed_nodes() {
        let md = format_game_catalog_md();
        assert!(md.contains("game::scene"));
        assert!(md.contains("game::entity"));
        assert!(md.contains("game::prefab"));
        assert!(md.contains("game::ability"));
        assert!(md.contains("plane|box|capsule|sphere"));
        assert!(md.contains("closed:"));
    }
}
