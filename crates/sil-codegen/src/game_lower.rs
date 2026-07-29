//! Lower `game::scene` trees to a deterministic JSON runtime manifest.
//!
//! Authors declare intent; this pass encodes SPEC systems as data consumed by
//! the compiler-owned Babylon/WebGPU runtime under `templates/game/`.

use serde_json::{json, Map, Value};
use sil_core::{Expr, Game, GameNode};

/// Lower a validated `game` declaration to a JSON manifest object.
pub fn lower_game(game: &Game) -> Result<Value, String> {
    if game.root.name != "scene" {
        return Err("game root must be `game::scene(...)`".into());
    }
    let root = &game.root;
    let title = string_prop(root, "title").unwrap_or_else(|| game.name.clone());
    let target_fps = number_prop(root, "target_fps").unwrap_or(90.0) as u64;
    let renderer = ident_prop(root, "renderer").unwrap_or_else(|| "webgpu".into());
    if renderer != "webgpu" {
        return Err(format!(
            "game::scene :renderer({renderer}) is unsupported; only webgpu is allowed"
        ));
    }

    let mut manifest = json!({
        "title": title,
        "targetFps": target_fps,
        "renderer": "webgpu",
    });
    let obj = manifest.as_object_mut().unwrap();

    for child in &root.children {
        match child.name.as_str() {
            "terrain" => {
                obj.insert("terrain".into(), lower_terrain(child)?);
            }
            "surface" => {
                obj.insert(
                    "surface".into(),
                    json!({
                        "profile": ident_prop(child, "profile").unwrap_or_else(|| "snow".into()),
                        "glint": number_prop(child, "glint"),
                        "scatter": number_prop(child, "scatter"),
                    }),
                );
            }
            "deformation" => {
                let extent = number_prop(child, "coverage_m")
                    .or_else(|| number_prop(child, "extent_m"))
                    .unwrap_or(80.0);
                let resolution = number_prop(child, "resolution");
                let texel_cm = number_prop(child, "texel_cm").unwrap_or_else(|| {
                    resolution
                        .map(|res| (extent * 100.0 / res.max(1.0)).max(0.5))
                        .unwrap_or(2.0)
                });
                obj.insert(
                    "deformation".into(),
                    json!({
                        "extentM": extent,
                        "texelCm": texel_cm,
                        "resolution": resolution,
                        "refillRate": number_prop(child, "refill_rate").or_else(|| number_prop(child, "refill")),
                    }),
                );
            }
            "environment" => {
                obj.insert(
                    "environment".into(),
                    json!({
                        "sunElevationDeg": number_prop(child, "sun_elevation_deg").unwrap_or(18.0),
                        "sunAzimuthDeg": number_prop(child, "sun_azimuth_deg"),
                        "fogDensity": number_prop(child, "fog_density").unwrap_or(0.012),
                        "spindrift": bool_prop(child, "spindrift").unwrap_or(true),
                    }),
                );
            }
            "post_process" => {
                let stages = obj
                    .entry("post")
                    .or_insert_with(|| Value::Array(Vec::new()));
                let arr = stages.as_array_mut().unwrap();
                arr.push(json!({
                    "name": ident_prop(child, "stage").or_else(|| string_prop(child, "stage")).unwrap_or_else(|| "unknown".into()),
                    "enabled": bool_prop(child, "enabled").unwrap_or(true),
                }));
            }
            "character" => {
                let mut cloth_regions = Vec::new();
                let mut fur_regions = Vec::new();
                let mut fur_shells = None;
                for c in &child.children {
                    match c.name.as_str() {
                        "cloth" => {
                            cloth_regions
                                .push(ident_prop(c, "region").unwrap_or_else(|| "hem".into()));
                        }
                        "fur" => {
                            fur_regions
                                .push(ident_prop(c, "region").unwrap_or_else(|| "hood".into()));
                            if let Some(shells) = number_prop(c, "shells") {
                                fur_shells = Some(
                                    fur_shells
                                        .map(|prev: f64| prev.max(shells))
                                        .unwrap_or(shells),
                                );
                            }
                        }
                        _ => {}
                    }
                }
                let robe = bool_prop(child, "robe").unwrap_or(true);
                obj.insert(
                    "character".into(),
                    json!({
                        "style": ident_prop(child, "style").unwrap_or_else(|| "robe".into()),
                        "robe": robe,
                        "fur": !fur_regions.is_empty() || child.contains_node("fur"),
                        "cloth": !cloth_regions.is_empty() || child.contains_node("cloth"),
                        "clothRegions": cloth_regions,
                        "furRegions": fur_regions,
                        "furShells": fur_shells,
                        "moveSpeed": number_prop(child, "move_speed"),
                    }),
                );
            }
            "camera" => {
                obj.insert(
                    "camera".into(),
                    json!({
                        "mode": ident_prop(child, "mode").unwrap_or_else(|| "third_person".into()),
                        "fovDeg": number_prop(child, "fov_deg").unwrap_or(55.0),
                        "distanceM": number_prop(child, "distance_m"),
                        "shoulderOffsetM": number_prop(child, "shoulder_offset_m")
                            .or_else(|| number_prop(child, "shoulder_m")),
                    }),
                );
            }
            "controls" => {
                let scheme = string_prop(child, "scheme")
                    .or_else(|| ident_prop(child, "scheme"))
                    .unwrap_or_else(|| "wasd_mouse".into());
                obj.insert(
                    "controls".into(),
                    json!({
                        "move": string_prop(child, "move").or_else(|| ident_prop(child, "move")).unwrap_or_else(|| "wasd".into()),
                        "look": string_prop(child, "look").or_else(|| ident_prop(child, "look")).unwrap_or_else(|| "mouse".into()),
                        "zoom": string_prop(child, "zoom").or_else(|| ident_prop(child, "zoom")).unwrap_or_else(|| "wheel".into()),
                        "scheme": scheme,
                    }),
                );
            }
            "movement_mode" => {
                let modes = obj
                    .entry("movementModes")
                    .or_insert_with(|| Value::Array(Vec::new()));
                modes.as_array_mut().unwrap().push(json!({
                    "name": string_prop(child, "name").or_else(|| ident_prop(child, "name")).unwrap_or_else(|| "surf".into()),
                    "hold": string_prop(child, "hold").or_else(|| ident_prop(child, "hold")).unwrap_or_else(|| "RMB".into()),
                    "effects": lower_effects(&child.children)?,
                }));
            }
            "ability" => {
                let abilities = obj
                    .entry("abilities")
                    .or_insert_with(|| Value::Array(Vec::new()));
                abilities.as_array_mut().unwrap().push(json!({
                    "name": string_prop(child, "name").unwrap_or_else(|| "Ability".into()),
                    "key": string_prop(child, "key").unwrap_or_else(|| "1".into()),
                    "effects": lower_effects(&child.children)?,
                }));
            }
            "overlay" => {
                obj.insert(
                    "overlay".into(),
                    json!({
                        "toggle": string_prop(child, "toggle").unwrap_or_else(|| "F1".into()),
                    }),
                );
            }
            "dynamic_light" => {
                let lights = obj
                    .entry("dynamicLights")
                    .or_insert_with(|| Value::Array(Vec::new()));
                lights.as_array_mut().unwrap().push(json!({
                    "radiusM": number_prop(child, "radius_m").unwrap_or(4.0),
                    "intensity": number_prop(child, "intensity").unwrap_or(1.0),
                    "color": string_prop(child, "color").unwrap_or_else(|| "#a8d4ff".into()),
                }));
            }
            other => {
                return Err(format!(
                    "game::scene cannot contain top-level `game::{other}`"
                ));
            }
        }
    }

    Ok(manifest)
}

