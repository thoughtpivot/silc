//! Lower `game::scene` trees to a deterministic JSON runtime manifest.
//!
//! Encodes Godot-style scene trees, Unity prefabs/data, and Unreal mode/pawn
//! ownership for the compiler-owned Babylon/WebGPU kernel under `templates/game/`.

use serde_json::{json, Map, Value};
use sil_core::{Expr, Game, GameNode, UnaryOp};

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

    let mut data = Map::new();
    let mut prefabs = Map::new();
    let mut assets = Map::new();
    let mut materials = Map::new();
    let mut weapons = Map::new();
    let mut zones = Vec::new();
    let mut tilemaps = Vec::new();
    let mut encounters = Vec::new();
    let mut objectives = Vec::new();
    let mut scene_children = Vec::new();
    let mut signal_edges = Vec::new();
    let mut post = Vec::new();
    let mut mode = Value::Null;
    let mut controller = json!({ "scheme": "wasd_mouse" });
    let mut camera = json!({
        "mode": "third_person",
        "distanceM": 4.5,
        "shoulderOffsetM": 0.6,
        "follow": "pawn",
    });
    let mut overlay = json!({ "toggle": "F1" });
    let mut hud = Value::Null;
    let mut environment = Value::Null;
    let mut shadow = Value::Null;

    for child in &root.children {
        match child.name.as_str() {
            "data" => {
                let name = string_prop(child, "name").ok_or("game::data requires :name")?;
                data.insert(name, lower_data_props(child));
            }
            "asset" => {
                let name = string_prop(child, "name").ok_or("game::asset requires :name")?;
                assets.insert(name, lower_asset(child));
            }
            "material" => {
                let name = string_prop(child, "name").ok_or("game::material requires :name")?;
                materials.insert(name, lower_material(child));
            }
            "prefab" => {
                let name = string_prop(child, "name").ok_or("game::prefab requires :name")?;
                prefabs.insert(name.clone(), lower_entity_like(child, Some(&name))?);
            }
            "entity" => {
                scene_children.push(lower_entity_like(child, None)?);
            }
            "spawn" => {
                scene_children.push(lower_spawn(child)?);
            }
            "zone" => {
                zones.push(lower_zone(child)?);
            }
            "weapon" => {
                let name = string_prop(child, "name").ok_or("game::weapon requires :name")?;
                weapons.insert(name, lower_weapon_def(child)?);
            }
            "encounter" => {
                encounters.push(lower_encounter(child)?);
            }
            "objective" => {
                objectives.push(lower_objective(child));
            }
            "mode" => {
                mode = lower_mode(child)?;
            }
            "controller" => {
                controller = json!({
                    "scheme": ident_prop(child, "scheme").unwrap_or_else(|| "wasd_mouse".into()),
                });
            }
            "camera" => {
                camera = json!({
                    "mode": ident_prop(child, "mode").unwrap_or_else(|| "third_person".into()),
                    "distanceM": number_prop(child, "distance_m").unwrap_or(4.5),
                    "shoulderOffsetM": number_prop(child, "shoulder_offset_m").unwrap_or(0.6),
                    "follow": ident_prop(child, "follow").unwrap_or_else(|| "pawn".into()),
                });
            }
            "post_process" => {
                post.push(json!({
                    "name": ident_prop(child, "stage").unwrap_or_else(|| "unknown".into()),
                    "enabled": bool_prop(child, "enabled").unwrap_or(true),
                }));
            }
            "overlay" => {
                overlay = json!({
                    "toggle": string_prop(child, "toggle").unwrap_or_else(|| "F1".into()),
                });
            }
            "hud" => {
                hud = lower_hud(child);
            }
            "environment" => {
                environment = lower_environment(child);
            }
            "shadow" => {
                shadow = lower_shadow(child);
            }
            "signal" => {
                if let Some(from) = ident_prop(child, "name").or_else(|| string_prop(child, "name")) {
                    if let Some(to) = string_prop(child, "on").or_else(|| ident_prop(child, "on")) {
                        signal_edges.push(json!({ "from": from, "to": to }));
                    }
                }
            }
            "group" => {}
            "tilemap" => {
                tilemaps.push(json!({
                    "asset": string_prop(child, "asset").unwrap_or_default(),
                    "tileset": string_prop(child, "tileset").unwrap_or_default(),
                    "tileSize": number_prop(child, "tile_size"),
                    "collisionLayer": string_prop(child, "collision_layer"),
                }));
            }
            other => {
                return Err(format!(
                    "game::scene cannot contain top-level `game::{other}`"
                ));
            }
        }
    }

    collect_signal_edges_from_tree(&scene_children, &mut signal_edges);
    for zone in &zones {
        if let Some(children) = zone.get("children").and_then(|c| c.as_array()) {
            collect_signal_edges_from_tree(children, &mut signal_edges);
        }
    }
    for (_k, prefab) in &prefabs {
        if let Some(arr) = prefab.get("signals").and_then(|s| s.as_array()) {
            for sig in arr {
                if let (Some(name), Some(on)) = (
                    sig.get("name").and_then(|v| v.as_str()),
                    sig.get("on").and_then(|v| v.as_str()),
                ) {
                    if !on.is_empty() {
                        signal_edges.push(json!({ "from": name, "to": on }));
                    }
                }
            }
        }
    }

    Ok(json!({
        "title": title,
        "targetFps": target_fps,
        "renderer": "webgpu",
        "data": data,
        "assets": assets,
        "materials": materials,
        "weapons": weapons,
        "zones": zones,
        "tilemaps": tilemaps,
        "encounters": encounters,
        "objectives": objectives,
        "prefabs": prefabs,
        "scene": { "children": scene_children },
        "mode": mode,
        "controller": controller,
        "camera": camera,
        "hud": hud,
        "environment": environment,
        "shadow": shadow,
        "signals": signal_edges,
        "post": post,
        "overlay": overlay,
    }))
}

