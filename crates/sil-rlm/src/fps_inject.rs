//! Deterministic additive injectors for closed FPS megastructure authorship.
//!
//! Small local models truncate large `game::zone` trees. When a task names the
//! closed megastructure / hostile loadout, Assist grafts catalog-valid nodes
//! into the seed scene so `main.silc` remains assist-authored (not hand-edited).

/// Top surface of a `floor_2x2` slab. Props rest here, not at the slab base,
/// or short pieces end up buried under the walkable surface.
const FLOOR_TOP: f64 = 0.2;

/// Compact kit-piece entity helper.
fn kit_ent(name: &str, asset: &str, x: f64, y: f64, z: f64, yaw: f64) -> String {
    if yaw.abs() > 0.01 {
        format!(
            r##"game::entity(
            :name("{name}"),
            :x({x}),
            :y({y}),
            :z({z}),
            :yaw({yaw}),
            game::mesh(:asset("{asset}"), :size(1), :color("#808080")),
            game::collider(:shape(box), :size(1))
        )"##
        )
    } else {
        format!(
            r##"game::entity(
            :name("{name}"),
            :x({x}),
            :y({y}),
            :z({z}),
            game::mesh(:asset("{asset}"), :size(1), :color("#808080")),
            game::collider(:shape(box), :size(1))
        )"##
        )
    }
}

fn floor_grid(prefix: &str, ox: f64, oz: f64, nx: i32, nz: i32) -> Vec<String> {
    let mut out = Vec::new();
    for ix in 0..nx {
        for iz in 0..nz {
            let x = ox + ix as f64 * 2.0;
            let z = oz + iz as f64 * 2.0;
            out.push(kit_ent(
                &format!("{prefix}Floor_{ix}_{iz}"),
                "floor_2x2",
                x,
                0.0,
                z,
                0.0,
            ));
        }
    }
    out
}

/// Perimeter side of a room, named for the wall it replaces.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// −Z wall.
    N,
    /// +Z wall.
    S,
    /// −X wall.
    W,
    /// +X wall.
    E,
}

/// Walls hugging an `nx`×`nz` floor grid. Sides listed in `doors` get a
/// `wall_door_2x3` in their middle slot instead of a solid segment, so rooms
/// stay reachable; overlapping a door onto a wall would seal it shut.
fn wall_ring(prefix: &str, ox: f64, oz: f64, nx: i32, nz: i32, doors: &[Side]) -> Vec<String> {
    let mut out = Vec::new();
    let min_x = ox - 1.0;
    let max_x = ox + (nx - 1) as f64 * 2.0 + 1.0;
    let min_z = oz - 1.0;
    let max_z = oz + (nz - 1) as f64 * 2.0 + 1.0;
    let mid_x = nx / 2;
    let mid_z = nz / 2;

    for ix in 0..nx {
        let x = ox + ix as f64 * 2.0;
        for (side, z, tag) in [(Side::N, min_z, "N"), (Side::S, max_z, "S")] {
            let (name, asset) = if doors.contains(&side) && ix == mid_x {
                (format!("{prefix}Door{tag}"), "wall_door_2x3")
            } else {
                (format!("{prefix}Wall{tag}_{ix}"), "wall_2x3")
            };
            out.push(kit_ent(&name, asset, x, 0.0, z, 0.0));
        }
    }
    for iz in 0..nz {
        let z = oz + iz as f64 * 2.0;
        for (side, x, tag) in [(Side::W, min_x, "W"), (Side::E, max_x, "E")] {
            let (name, asset) = if doors.contains(&side) && iz == mid_z {
                (format!("{prefix}Door{tag}"), "wall_door_2x3")
            } else {
                (format!("{prefix}Wall{tag}_{iz}"), "wall_2x3")
            };
            out.push(kit_ent(&name, asset, x, 0.0, z, 90.0));
        }
    }
    out
}

