# ADR-013: Procedural Asset Generation

- **Status:** Accepted
- **Date:** 2026-07-31
- **Related:** [ADR-012](ADR-012-webgpu-game-subject.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)
- **Canonical:** [`crates/sil-core/src/game.rs`](../crates/sil-core/src/game.rs),
  [`game_lower`](../crates/sil-codegen/src/game_lower.rs),
  [`game_bake_worker.py`](../crates/sil-codegen/templates/game_bake_worker.py)

## Context

Game authors need sprite assets (character animations, enemies, items, tiles) but
creating pixel art requires specialized skills and external tools. The existing
`game::asset` node imports external files (GLTF, textures, audio) but cannot
generate assets procedurally.

ADR-012 established the bake pipeline (`game_bake_worker.py`) for compile-time
asset processing. This ADR extends that pipeline with procedural generation.

## Decision

1. **New `game::generate` node.** Authors declare procedurally generated assets
   with `:type`, `:preset`, `:style`, `:palette`, and `:animations` props.
   Distinct from `game::asset` which imports external files.

2. **Archetype + style separation.** Presets define *what* to generate
   (`character`, `enemy`, `item`, `tile`, `effect`). Styles define *how* it
   looks (`pixel_8`, `pixel_16`, `pixel_32`, `flat`, `outline`).

3. **Author-specified animations.** The `:animations` prop lists which animation
   clips to generate. This supports platformers (`idle, walk, jump, fall`),
   top-down games (`walk_up, walk_down, walk_left, walk_right`), and any other
   game type without hardcoding animation sets.

4. **Bake pipeline extension.** `game_bake_worker.py` gains a generator dispatch
   that produces PNG sprite sheets and JSON animation metadata to `baked/sprites/`.

5. **Palette customization.** The `:palette` prop maps semantic color names to
   hex values, allowing the same preset to produce visually distinct sprites.

6. **Export to public/assets.** The `:export(true)` prop copies generated assets
   to `public/assets/` alongside `baked/sprites/`, enabling external tool editing
   (Aseprite, etc.). Manual edits in `public/assets/` take precedence over
   regeneration on subsequent builds.

## Syntax

```silc
// Platformer character
game::generate(:type(sprite), :name("hero"),
    :preset(character),
    :style(pixel_16),
    :palette(primary: "#E52521", secondary: "#0000AA", skin: "#FFCC99"),
    :animations(idle, walk, jump, fall, hurt, dead)
)

// Top-down RPG character
game::generate(:type(sprite), :name("wizard"),
    :preset(character),
    :style(pixel_16),
    :palette(robe: "#4B0082", skin: "#FFCC99"),
    :animations(idle, walk_up, walk_down, walk_left, walk_right, cast, hurt, dead)
)

// Collectible item
game::generate(:type(sprite), :name("coin"),
    :preset(item),
    :style(pixel_16),
    :palette(gold: "#FFD700"),
    :animations(idle, collected)
)
```

## Catalog Node

```rust
GameNodeSpec {
    name: "generate",
    description: "Procedurally generated asset created at build time.",
    props: &[
        gp_closed("type", ..., &["sprite", "texture", "material"]),
        gp("name", ..., "Generated asset identity."),
        gp_closed("preset", ..., &["character", "enemy", "item", "tile", "effect"]),
        gp_closed("style", ..., &["pixel_8", "pixel_16", "pixel_32", "flat", "outline"]),
        gp("frame_size", ..., "Frame dimensions in pixels."),
        gp("palette", ..., "Color palette map."),
        gp("animations", ..., "Animation names to generate."),
        gp("export", ..., "When true, also export to public/assets/."),
    ],
}
```

## Bake Pipeline

```
main.silc
    ↓ (parse)
game_lower.rs
    ↓ (collect game::generate nodes)
bake_plan.json { "generatedAssets": [...] }
    ↓ (CPython bake)
game_bake_worker.py
    ↓ (generator dispatch)
baked/sprites/{name}.png + {name}.json
    ↓ (if :export(true))
public/assets/{name}.png + {name}.json
```

## Asset Resolution Priority

The runtime sprite loader checks paths in this order:
1. `baked/sprites/{name}.png` - Generated assets (always regenerated)
2. `public/assets/{name}.png` - Manual/exported assets (preserved across builds)

If a manually edited file exists in `public/assets/`, it is copied to `baked/sprites/`
and used instead of regenerating. This allows artists to refine generated sprites
while keeping the declarative `game::generate` node in the source.

## Presets (Phase 1)

| Preset | Description | Default Animations |
|--------|-------------|-------------------|
| `character` | Bipedal or player-controlled entity | idle, walk, hurt, dead |
| `enemy` | Hostile or NPC entity | idle, walk, hurt, dead |
| `item` | Collectible or pickup | idle, collected |
| `tile` | Environmental tile or block | idle, active, broken |
| `effect` | Particle or VFX sprite | play |

## Styles (Phase 1)

| Style | Description |
|-------|-------------|
| `pixel_8` | 8x8 to 16x16 retro pixel art (NES-era) |
| `pixel_16` | 16x16 to 32x32 pixel art (SNES-era) |
| `pixel_32` | 32x32 to 64x64 detailed pixel art |
| `flat` | Solid color shapes with minimal detail |
| `outline` | Outlined shapes with fill |

## Consequences

- Authors can create game sprites without external art tools.
- Palette customization enables visual variety from the same preset.
- The generator architecture supports future expansion (procedural textures,
  shader-based generation, pixel-level control) without API changes.
- `game::asset` continues to work unchanged for external file imports.

## Future Expansion

- **Phase 2:** Advanced presets with `:scale`, `:detail`, `:effects`
- **Phase 3:** Procedural algorithms (perlin noise, cellular automata)
- **Phase 4:** Pixel-level control with `:frames` prop
- **Phase 5:** Shader-based generation