/// Derive a compile-time CPython bake plan from the lowered scene graph.
pub fn bake_plan_from_manifest(manifest: &Value) -> Value {
    let data = manifest.get("data").cloned().unwrap_or_else(|| json!({}));
    let prefabs = manifest.get("prefabs").cloned().unwrap_or_else(|| json!({}));
    let assets = manifest.get("assets").cloned().unwrap_or_else(|| json!({}));
    let materials = manifest.get("materials").cloned().unwrap_or_else(|| json!({}));
    let zones = manifest.get("zones").cloned().unwrap_or_else(|| json!([]));
    let weapons = manifest.get("weapons").cloned().unwrap_or_else(|| json!({}));
    let encounters = manifest.get("encounters").cloned().unwrap_or_else(|| json!([]));
    let objectives = manifest.get("objectives").cloned().unwrap_or_else(|| json!([]));
    let environment = manifest.get("environment").cloned().unwrap_or(Value::Null);
    let signals = manifest.get("signals").cloned().unwrap_or_else(|| json!([]));
    let mode = manifest.get("mode").cloned().unwrap_or(Value::Null);
    let scene = manifest
        .get("scene")
        .cloned()
        .unwrap_or_else(|| json!({ "children": [] }));

    let mut colliders = Vec::new();
    let mut spawns = Vec::new();
    collect_colliders_and_spawns(&prefabs, &scene, &mut colliders, &mut spawns);
    for zone in zones.as_array().unwrap_or(&Vec::new()) {
        if let Some(children) = zone.get("children").and_then(|c| c.as_array()) {
            walk_spawns(children, &mut spawns);
        }
        if let Some(zone_spawns) = zone.get("spawns").and_then(|s| s.as_array()) {
            for s in zone_spawns {
                spawns.push(s.clone());
            }
        }
    }

    let nav_hints = collect_nav_hints(&assets, &prefabs, &scene, &zones);
    let mind_refs = collect_mind_refs(&prefabs, &scene, &zones);

    json!({
        "engine": "cpython-bake-v1",
        "data": data,
        "assets": assets,
        "materials": materials,
        "zones": zones,
        "weapons": weapons,
        "encounters": encounters,
        "objectives": objectives,
        "environment": environment,
        "navHints": nav_hints,
        "mindRefs": mind_refs,
        "prefabs": prefabs,
        "colliders": colliders,
        "spawns": spawns,
        "signals": signals,
        "mode": mode,
        "attributes": collect_attributes(&prefabs),
    })
}

fn collect_nav_hints(
    assets: &Value,
    prefabs: &Value,
    scene: &Value,
    zones: &Value,
) -> Value {
    let mut navmesh_assets = Vec::new();
    if let Some(obj) = assets.as_object() {
        for (name, asset) in obj {
            if asset.get("kind").and_then(|k| k.as_str()) == Some("navmesh") {
                navmesh_assets.push(json!({
                    "name": name,
                    "path": asset.get("path"),
                }));
            }
        }
    }

    let mut agents = Vec::new();
    collect_nav_agents(prefabs, &mut agents);
    if let Some(children) = scene.get("children").and_then(|c| c.as_array()) {
        walk_nav_agents(children, &mut agents);
    }
    for zone in zones.as_array().unwrap_or(&Vec::new()) {
        if let Some(children) = zone.get("children").and_then(|c| c.as_array()) {
            walk_nav_agents(children, &mut agents);
        }
    }

    json!({
        "navmeshAssets": navmesh_assets,
        "agents": agents,
    })
}

fn collect_nav_agents(prefabs: &Value, agents: &mut Vec<Value>) {
    if let Some(obj) = prefabs.as_object() {
        for (name, prefab) in obj {
            if let Some(nav) = prefab.get("navAgent") {
                let mut entry = nav.clone();
                if let Some(m) = entry.as_object_mut() {
                    m.insert("prefab".into(), json!(name));
                }
                agents.push(entry);
            }
        }
    }
}

