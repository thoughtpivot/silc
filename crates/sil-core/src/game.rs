//! Game subject: real-time WebGPU scene/effect trees (ADR-012).
//!
//! Authors declare `game Name { game::scene(...) }`. The catalog is closed;
//! compiler-owned Babylon/WGSL templates implement the nodes. Distinct from
//! dual-surface `ui::*` / `app` routes.

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
    "terrain",
    "surface",
    "deformation",
    "environment",
    "post_process",
    "character",
    "camera",
    "controls",
    "movement_mode",
    "ability",
    "overlay",
    "dynamic_light",
];

const TERRAIN_CHILDREN: &[&str] = &["height_layer"];
const CHARACTER_CHILDREN: &[&str] = &["cloth", "fur"];
const ABILITY_CHILDREN: &[&str] = &[
    "ribbon",
    "wake",
    "particle_emitter",
    "terrain_brush",
    "state_write",
    "crystal_growth",
    "dynamic_light",
    "camera_impulse",
];
const MOVEMENT_CHILDREN: &[&str] = &["wake", "terrain_brush", "state_write", "particle_emitter"];

pub const GAME_NODE_CATALOG: &[GameNodeSpec] = &[
    GameNodeSpec {
        name: "scene",
        description: "Root WebGPU game scene. Required single root of a `game` declaration.",
        props: &[
            GamePropSpec {
                name: "title",
                kind: GamePropKind::String,
                required: true,
            },
            GamePropSpec {
                name: "renderer",
                kind: GamePropKind::Ident,
                required: false,
            },
            GamePropSpec {
                name: "target_fps",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::AnyOf(SCENE_CHILDREN),
    },
    GameNodeSpec {
        name: "terrain",
        description: "Player-centered clipmap terrain with procedural height layers.",
        props: &[
            GamePropSpec {
                name: "extent_m",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "near_spacing_cm",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "wind_dir",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::AnyOf(TERRAIN_CHILDREN),
    },
    GameNodeSpec {
        name: "height_layer",
        description: "One procedural height octave (dune / drift / sastrugi).",
        props: &[
            GamePropSpec {
                name: "kind",
                kind: GamePropKind::Ident,
                required: true,
            },
            GamePropSpec {
                name: "amplitude_m",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "wavelength_m",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "shear",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "surface",
        description: "Surface shading profile (snow / ice) with art knobs.",
        props: &[
            GamePropSpec {
                name: "profile",
                kind: GamePropKind::Ident,
                required: true,
            },
            GamePropSpec {
                name: "glint",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "scatter",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "deformation",
        description: "Persistent player-following terrain state buffer.",
        props: &[
            GamePropSpec {
                name: "coverage_m",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "resolution",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "texel_cm",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "refill_rate",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "environment",
        description: "Sun, ambient, fog, spindrift, and sky IBL.",
        props: &[
            GamePropSpec {
                name: "sun_elevation_deg",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "sun_azimuth_deg",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "fog_density",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "spindrift",
                kind: GamePropKind::Flag,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "post_process",
        description: "One post-process stage in the ordered chain.",
        props: &[
            GamePropSpec {
                name: "stage",
                kind: GamePropKind::Ident,
                required: true,
            },
            GamePropSpec {
                name: "enabled",
                kind: GamePropKind::Bool,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "character",
        description: "Third-person player figure (robe silhouette).",
        props: &[
            GamePropSpec {
                name: "style",
                kind: GamePropKind::Ident,
                required: false,
            },
            GamePropSpec {
                name: "move_speed",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::AnyOf(CHARACTER_CHILDREN),
    },
    GameNodeSpec {
        name: "cloth",
        description: "Cloth simulation regions on the character.",
        props: &[GamePropSpec {
            name: "region",
            kind: GamePropKind::Ident,
            required: true,
        }],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "fur",
        description: "Shell-fur trim on hood/cuffs.",
        props: &[
            GamePropSpec {
                name: "region",
                kind: GamePropKind::Ident,
                required: true,
            },
            GamePropSpec {
                name: "shells",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "camera",
        description: "Third-person spring-arm camera.",
        props: &[
            GamePropSpec {
                name: "mode",
                kind: GamePropKind::Ident,
                required: false,
            },
            GamePropSpec {
                name: "distance_m",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "shoulder_offset_m",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "controls",
        description: "WASD + mouse orbit/zoom input binding.",
        props: &[GamePropSpec {
            name: "scheme",
            kind: GamePropKind::Ident,
            required: false,
        }],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "movement_mode",
        description: "Locomotion or snow-surf mode with optional wake/brush children.",
        props: &[
            GamePropSpec {
                name: "name",
                kind: GamePropKind::Ident,
                required: true,
            },
            GamePropSpec {
                name: "hold",
                kind: GamePropKind::Ident,
                required: false,
            },
        ],
        children: GameChildPolicy::AnyOf(MOVEMENT_CHILDREN),
    },
    GameNodeSpec {
        name: "ability",
        description: "Keyed ability composed of effect children.",
        props: &[
            GamePropSpec {
                name: "name",
                kind: GamePropKind::String,
                required: true,
            },
            GamePropSpec {
                name: "key",
                kind: GamePropKind::String,
                required: true,
            },
        ],
        children: GameChildPolicy::AnyOf(ABILITY_CHILDREN),
    },
    GameNodeSpec {
        name: "ribbon",
        description: "Swept ribbon/tube water or snow body.",
        props: &[
            GamePropSpec {
                name: "kind",
                kind: GamePropKind::Ident,
                required: false,
            },
            GamePropSpec {
                name: "width_m",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "wake",
        description: "Curling surf wake of displaced mass.",
        props: &[GamePropSpec {
            name: "intensity",
            kind: GamePropKind::Number,
            required: false,
        }],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "particle_emitter",
        description: "Pooled GPU particle burst or continuous spray.",
        props: &[
            GamePropSpec {
                name: "kind",
                kind: GamePropKind::Ident,
                required: true,
            },
            GamePropSpec {
                name: "count",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "terrain_brush",
        description: "Writes depression/berms into the deformation buffer.",
        props: &[
            GamePropSpec {
                name: "shape",
                kind: GamePropKind::Ident,
                required: false,
            },
            GamePropSpec {
                name: "depth_m",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "radius_m",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "state_write",
        description: "Persistent surface state write (compression / wetness / ice).",
        props: &[
            GamePropSpec {
                name: "channel",
                kind: GamePropKind::Ident,
                required: true,
            },
            GamePropSpec {
                name: "amount",
                kind: GamePropKind::Number,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "crystal_growth",
        description: "Refractive ice crystal growth from the drift.",
        props: &[GamePropSpec {
            name: "scale_m",
            kind: GamePropKind::Number,
            required: false,
        }],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "dynamic_light",
        description: "Tight-radius dynamic light (spell illumination).",
        props: &[
            GamePropSpec {
                name: "radius_m",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "intensity",
                kind: GamePropKind::Number,
                required: false,
            },
            GamePropSpec {
                name: "color",
                kind: GamePropKind::String,
                required: false,
            },
        ],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "camera_impulse",
        description: "Subtle camera shake impulse on ability cast.",
        props: &[GamePropSpec {
            name: "strength",
            kind: GamePropKind::Number,
            required: false,
        }],
        children: GameChildPolicy::None,
    },
    GameNodeSpec {
        name: "overlay",
        description: "Hidden-by-default settings/perf overlay (F1/backtick).",
        props: &[GamePropSpec {
            name: "toggle",
            kind: GamePropKind::String,
            required: true,
        }],
        children: GameChildPolicy::None,
    },
];

pub fn lookup_game_node(name: &str) -> Option<&'static GameNodeSpec> {
    GAME_NODE_CATALOG.iter().find(|n| n.name == name)
}

pub fn catalog_game_node_names() -> Vec<&'static str> {
    GAME_NODE_CATALOG.iter().map(|n| n.name).collect()
}

/// Markdown digest of the closed `game::*` catalog for assist / docs.
pub fn format_game_catalog_md() -> String {
    let mut out = String::from(
        "# game::* catalog (ADR-012)\n\n\
         WebGPU programs declare one `game Name { game::scene(...) }` tree. \
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
                out.push_str(&format!("- `:{}` ({:?}, {req})\n", prop.name, prop.kind));
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
        "Closed enums: `:renderer(webgpu)`; surface `:profile(snow|ice)`; \
         height_layer `:kind(dune|drift|sastrugi|ripple)`; \
         post_process `:stage(taa|ssao|ssr|dof|bloom|tonemap|grain|sharpen)`; \
         terrain_brush `:shape(circle|groove|channel|score|crater|ring|crescent)`; \
         particle_emitter `:kind(spray|powder|drift|sparkle)`; \
         state_write `:channel(compression|wetness|ice)`; \
         cloth/fur `:region(hem|sleeves|mantle|hood|cuffs)`.\n",
    );
    out
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
        for child in &node.children {
            if child.name == "ability" {
                if let Some(Expr::String(k)) = child.prop("key") {
                    if !keys.insert(k.clone()) {
                        return Err(format!("duplicate ability key `{k}`"));
                    }
                }
            }
        }
    }
    if node.name == "surface" {
        if let Some(Expr::Ident(p)) = node.prop("profile") {
            if p != "snow" && p != "ice" {
                return Err("game::surface :profile must be `snow` or `ice`".into());
            }
        }
    }
    if node.name == "height_layer" {
        if let Some(Expr::Ident(k)) = node.prop("kind") {
            if !matches!(k.as_str(), "dune" | "drift" | "sastrugi" | "ripple") {
                return Err("game::height_layer :kind must be dune|drift|sastrugi|ripple".into());
            }
        }
    }
    if node.name == "post_process" {
        if let Some(Expr::Ident(s)) = node.prop("stage") {
            let ok = matches!(
                s.as_str(),
                "taa" | "ssao" | "ssr" | "dof" | "bloom" | "tonemap" | "grain" | "sharpen"
            );
            if !ok {
                return Err(format!("unknown post_process stage `{s}`"));
            }
        }
    }
    if node.name == "terrain_brush" {
        if let Some(Expr::Ident(s)) = node.prop("shape") {
            let ok = matches!(
                s.as_str(),
                "circle" | "groove" | "channel" | "score" | "crater" | "ring" | "crescent"
            );
            if !ok {
                return Err(format!(
                    "game::terrain_brush :shape must be circle|groove|channel|score|crater|ring|crescent"
                ));
            }
        }
    }
    if node.name == "particle_emitter" {
        if let Some(Expr::Ident(k)) = node.prop("kind") {
            let ok = matches!(k.as_str(), "spray" | "powder" | "drift" | "sparkle");
            if !ok {
                return Err(
                    "game::particle_emitter :kind must be spray|powder|drift|sparkle".into(),
                );
            }
        }
    }
    if node.name == "state_write" {
        if let Some(Expr::Ident(c)) = node.prop("channel") {
            let ok = matches!(c.as_str(), "compression" | "wetness" | "ice");
            if !ok {
                return Err("game::state_write :channel must be compression|wetness|ice".into());
            }
        }
    }
    if node.name == "cloth" || node.name == "fur" {
        if let Some(Expr::Ident(r)) = node.prop("region") {
            let ok = matches!(r.as_str(), "hem" | "sleeves" | "mantle" | "hood" | "cuffs");
            if !ok {
                return Err(format!(
                    "game::{} :region must be hem|sleeves|mantle|hood|cuffs",
                    node.name
                ));
            }
        }
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
        assert!(GAME_NODE_CATALOG.len() >= 20);
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
    fn format_game_catalog_lists_closed_nodes() {
        let md = format_game_catalog_md();
        assert!(md.contains("game::scene"));
        assert!(md.contains("game::terrain"));
        assert!(md.contains("game::ability"));
        assert!(md.contains("dune|drift|sastrugi|ripple"));
    }
}
