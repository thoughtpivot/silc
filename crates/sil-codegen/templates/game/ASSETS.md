# Game assets

Compiler-owned. No CDN fetches.

## Runtime manifests

- `public/manifest.json` — lowered scene graph (written per program at emit)
- `public/baked/game_bake.json` — CPython bake output (data, prefabs, colliders, signals, textures, kit, materials, zones, weapons, navHints, mindRefs, environment)
- `public/baked/kit.json` — modular megastructure kit metadata (2 m grid)
- `public/baked/materials.json` — PBR material → texture map registry
- Kit piece names (`floor_2x2`, `wall_2x3`, `desk_1.6`, …) are valid mesh `:asset` refs; the bake worker auto-registers them as `kit:<name>` without CDN fetches.
- `public/baked/textures/*.png` — procedural PBR maps generated at compile time

## Procedural kit license

All kit geometry metadata and PBR textures are **Silc-generated**, **compiler-owned**, and baked locally at `silc build`. They are not fetched from a CDN and carry no third-party asset license. Authors reference kit pieces and materials by name in `.silc`; the runtime assembles Babylon primitives from baked dimensions and textures.

## Kit pieces (2 m grid)

| Piece | Kind | Size (m) | Material |
| --- | --- | --- | --- |
| `floor_2x2` | floor | 2 × 0.2 × 2 | concrete |
| `wall_2x3` | wall | 2 × 3 × 0.2 | concrete |
| `wall_door_2x3` | wall_door | 2 × 3 × 0.2 (1 × 2.1 opening) | concrete |
| `wall_window_2x3` | wall_window | 2 × 3 × 0.2 (1.2 × 0.8 opening) | concrete |
| `column_0.4` | column | 0.4 × 3 × 0.4 | concrete |
| `crate_1` | prop | 1 × 1 × 1 | wood |
| `desk_1.6` | prop | 1.6 × 0.75 × 0.8 | wood |
| `chair_0.5` | prop | 0.5 × 0.9 × 0.5 | metal |
| `locker_0.6` | prop | 0.6 × 2 × 0.5 | metal |
| `lab_table_2` | prop | 2 × 0.85 × 0.9 | metal |
| `bunk_2` | prop | 2 × 1.6 × 1.0 | metal |
| `planter_1` | prop | 1 × 0.6 × 1 | concrete |
| `vent_1.2` | prop | 1.2 × 0.15 × 0.6 | metal |
| `cover_low` | cover (low) | 2 × 1.2 × 0.6 | concrete |
| `cover_high` | cover (high) | 2 × 2.4 × 0.6 | concrete |
| `door_frame` | door_frame | 1 × 2.1 × 0.15 | metal |

## Procedural textures

`concrete_albedo`, `concrete_normal`, `concrete_roughness`, `metal_albedo`, `metal_roughness`, `wood_albedo`, `glass_albedo`, `plaster_albedo`, `asphalt_albedo`, `emissive_strip`.

## Fallback meshes

Primitive meshes are Babylon `MeshBuilder` at runtime (plane/box/capsule/sphere) when no kit piece or external asset is bound. Particle cues use a 1×1 data-URI texture (no external image packs in v1).