fn zone(name: &str, kind: &str, children: &[String]) -> String {
    let mut body = String::new();
    for (i, child) in children.iter().enumerate() {
        let comma = if i + 1 < children.len() { "," } else { "" };
        let indented = child
            .lines()
            .map(|l| format!("            {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        body.push_str(&indented);
        body.push_str(comma);
        body.push('\n');
    }
    format!(
        "game::zone(\n        :name(\"{name}\"),\n        :kind({kind}),\n{body}        )"
    )
}

/// Build the five-room / four-walkway / outdoor megastructure zones.
pub fn megastructure_zone_nodes() -> Vec<String> {
    let mut zones = Vec::new();

    // Security lobby (−16,0)
    {
        // East door opens onto the glass bridge.
        let mut c = floor_grid("Lobby", -20.0, -4.0, 5, 5);
        c.extend(wall_ring("Lobby", -20.0, -4.0, 5, 5, &[Side::E]));
        c.push(kit_ent("LobbyDesk", "desk_1.6", -14.0, FLOOR_TOP, 0.0, 0.0));
        c.push(kit_ent("LobbyChair", "chair_0.5", -14.0, FLOOR_TOP, 1.2, 180.0));
        c.push(kit_ent("LobbyLocker", "locker_0.6", -18.0, FLOOR_TOP, -2.0, 0.0));
        c.push(kit_ent("LobbyCover", "cover_low", -12.0, FLOOR_TOP, 2.0, 0.0));
        c.push(
            r##"game::entity(
            :name("LobbyLamp"),
            :x(-14),
            :y(2.6),
            :z(0),
            game::light(:kind(spot), :intensity(1.4), :color("#cfe8ff"), :radius_m(10), :cast_shadows(true))
        )"##
            .into(),
        );
        zones.push(zone("SecurityLobby", "room", &c));
    }

    // Operations / control (0,0)
    {
        // Hub room: doors on all four sides reach every walkway.
        let mut c = floor_grid("Ops", -4.0, -4.0, 5, 5);
        c.extend(wall_ring(
            "Ops",
            -4.0,
            -4.0,
            5,
            5,
            &[Side::N, Side::S, Side::W, Side::E],
        ));
        // Keep the origin clear — the player pawn spawns there.
        c.push(kit_ent("OpsConsole", "desk_1.6", -3.0, FLOOR_TOP, 3.0, 0.0));
        c.push(kit_ent("OpsChair", "chair_0.5", -3.0, FLOOR_TOP, 1.6, 180.0));
        c.push(kit_ent("OpsCrate", "crate_1", 2.0, FLOOR_TOP, -2.0, 0.0));
        c.push(
            r##"game::entity(
            :name("OpsLamp"),
            :x(0),
            :y(2.8),
            :z(0),
            game::light(:kind(point), :intensity(1.2), :color("#a8d4ff"), :radius_m(12))
        )"##
            .into(),
        );
        zones.push(zone("OpsControl", "room", &c));
    }

    // Research lab (16,0)
    {
        // West door opens onto the industrial catwalk.
        let mut c = floor_grid("Lab", 12.0, -4.0, 5, 5);
        c.extend(wall_ring("Lab", 12.0, -4.0, 5, 5, &[Side::W]));
        c.push(kit_ent("LabTable1", "lab_table_2", 16.0, FLOOR_TOP, 0.0, 0.0));
        c.push(kit_ent("LabTable2", "lab_table_2", 18.0, FLOOR_TOP, -2.0, 90.0));
        c.push(kit_ent("LabLocker", "locker_0.6", 14.0, FLOOR_TOP, 2.0, 0.0));
        c.push(kit_ent("LabCover", "cover_high", 20.0, FLOOR_TOP, 0.0, 90.0));
        c.push(
            r##"game::entity(
            :name("LabLamp"),
            :x(16),
            :y(2.6),
            :z(0),
            game::light(:kind(spot), :intensity(1.3), :color("#d0ffe8"), :radius_m(10), :cast_shadows(true))
        )"##
            .into(),
        );
        zones.push(zone("ResearchLab", "room", &c));
    }

    // Barracks (−0,−16)
    {
        // South door opens onto the service corridor.
        let mut c = floor_grid("Barracks", -4.0, -20.0, 5, 5);
        c.extend(wall_ring("Barracks", -4.0, -20.0, 5, 5, &[Side::S]));
        c.push(kit_ent("Bunk1", "bunk_2", -2.0, FLOOR_TOP, -16.0, 0.0));
        c.push(kit_ent("Bunk2", "bunk_2", 2.0, FLOOR_TOP, -16.0, 0.0));
        c.push(kit_ent("BarracksLocker", "locker_0.6", -2.0, FLOOR_TOP, -18.0, 0.0));
        c.push(kit_ent("BarracksBench", "chair_0.5", 0.0, FLOOR_TOP, -14.0, 0.0));
        zones.push(zone("BarracksLounge", "room", &c));
    }

    // Reactor / assembly (0,16)
    {
        // North door reaches the skywalk, south door the rooftop courtyard.
        let mut c = floor_grid("Reactor", -4.0, 12.0, 5, 5);
        c.extend(wall_ring("Reactor", -4.0, 12.0, 5, 5, &[Side::N, Side::S]));
        c.push(kit_ent("ReactorColumn", "column_0.4", 0.0, FLOOR_TOP, 16.0, 0.0));
        c.push(kit_ent("ReactorCrate1", "crate_1", -2.0, FLOOR_TOP, 14.0, 0.0));
        c.push(kit_ent("ReactorCrate2", "crate_1", 2.0, FLOOR_TOP, 18.0, 15.0));
        c.push(kit_ent("ReactorCover", "cover_high", 0.0, FLOOR_TOP, 14.0, 0.0));
        c.push(
            r##"game::entity(
            :name("ReactorLamp"),
            :x(0),
            :y(3.0),
            :z(16),
            game::light(:kind(point), :intensity(1.6), :color("#ffd0a0"), :radius_m(14))
        )"##
            .into(),
        );
        zones.push(zone("ReactorHall", "room", &c));
    }

    // Walkways
    {
        let mut c = floor_grid("GlassBridge", -12.0, -2.0, 4, 2);
        c.push(kit_ent("GlassWin1", "wall_window_2x3", -10.0, 0.0, -3.0, 0.0));
        c.push(kit_ent("GlassWin2", "wall_window_2x3", -10.0, 0.0, 1.0, 0.0));
        zones.push(zone("GlassBridge", "walkway", &c));
    }
    {
        let mut c = floor_grid("Catwalk", 4.0, -2.0, 4, 2);
        c.push(kit_ent("CatwalkRail1", "cover_low", 6.0, FLOOR_TOP, -3.0, 0.0));
        c.push(kit_ent("CatwalkRail2", "cover_low", 8.0, FLOOR_TOP, 1.0, 0.0));
        zones.push(zone("IndustrialCatwalk", "walkway", &c));
    }
    {
        let mut c = floor_grid("Service", -2.0, -12.0, 2, 4);
        c.push(kit_ent("ServiceVent", "vent_1.2", 0.0, 2.5, -10.0, 0.0));
        // Hug the wall so the corridor stays walkable.
        c.push(kit_ent("ServiceCrate", "crate_1", -2.0, FLOOR_TOP, -8.0, 0.0));
        zones.push(zone("ServiceCorridor", "walkway", &c));
    }
    {
        let mut c = floor_grid("Skywalk", -2.0, 4.0, 2, 4);
        c.push(kit_ent("SkywalkWin", "wall_window_2x3", -3.0, 0.0, 8.0, 90.0));
        zones.push(zone("ExteriorSkywalk", "walkway", &c));
    }

    // Outdoor courtyard / roof
    {
        let mut c = floor_grid("Court", -6.0, 20.0, 6, 4);
        c.push(kit_ent("Planter1", "planter_1", -4.0, FLOOR_TOP, 22.0, 0.0));
        c.push(kit_ent("Planter2", "planter_1", 4.0, FLOOR_TOP, 22.0, 0.0));
        c.push(kit_ent("VentUnit1", "vent_1.2", 0.0, FLOOR_TOP, 26.0, 0.0));
        c.push(kit_ent("CourtCover1", "cover_low", -2.0, FLOOR_TOP, 24.0, 0.0));
        c.push(kit_ent("CourtCover2", "cover_high", 2.0, FLOOR_TOP, 24.0, 90.0));
        c.push(
            r##"game::entity(
            :name("CourtSunFill"),
            :x(0),
            :y(8),
            :z(24),
            game::light(:kind(directional), :intensity(0.35), :color("#fff4e0"))
        )"##
            .into(),
        );
        zones.push(zone("RooftopCourtyard", "outdoor", &c));
    }

    zones
}