/// Derive a compile-time CPython bake plan from the lowered scene (terrain/surface).
pub fn bake_plan_from_manifest(manifest: &Value) -> Value {
    let terrain = manifest.get("terrain").cloned().unwrap_or(json!({}));
    let layers = terrain.get("layers").cloned().unwrap_or_else(|| json!([]));
    let wind = terrain
        .get("windDeg")
        .and_then(|v| v.as_f64())
        .unwrap_or(35.0);
    let surface = manifest
        .get("surface")
        .and_then(|s| s.get("profile"))
        .and_then(|p| p.as_str())
        .unwrap_or("snow");
    json!({
        "resolution": 256,
        "windDeg": wind,
        "layers": layers,
        "surfaceProfile": surface,
        "engine": "cpython-bake-v1",
    })
}

fn lower_terrain(node: &GameNode) -> Result<Value, String> {
    let mut layers = Vec::new();
    for child in &node.children {
        if child.name != "height_layer" {
            return Err(format!(
                "game::terrain only accepts game::height_layer children, got game::{}",
                child.name
            ));
        }
        layers.push(json!({
            "kind": ident_prop(child, "kind").unwrap_or_else(|| "dune".into()),
            "scaleM": number_prop(child, "wavelength_m").unwrap_or(40.0),
            "amplitudeM": number_prop(child, "amplitude_m").unwrap_or(2.0),
            "shear": number_prop(child, "shear"),
        }));
    }
    Ok(json!({
        "windDeg": number_prop(node, "wind_dir").unwrap_or(35.0),
        "layers": layers,
        "extentM": number_prop(node, "extent_m").unwrap_or(800.0),
        "nearSpacingCm": number_prop(node, "near_spacing_cm").unwrap_or(8.0),
    }))
}

fn lower_effects(nodes: &[GameNode]) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for node in nodes {
        out.push(lower_effect(node)?);
    }
    Ok(out)
}