fn walk_nav_agents(nodes: &[Value], agents: &mut Vec<Value>) {
    for node in nodes {
        if let Some(nav) = node.get("navAgent") {
            let mut entry = nav.clone();
            if let Some(m) = entry.as_object_mut() {
                if let Some(id) = node.get("id").or_else(|| node.get("name")) {
                    m.insert("entity".into(), id.clone());
                }
            }
            agents.push(entry);
        }
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            walk_nav_agents(children, agents);
        }
    }
}

fn collect_mind_refs(prefabs: &Value, scene: &Value, zones: &Value) -> Value {
    let mut refs = Vec::new();
    if let Some(obj) = prefabs.as_object() {
        for prefab in obj.values() {
            push_mind_ref(prefab, &mut refs);
        }
    }
    if let Some(children) = scene.get("children").and_then(|c| c.as_array()) {
        walk_mind_refs(children, &mut refs);
    }
    for zone in zones.as_array().unwrap_or(&Vec::new()) {
        if let Some(children) = zone.get("children").and_then(|c| c.as_array()) {
            walk_mind_refs(children, &mut refs);
        }
    }
    refs.sort();
    refs.dedup();
    Value::Array(refs.into_iter().map(Value::String).collect())
}

fn push_mind_ref(node: &Value, refs: &mut Vec<String>) {
    if let Some(mind) = node.get("mind") {
        if let Some(r) = mind.get("ref").and_then(|v| v.as_str()) {
            refs.push(r.to_string());
        }
    }
}

fn walk_mind_refs(nodes: &[Value], refs: &mut Vec<String>) {
    for node in nodes {
        push_mind_ref(node, refs);
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            walk_mind_refs(children, refs);
        }
    }
}

fn collect_attributes(prefabs: &Value) -> Value {
    let mut out = Map::new();
    if let Some(obj) = prefabs.as_object() {
        for (name, prefab) in obj {
            if let Some(attrs) = prefab.get("attributes") {
                out.insert(name.clone(), attrs.clone());
            }
        }
    }
    Value::Object(out)
}

fn collect_colliders_and_spawns(
    prefabs: &Value,
    scene: &Value,
    colliders: &mut Vec<Value>,
    spawns: &mut Vec<Value>,
) {
    if let Some(obj) = prefabs.as_object() {
        for (name, prefab) in obj {
            if let Some(c) = prefab.get("collider") {
                let mut entry = c.clone();
                if let Some(m) = entry.as_object_mut() {
                    m.insert("prefab".into(), json!(name));
                }
                colliders.push(entry);
            }
        }
    }
    if let Some(children) = scene.get("children").and_then(|c| c.as_array()) {
        walk_spawns(children, spawns);
    }
}

fn walk_spawns(children: &[Value], spawns: &mut Vec<Value>) {
    for child in children {
        if child.get("spawn").is_some() {
            spawns.push(child.clone());
        }
        if let Some(nested) = child.get("children").and_then(|c| c.as_array()) {
            walk_spawns(nested, spawns);
        }
    }
}

fn collect_signal_edges_from_tree(nodes: &[Value], edges: &mut Vec<Value>) {
    for node in nodes {
        if let Some(sigs) = node.get("signals").and_then(|s| s.as_array()) {
            for sig in sigs {
                if let (Some(name), Some(on)) = (
                    sig.get("name").and_then(|v| v.as_str()),
                    sig.get("on").and_then(|v| v.as_str()),
                ) {
                    if !on.is_empty() {
                        edges.push(json!({ "from": name, "to": on }));
                    }
                }
            }
        }
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            collect_signal_edges_from_tree(children, edges);
        }
    }
}

fn lower_transform(node: &GameNode) -> Value {
    json!({
        "x": number_prop(node, "x").unwrap_or(0.0),
        "y": number_prop(node, "y").unwrap_or(0.0),
        "z": number_prop(node, "z").unwrap_or(0.0),
        "yaw": number_prop(node, "yaw").unwrap_or(0.0),
        "pitch": number_prop(node, "pitch").unwrap_or(0.0),
        "roll": number_prop(node, "roll").unwrap_or(0.0),
        "sx": number_prop(node, "sx").unwrap_or(1.0),
        "sy": number_prop(node, "sy").unwrap_or(1.0),
        "sz": number_prop(node, "sz").unwrap_or(1.0),
    })
}

fn lower_data_props(node: &GameNode) -> Value {
    let mut props = Map::new();
    for (k, v) in &node.props {
        if k == "name" {
            continue;
        }
        props.insert(snake_to_camel_prop(k).unwrap_or_else(|| k.clone()), expr_json(v));
        props.insert(k.clone(), expr_json(v));
    }
    Value::Object(props)
}

fn lower_asset(node: &GameNode) -> Value {
    json!({
        "path": string_prop(node, "path").unwrap_or_default(),
        "kind": ident_prop(node, "kind").unwrap_or_else(|| "gltf".into()),
    })
}