/// The opening hostile wave on its own, for rebuilds that keep the prefabs.
pub fn hostile_encounter_wave() -> String {
    r##"game::encounter(
            :id("wave_alpha"),
            :wave(1),
            game::spawn(:prefab("Suppressor"), :x(-17), :y(1), :z(2)),
            game::spawn(:prefab("Flanker"), :x(13), :y(1), :z(-2)),
            game::spawn(:prefab("Breacher"), :x(-2), :y(1), :z(18)),
            game::spawn(:prefab("Suppressor"), :x(0), :y(1), :z(22))
        )"##
    .into()
}

/// Hostile archetype prefabs + mind data + encounter wave.
pub fn hostile_encounter_nodes() -> Vec<String> {
    vec![
        r##"game::data(:name("SuppressorMind"), :persona("Suppressor — hold angles, controlled bursts."), :aggression(0.65), :morale(0.7), :cadence_s(4))"##.into(),
        r##"game::data(:name("FlankerMind"), :persona("Flanker — cut left, keep moving."), :aggression(0.75), :morale(0.6), :cadence_s(3.5))"##.into(),
        r##"game::data(:name("BreacherMind"), :persona("Breacher — push hard, close distance."), :aggression(0.9), :morale(0.55), :cadence_s(3))"##.into(),
        r##"game::data(:name("HostileWalk"), :speed(3.4))"##.into(),
        r##"game::prefab(
            :name("Suppressor"),
            game::mesh(:shape(capsule), :size(1.8), :color("#c45c5c")),
            game::collider(:shape(capsule), :size(1.8)),
            game::movement(:style(walk), :ref("HostileWalk")),
            game::attribute(:name("health"), :value(100), :max(100)),
            game::npc(:archetype(suppressor), :faction(hostile)),
            game::perception(:sight_m(28), :hear_m(14), :fov_deg(110)),
            game::behavior(:tree(patrol_combat), :default_tactic(suppress)),
            game::mind(:ref("SuppressorMind"), :cadence_s(4)),
            game::nav_agent(:radius(0.35), :height(1.8), :max_speed(3.4)),
            game::group(:name("hostiles"))
        )"##.into(),
        r##"game::prefab(
            :name("Flanker"),
            game::mesh(:shape(capsule), :size(1.75), :color("#d4a24c")),
            game::collider(:shape(capsule), :size(1.75)),
            game::movement(:style(walk), :ref("HostileWalk")),
            game::attribute(:name("health"), :value(90), :max(90)),
            game::npc(:archetype(flanker), :faction(hostile)),
            game::perception(:sight_m(30), :hear_m(16), :fov_deg(120)),
            game::behavior(:tree(patrol_combat), :default_tactic(flank)),
            game::mind(:ref("FlankerMind"), :cadence_s(3.5)),
            game::nav_agent(:radius(0.32), :height(1.75), :max_speed(3.8)),
            game::group(:name("hostiles"))
        )"##.into(),
        r##"game::prefab(
            :name("Breacher"),
            game::mesh(:shape(capsule), :size(1.9), :color("#7a4cc4")),
            game::collider(:shape(capsule), :size(1.9)),
            game::movement(:style(walk), :ref("HostileWalk")),
            game::attribute(:name("health"), :value(120), :max(120)),
            game::npc(:archetype(breacher), :faction(hostile)),
            game::perception(:sight_m(24), :hear_m(12), :fov_deg(100)),
            game::behavior(:tree(patrol_combat), :default_tactic(push)),
            game::mind(:ref("BreacherMind"), :cadence_s(3)),
            game::nav_agent(:radius(0.4), :height(1.9), :max_speed(3.2)),
            game::group(:name("hostiles"))
        )"##.into(),
        r##"game::encounter(
            :id("wave_alpha"),
            :wave(1),
            game::spawn(:prefab("Suppressor"), :x(-17), :y(1), :z(2)),
            game::spawn(:prefab("Flanker"), :x(13), :y(1), :z(-2)),
            game::spawn(:prefab("Breacher"), :x(-2), :y(1), :z(18)),
            game::spawn(:prefab("Suppressor"), :x(0), :y(1), :z(22))
        )"##.into(),
        r##"game::objective(:id("clear_hostiles"), :kind(clear_hostiles), :target("hostiles"))"##.into(),
    ]
}