fn lower_effect(node: &GameNode) -> Result<Value, String> {
    let mut props = Map::new();
    for (k, v) in &node.props {
        // Emit both authored snake_case and runtime camelCase aliases so
        // compiler-owned TypeScript effect modules can read either form.
        props.insert(k.clone(), expr_json(v));
        if let Some(alias) = snake_to_camel_prop(k) {
            props.insert(alias, expr_json(v));
        }
    }
    let children = lower_effects(&node.children)?;
    Ok(json!({
        "type": node.name,
        "props": props,
        "children": children,
    }))
}

fn snake_to_camel_prop(name: &str) -> Option<String> {
    if !name.contains('_') {
        return None;
    }
    let mut out = String::with_capacity(name.len());
    let mut upper = false;
    for ch in name.chars() {
        if ch == '_' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

fn expr_json(expr: &Expr) -> Value {
    match expr {
        Expr::String(s) => Value::String(s.clone()),
        Expr::Number(n) => n
            .parse::<f64>()
            .map(|f| json!(f))
            .unwrap_or_else(|_| Value::String(n.clone())),
        Expr::Bool(b) => Value::Bool(*b),
        Expr::Ident(s) => Value::String(s.clone()),
        _ => Value::Null,
    }
}

fn string_prop(node: &GameNode, name: &str) -> Option<String> {
    node.prop(name)
        .and_then(|e| e.as_string_literal().map(|s| s.to_string()))
}

fn ident_prop(node: &GameNode, name: &str) -> Option<String> {
    node.prop(name).and_then(|e| match e {
        Expr::Ident(s) => Some(s.clone()),
        Expr::String(s) => Some(s.clone()),
        _ => None,
    })
}

fn number_prop(node: &GameNode, name: &str) -> Option<f64> {
    node.prop(name).and_then(|e| match e {
        Expr::Number(n) => n.parse().ok(),
        _ => None,
    })
}

fn bool_prop(node: &GameNode, name: &str) -> Option<bool> {
    node.prop(name).and_then(|e| match e {
        Expr::Bool(b) => Some(*b),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sil_core::types::Span;
    use sil_core::{Game, GameNode};

    fn span() -> Span {
        Span::new(0, 0, 1, 1)
    }

    fn node(name: &str, props: Vec<(&str, Expr)>, children: Vec<GameNode>) -> GameNode {
        GameNode {
            name: name.into(),
            name_span: span(),
            props: props.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            prop_spans: Vec::new(),
            children,
            span: span(),
        }
    }

    #[test]
    fn lowers_minimal_scene() {
        let game = Game {
            name: "Demo".into(),
            root: node(
                "scene",
                vec![("title", Expr::String("Demo".into()))],
                vec![node(
                    "overlay",
                    vec![("toggle", Expr::String("F1".into()))],
                    vec![],
                )],
            ),
            span: span(),
        };
        let v = lower_game(&game).unwrap();
        assert_eq!(v["title"], "Demo");
        assert_eq!(v["renderer"], "webgpu");
        assert_eq!(v["overlay"]["toggle"], "F1");
    }

    #[test]
    fn lowers_terrain_shear_deformation_resolution_and_effect_aliases() {
        let game = Game {
            name: "Demo".into(),
            root: node(
                "scene",
                vec![("title", Expr::String("Demo".into()))],
                vec![
                    node(
                        "terrain",
                        vec![
                            ("extent_m", Expr::Number("800".into())),
                            ("near_spacing_cm", Expr::Number("8".into())),
                        ],
                        vec![node(
                            "height_layer",
                            vec![
                                ("kind", Expr::Ident("sastrugi".into())),
                                ("amplitude_m", Expr::Number("0.4".into())),
                                ("wavelength_m", Expr::Number("2".into())),
                                ("shear", Expr::Number("0.6".into())),
                            ],
                            vec![],
                        )],
                    ),
                    node(
                        "deformation",
                        vec![
                            ("coverage_m", Expr::Number("80".into())),
                            ("resolution", Expr::Number("2048".into())),
                        ],
                        vec![],
                    ),
                    node(
                        "ability",
                        vec![
                            ("name", Expr::String("Sweep".into())),
                            ("key", Expr::String("1".into())),
                        ],
                        vec![node(
                            "terrain_brush",
                            vec![
                                ("shape", Expr::Ident("channel".into())),
                                ("depth_m", Expr::Number("0.25".into())),
                                ("radius_m", Expr::Number("1.8".into())),
                            ],
                            vec![],
                        )],
                    ),
                ],
            ),
            span: span(),
        };
        let v = lower_game(&game).unwrap();
        assert_eq!(v["terrain"]["layers"][0]["shear"], 0.6);
        assert_eq!(v["terrain"]["nearSpacingCm"], 8.0);
        assert_eq!(v["deformation"]["resolution"], 2048.0);
        assert_eq!(v["abilities"][0]["effects"][0]["props"]["depth_m"], 0.25);
        assert_eq!(v["abilities"][0]["effects"][0]["props"]["depthM"], 0.25);
    }
}