fn lower_material(node: &GameNode) -> Value {
    json!({
        "albedo": string_prop(node, "albedo"),
        "normal": string_prop(node, "normal"),
        "roughness": number_prop(node, "roughness"),
        "metallic": number_prop(node, "metallic"),
        "ao": string_prop(node, "ao"),
        "emissive": string_prop(node, "emissive"),
        "tiling": number_prop(node, "tiling"),
    })
}

fn lower_hud(node: &GameNode) -> Value {
    json!({
        "showCrosshair": bool_prop(node, "show_crosshair").unwrap_or(false),
        "showAmmo": bool_prop(node, "show_ammo").unwrap_or(false),
        "showHealth": bool_prop(node, "show_health").unwrap_or(false),
    })
}

fn lower_environment(node: &GameNode) -> Value {
    json!({
        "fogDensity": number_prop(node, "fog_density"),
        "fogColor": string_prop(node, "fog_color"),
        "skyColor": string_prop(node, "sky_color"),
        "exposure": number_prop(node, "exposure"),
    })
}

fn lower_shadow(node: &GameNode) -> Value {
    json!({
        "enabled": bool_prop(node, "enabled").unwrap_or(true),
        "cascadeCount": number_prop(node, "cascade_count").unwrap_or(3.0) as u64,
    })
}

fn lower_objective(node: &GameNode) -> Value {
    json!({
        "id": string_prop(node, "id").unwrap_or_else(|| "objective".into()),
        "kind": ident_prop(node, "kind").unwrap_or_else(|| "clear_hostiles".into()),
        "target": string_prop(node, "target"),
    })
}

fn lower_encounter(node: &GameNode) -> Result<Value, String> {
    let mut spawns = Vec::new();
    for child in &node.children {
        if child.name == "spawn" {
            spawns.push(lower_spawn(child)?);
        }
    }
    Ok(json!({
        "id": string_prop(node, "id").ok_or("game::encounter requires :id")?,
        "wave": number_prop(node, "wave"),
        "spawns": spawns,
    }))
}

fn lower_zone(node: &GameNode) -> Result<Value, String> {
    let mut children = Vec::new();
    let mut spawns = Vec::new();
    let mut lights = Vec::new();
    let mut signals = Vec::new();
    let mut groups = Vec::new();

    for child in &node.children {
        match child.name.as_str() {
            "entity" => children.push(lower_entity_like(child, None)?),
            "spawn" => spawns.push(lower_spawn(child)?),
            "light" => lights.push(lower_light_component(child)),
            "signal" => {
                signals.push(json!({
                    "name": ident_prop(child, "name").or_else(|| string_prop(child, "name")).unwrap_or_else(|| "signal".into()),
                    "on": string_prop(child, "on").or_else(|| ident_prop(child, "on")).unwrap_or_default(),
                }));
            }
            "group" => {
                groups.push(string_prop(child, "name").unwrap_or_else(|| "default".into()));
            }
            other => {
                return Err(format!("game::zone cannot contain game::{other}"));
            }
        }
    }

    Ok(json!({
        "name": string_prop(node, "name").ok_or("game::zone requires :name")?,
        "kind": ident_prop(node, "kind").unwrap_or_else(|| "room".into()),
        "children": children,
        "spawns": spawns,
        "lights": lights,
        "signals": signals,
        "groups": groups,
    }))
}

fn lower_weapon_def(node: &GameNode) -> Result<Value, String> {
    let mut projectiles = Vec::new();
    let mut effects = Vec::new();
    for child in &node.children {
        match child.name.as_str() {
            "projectile" => projectiles.push(lower_projectile(child)),
            "particle_emitter" | "dynamic_light" | "camera_impulse" | "audio" => {
                effects.push(lower_effect(child)?);
            }
            other => {
                return Err(format!("game::weapon cannot contain game::{other}"));
            }
        }
    }
    Ok(json!({
        "slot": slot_prop(node),
        "fireMode": ident_prop(node, "fire_mode").unwrap_or_else(|| "hitscan".into()),
        "ref": string_prop(node, "ref"),
        "damage": number_prop(node, "damage"),
        "fireRate": number_prop(node, "fire_rate"),
        "magazine": number_prop(node, "magazine"),
        "reload": number_prop(node, "reload"),
        "spread": number_prop(node, "spread"),
        "projectiles": projectiles,
        "effects": effects,
    }))
}

fn lower_projectile(node: &GameNode) -> Value {
    json!({
        "kind": ident_prop(node, "kind").unwrap_or_else(|| "tracer".into()),
        "speed": number_prop(node, "speed"),
        "lifetime": number_prop(node, "lifetime"),
        "splashRadius": number_prop(node, "splash_radius"),
        "color": string_prop(node, "color"),
        "size": number_prop(node, "size"),
    })
}