pub fn wants_megastructure(task: &str) -> bool {
    let lower = task.to_lowercase();
    (lower.contains("megastructure")
        || lower.contains("five room")
        || lower.contains("5 room")
        || lower.contains("security lobby")
        || lower.contains("rooftop"))
        && (lower.contains("zone")
            || lower.contains("room")
            || lower.contains("walkway")
            || lower.contains("courtyard")
            || lower.contains("furniture")
            || lower.contains("kit"))
}

/// True when the task asks to regenerate an already-authored megastructure,
/// for example to reconnect rooms whose doorways are walled shut.
pub fn wants_megastructure_rebuild(task: &str) -> bool {
    let lower = task.to_lowercase();
    (lower.contains("rebuild")
        || lower.contains("regenerate")
        || lower.contains("reconnect")
        || lower.contains("connect"))
        && (lower.contains("megastructure")
            || lower.contains("zone")
            || lower.contains("room")
            || lower.contains("door"))
}

pub fn wants_hostiles(task: &str) -> bool {
    let lower = task.to_lowercase();
    (lower.contains("suppressor") && lower.contains("flanker") && lower.contains("breacher"))
        || (lower.contains("hostile") && lower.contains("encounter"))
        || (lower.contains("npc") && lower.contains("mind") && lower.contains("encounter"))
}

pub fn wants_strip_neon(task: &str) -> bool {
    let lower = task.to_lowercase();
    lower.contains("remove neon")
        || lower.contains("strip neon")
        || (lower.contains("remove") && lower.contains("neonsphere"))
}