fn lower_mode(node: &GameNode) -> Result<Value, String> {
    let mut spawns = Vec::new();
    let mut mode_encounters = Vec::new();
    let mut mode_objectives = Vec::new();
    for child in &node.children {
        match child.name.as_str() {
            "spawn" => spawns.push(lower_spawn(child)?),
            "encounter" => mode_encounters.push(lower_encounter(child)?),
            "objective" => mode_objectives.push(lower_objective(child)),
            other => {
                return Err(format!("game::mode cannot contain game::{other}"));
            }
        }
    }
    Ok(json!({
        "id": string_prop(node, "id").unwrap_or_else(|| "default".into()),
        "possess": string_prop(node, "possess"),
        "spawns": spawns,
        "encounters": mode_encounters,
        "objectives": mode_objectives,
    }))
}

fn lower_spawn(node: &GameNode) -> Result<Value, String> {
    Ok(json!({
        "spawn": string_prop(node, "prefab").ok_or("game::spawn requires :prefab")?,
        "overrides": {
            "transform": lower_transform(node),
        },
        "asPawn": bool_prop(node, "as_pawn").unwrap_or(false),
    }))
}

fn lower_light_component(node: &GameNode) -> Value {
    json!({
        "kind": ident_prop(node, "kind").unwrap_or_else(|| "directional".into()),
        "intensity": number_prop(node, "intensity").unwrap_or(1.0),
        "color": string_prop(node, "color").unwrap_or_else(|| "#ffffff".into()),
        "radiusM": number_prop(node, "radius_m").unwrap_or(8.0),
        "castShadows": bool_prop(node, "cast_shadows").unwrap_or(false),
    })
}

fn lower_mesh_component(node: &GameNode) -> Value {
    let mut mesh = json!({
        "size": number_prop(node, "size").unwrap_or(1.0),
        "color": string_prop(node, "color").unwrap_or_else(|| "#888888".into()),
    });
    if let Some(shape) = ident_prop(node, "shape") {
        mesh["shape"] = json!(shape);
    }
    if let Some(asset) = string_prop(node, "asset") {
        mesh["asset"] = json!(asset);
    }
    if let Some(material) = string_prop(node, "material") {
        mesh["material"] = json!(material);
    }
    mesh
}

fn lower_entity_like(node: &GameNode, forced_name: Option<&str>) -> Result<Value, String> {
    let name = forced_name
        .map(|s| s.to_string())
        .or_else(|| string_prop(node, "name"))
        .unwrap_or_else(|| node.name.clone());

    let mut children = Vec::new();
    let mut signals = Vec::new();
    let mut groups = Vec::new();
    let mut components = Vec::new();
    let mut mesh = Value::Null;
    let mut light = Value::Null;
    let mut collider = Value::Null;
    let mut movement = Value::Null;
    let mut attributes = Vec::new();
    let mut abilities = Vec::new();
    let mut weapon = Value::Null;
    let mut ammo = Value::Null;
    let mut damage = Value::Null;
    let mut pickup = Value::Null;
    let mut npc = Value::Null;
    let mut perception = Value::Null;
    let mut behavior = Value::Null;
    let mut mind = Value::Null;
    let mut nav_agent = Value::Null;
    let mut door = Value::Null;
    let mut trigger = Value::Null;
    let mut cover = Value::Null;
    let mut audio = Value::Null;
    // Platformer components
    let mut sprite = Value::Null;
    let mut collectible = Value::Null;
    let mut interactable = Value::Null;
    let mut patrol = Value::Null;
    let mut warp = Value::Null;
    let mut level_end = Value::Null;
    let mut is_pawn = false;

    for child in &node.children {
        match child.name.as_str() {
            "entity" => children.push(lower_entity_like(child, None)?),
            "spawn" => children.push(lower_spawn(child)?),
            "signal" => {
                signals.push(json!({
                    "name": ident_prop(child, "name").or_else(|| string_prop(child, "name")).unwrap_or_else(|| "signal".into()),
                    "on": string_prop(child, "on").or_else(|| ident_prop(child, "on")).unwrap_or_default(),
                }));
            }
            "group" => {
                groups.push(string_prop(child, "name").unwrap_or_else(|| "default".into()));
            }
            "mesh" => {
                components.push("mesh");
                mesh = lower_mesh_component(child);
            }
            "light" => {
                components.push("light");
                light = lower_light_component(child);
            }
            "collider" => {
                components.push("collider");
                let size = number_prop(child, "size").unwrap_or(1.0);
                let sx = number_prop(node, "sx").unwrap_or(1.0);
                let sy = number_prop(node, "sy").unwrap_or(1.0);
                let sz = number_prop(node, "sz").unwrap_or(1.0);
                collider = json!({
                    "shape": ident_prop(child, "shape").unwrap_or_else(|| "box".into()),
                    "size": size,
                    "hull": {
                        "aabb": {
                            "x": size * sx,
                            "y": size * sy,
                            "z": size * sz
                        }
                    }
                });
            }
            "movement" => {
                components.push("movement");
                movement = json!({
                    "style": ident_prop(child, "style").unwrap_or_else(|| "walk".into()),
                    "speed": number_prop(child, "speed").unwrap_or(5.0),
                    "jumpSpeed": number_prop(child, "jump_speed"),
                    "sprintMul": number_prop(child, "sprint_mul"),
                    "ref": string_prop(child, "ref"),
                });
            }
            "attribute" => {
                components.push("attribute");
                attributes.push(json!({
                    "name": string_prop(child, "name").unwrap_or_else(|| "value".into()),
                    "value": number_prop(child, "value").unwrap_or(100.0),
                    "max": number_prop(child, "max"),
                }));
            }
            "pawn" => {
                components.push("pawn");
                is_pawn = true;
            }
            "ability" => {
                components.push("ability");
                abilities.push(json!({
                    "name": string_prop(child, "name").unwrap_or_else(|| "Ability".into()),
                    "key": string_prop(child, "key").unwrap_or_else(|| "1".into()),
                    "cooldown": number_prop(child, "cooldown").unwrap_or(0.5),
                    "cost": number_prop(child, "cost").unwrap_or(0.0),
                    "costAttr": string_prop(child, "cost_attr"),
                    "ref": string_prop(child, "ref"),
                    "effects": lower_effects(&child.children)?,
                }));
            }
            "weapon" => {
                components.push("weapon");
                weapon = json!({
                    "ref": string_prop(child, "ref").or_else(|| string_prop(child, "name")),
                    "slot": slot_prop(child),
                    "fireMode": ident_prop(child, "fire_mode"),
                    "damage": number_prop(child, "damage"),
                    "fireRate": number_prop(child, "fire_rate"),
                    "magazine": number_prop(child, "magazine"),
                    "reload": number_prop(child, "reload"),
                    "spread": number_prop(child, "spread"),
                });
            }
            "ammo" => {
                components.push("ammo");
                ammo = json!({
                    "name": string_prop(child, "name").unwrap_or_else(|| "ammo".into()),
                    "amount": number_prop(child, "amount").unwrap_or(0.0),
                    "max": number_prop(child, "max"),
                });
            }
            "damage" => {
                components.push("damage");
                damage = json!({
                    "amount": number_prop(child, "amount").unwrap_or(0.0),
                    "typeIdent": ident_prop(child, "type_ident").unwrap_or_else(|| "bullet".into()),
                });
            }
            "pickup" => {
                components.push("pickup");
                pickup = json!({
                    "kind": ident_prop(child, "kind").unwrap_or_else(|| "weapon".into()),
                    "ref": string_prop(child, "ref").unwrap_or_default(),
                    "amount": number_prop(child, "amount"),
                });
            }
            "npc" => {
                components.push("npc");
                npc = json!({
                    "archetype": ident_prop(child, "archetype").unwrap_or_else(|| "suppressor".into()),
                    "faction": ident_prop(child, "faction").unwrap_or_else(|| "hostile".into()),
                });
            }
            "perception" => {
                components.push("perception");
                perception = json!({
                    "sightM": number_prop(child, "sight_m"),
                    "hearM": number_prop(child, "hear_m"),
                    "fovDeg": number_prop(child, "fov_deg"),
                });
            }
            "behavior" => {
                components.push("behavior");
                behavior = json!({
                    "tree": ident_prop(child, "tree").unwrap_or_else(|| "patrol_combat".into()),
                    "defaultTactic": ident_prop(child, "default_tactic"),
                });
            }
            "mind" => {
                components.push("mind");
                mind = json!({
                    "ref": string_prop(child, "ref").unwrap_or_default(),
                    "cadenceS": number_prop(child, "cadence_s"),
                });
            }
            "nav_agent" => {
                components.push("navAgent");
                nav_agent = json!({
                    "radius": number_prop(child, "radius").unwrap_or(0.4),
                    "height": number_prop(child, "height").unwrap_or(1.8),
                    "maxSpeed": number_prop(child, "max_speed").unwrap_or(3.5),
                });
            }
            "door" => {
                components.push("door");
                door = json!({
                    "state": ident_prop(child, "state").unwrap_or_else(|| "closed".into()),
                    "auto": bool_prop(child, "auto").unwrap_or(false),
                });
            }
            "trigger" => {
                components.push("trigger");
                trigger = json!({
                    "kind": ident_prop(child, "kind").unwrap_or_else(|| "enter".into()),
                    "on": string_prop(child, "on").or_else(|| ident_prop(child, "on")).unwrap_or_default(),
                });
            }
            "cover" => {
                components.push("cover");
                cover = json!({
                    "quality": ident_prop(child, "quality").unwrap_or_else(|| "med".into()),
                });
            }
            "audio" => {
                components.push("audio");
                audio = json!({
                    "kind": ident_prop(child, "kind").unwrap_or_else(|| "oneshot".into()),
                    "path": string_prop(child, "path"),
                    "ref": string_prop(child, "ref"),
                    "volume": number_prop(child, "volume").unwrap_or(1.0),
                });
            }
            // Platformer components
            "sprite" => {
                components.push("sprite");
                sprite = json!({
                    "atlas": string_prop(child, "atlas").unwrap_or_default(),
                    "frame": string_prop(child, "frame"),
                    "width": number_prop(child, "width"),
                    "height": number_prop(child, "height"),
                    "animation": string_prop(child, "animation"),
                    "flipX": bool_prop(child, "flip_x"),
                    "billboard": bool_prop(child, "billboard"),
                });
            }
            "collectible" => {
                components.push("collectible");
                collectible = json!({
                    "kind": ident_prop(child, "kind").unwrap_or_else(|| "coin".into()),
                    "value": number_prop(child, "value"),
                    "onCollect": string_prop(child, "on_collect"),
                    "respawn": number_prop(child, "respawn"),
                });
            }
            "interactable" => {
                components.push("interactable");
                interactable = json!({
                    "kind": ident_prop(child, "kind").unwrap_or_else(|| "bumpable".into()),
                    "contents": string_prop(child, "contents"),
                    "health": number_prop(child, "health"),
                    "onInteract": string_prop(child, "on_interact"),
                });
            }
            "patrol" => {
                components.push("patrol");
                patrol = json!({
                    "behavior": ident_prop(child, "behavior").unwrap_or_else(|| "walk_reverse".into()),
                    "speed": number_prop(child, "speed"),
                    "bounds": number_prop(child, "bounds"),
                    "onStomp": string_prop(child, "on_stomp"),
                    "onTouch": string_prop(child, "on_touch"),
                });
            }
            "warp" => {
                components.push("warp");
                warp = json!({
                    "target": string_prop(child, "target").unwrap_or_default(),
                    "direction": ident_prop(child, "direction"),
                    "onWarp": string_prop(child, "on_warp"),
                });
            }
            "level_end" => {
                components.push("levelEnd");
                level_end = json!({
                    "onComplete": string_prop(child, "on_complete"),
                    "nextLevel": string_prop(child, "next_level"),
                });
            }
            other => {
                return Err(format!(
                    "game::{} cannot contain game::{other}",
                    node.name
                ));
            }
        }
    }

    Ok(json!({
        "id": name,
        "name": name,
        "transform": lower_transform(node),
        "components": components,
        "children": children,
        "signals": signals,
        "groups": groups,
        "mesh": mesh,
        "light": light,
        "collider": collider,
        "movement": movement,
        "attributes": attributes,
        "abilities": abilities,
        "weapon": weapon,
        "ammo": ammo,
        "damage": damage,
        "pickup": pickup,
        "npc": npc,
        "perception": perception,
        "behavior": behavior,
        "mind": mind,
        "navAgent": nav_agent,
        "door": door,
        "trigger": trigger,
        "cover": cover,
        "audio": audio,
        "pawn": is_pawn,
        // Platformer components
        "sprite": sprite,
        "collectible": collectible,
        "interactable": interactable,
        "patrol": patrol,
        "warp": warp,
        "levelEnd": level_end,
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
        props.insert(k.clone(), expr_json(v));
        if let Some(alias) = snake_to_camel_prop(k) {
            props.insert(alias, expr_json(v));
        }
    }
    Ok(json!({
        "type": node.name,
        "props": props,
        "children": [],
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
        other => expr_number(other).map(|f| json!(f)).unwrap_or(Value::Null),
    }
}

fn string_prop(node: &GameNode, name: &str) -> Option<String> {
    node.prop(name)
        .and_then(|e| e.as_string_literal().map(|s| s.to_string()))
}

fn slot_prop(node: &GameNode) -> Option<String> {
    match node.prop("slot") {
        Some(Expr::Number(n)) => Some(n.clone()),
        Some(Expr::String(s)) => Some(s.clone()),
        Some(Expr::Ident(s)) => Some(s.clone()),
        _ => None,
    }
}

fn ident_prop(node: &GameNode, name: &str) -> Option<String> {
    node.prop(name).and_then(|e| match e {
        Expr::Ident(s) => Some(s.clone()),
        Expr::String(s) => Some(s.clone()),
        _ => None,
    })
}

fn number_prop(node: &GameNode, name: &str) -> Option<f64> {
    node.prop(name).and_then(expr_number)
}

fn expr_number(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Number(n) => n.parse().ok(),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => expr_number(expr).map(|v| -v),
        _ => None,
    }
}

fn bool_prop(node: &GameNode, name: &str) -> Option<bool> {
    match node.prop(name) {
        Some(Expr::Bool(b)) => Some(*b),
        Some(Expr::Ident(s)) if s == "true" => Some(true),
        None => None,
        _ => node.prop(name).map(|_| true),
    }
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
        assert!(v["prefabs"].is_object());
        assert!(v["scene"]["children"].is_array());
        assert!(v["assets"].is_object());
        assert!(v["materials"].is_object());
        assert!(v["weapons"].is_object());
        assert!(v["zones"].is_array());
    }

    #[test]
    fn lowers_prefab_spawn_and_data() {
        let game = Game {
            name: "Arena".into(),
            root: node(
                "scene",
                vec![("title", Expr::String("ARENA".into()))],
                vec![
                    node(
                        "data",
                        vec![
                            ("name", Expr::String("WalkDefault".into())),
                            ("speed", Expr::Number("5".into())),
                        ],
                        vec![],
                    ),
                    node(
                        "prefab",
                        vec![("name", Expr::String("Player".into()))],
                        vec![
                            node(
                                "mesh",
                                vec![("shape", Expr::Ident("capsule".into()))],
                                vec![],
                            ),
                            node("pawn", vec![], vec![]),
                            node(
                                "movement",
                                vec![
                                    ("style", Expr::Ident("walk".into())),
                                    ("ref", Expr::String("WalkDefault".into())),
                                ],
                                vec![],
                            ),
                        ],
                    ),
                    node(
                        "spawn",
                        vec![
                            ("prefab", Expr::String("Player".into())),
                            ("x", Expr::Number("0".into())),
                            ("as_pawn", Expr::Bool(true)),
                        ],
                        vec![],
                    ),
                    node(
                        "mode",
                        vec![
                            ("id", Expr::String("arena".into())),
                            ("possess", Expr::String("Player".into())),
                        ],
                        vec![],
                    ),
                ],
            ),
            span: span(),
        };
        let v = lower_game(&game).unwrap();
        assert_eq!(v["data"]["WalkDefault"]["speed"], 5.0);
        assert_eq!(v["prefabs"]["Player"]["pawn"], true);
        assert_eq!(v["scene"]["children"][0]["spawn"], "Player");
        assert_eq!(v["mode"]["id"], "arena");
        let plan = bake_plan_from_manifest(&v);
        assert_eq!(plan["engine"], "cpython-bake-v1");
        assert!(plan["data"]["WalkDefault"].is_object());
    }

    #[test]
    fn lowers_weapon_zone_and_first_person_camera() {
        let game = Game {
            name: "Fps".into(),
            root: node(
                "scene",
                vec![("title", Expr::String("FPS".into()))],
                vec![
                    node(
                        "weapon",
                        vec![
                            ("name", Expr::String("Rifle".into())),
                            ("fire_mode", Expr::Ident("hitscan".into())),
                            ("damage", Expr::Number("25".into())),
                            ("fire_rate", Expr::Number("8".into())),
                        ],
                        vec![],
                    ),
                    node(
                        "zone",
                        vec![
                            ("name", Expr::String("Courtyard".into())),
                            ("kind", Expr::Ident("outdoor".into())),
                        ],
                        vec![node(
                            "entity",
                            vec![
                                ("name", Expr::String("CoverWall".into())),
                                ("x", Expr::Number("2".into())),
                                ("yaw", Expr::Number("45".into())),
                            ],
                            vec![node(
                                "cover",
                                vec![("quality", Expr::Ident("high".into()))],
                                vec![],
                            )],
                        )],
                    ),
                    node(
                        "camera",
                        vec![("mode", Expr::Ident("first_person".into()))],
                        vec![],
                    ),
                    node(
                        "prefab",
                        vec![
                            ("name", Expr::String("Hero".into())),
                            ("pitch", Expr::Number("10".into())),
                        ],
                        vec![
                            node(
                                "movement",
                                vec![
                                    ("style", Expr::Ident("first_person".into())),
                                    ("jump_speed", Expr::Number("6".into())),
                                    ("sprint_mul", Expr::Number("1.5".into())),
                                ],
                                vec![],
                            ),
                            node(
                                "weapon",
                                vec![("ref", Expr::String("Rifle".into())), ("slot", Expr::Number("1".into()))],
                                vec![],
                            ),
                        ],
                    ),
                ],
            ),
            span: span(),
        };
        let v = lower_game(&game).unwrap();
        assert_eq!(v["camera"]["mode"], "first_person");
        assert_eq!(v["weapons"]["Rifle"]["fireMode"], "hitscan");
        assert_eq!(v["weapons"]["Rifle"]["damage"], 25.0);
        assert_eq!(v["zones"][0]["name"], "Courtyard");
        assert_eq!(v["zones"][0]["kind"], "outdoor");
        assert_eq!(v["zones"][0]["children"][0]["transform"]["yaw"], 45.0);
        assert_eq!(v["prefabs"]["Hero"]["transform"]["pitch"], 10.0);
        assert_eq!(v["prefabs"]["Hero"]["movement"]["style"], "first_person");
        assert_eq!(v["prefabs"]["Hero"]["movement"]["jumpSpeed"], 6.0);
        assert_eq!(v["prefabs"]["Hero"]["movement"]["sprintMul"], 1.5);
        assert_eq!(v["prefabs"]["Hero"]["weapon"]["ref"], "Rifle");

        let plan = bake_plan_from_manifest(&v);
        assert!(plan["weapons"]["Rifle"].is_object());
        assert_eq!(plan["zones"].as_array().unwrap().len(), 1);
    }
}
