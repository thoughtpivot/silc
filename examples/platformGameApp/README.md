# Super Mario Bros Clone (Platformer)

A comprehensive platformer demonstrating the Silc game engine's side-scrolling
capabilities with reusable 2D game primitives. Features a Mario-style World 1-1
layout with enemies, collectibles, and interactive blocks.

## Controls

- **Arrow Left / A** — Move left
- **Arrow Right / D** — Move right
- **Space** — Jump (only when grounded)
- **F1** — Toggle debug overlay

## Run

```bash
silc build main.silc
silc main.silc
# → http://127.0.0.1:18140 (WebGPU required)
```

## Features

### Movement & Camera
- `game::movement :style(platformer)` — horizontal movement with jump, Z locked
- `game::camera :mode(side_scroll)` — orthographic camera looking at XY plane
- `game::controller :scheme(arrows_jump)` — arrow keys + space input scheme

### Enemies
- **Goombas** — Walk back and forth, can be stomped
- **Koopas** — Slower patrol, can be stomped

### Collectibles
- **Coins** — Floating gold spheres that add to score
- **Mushrooms** — Power-up collectibles

### Interactive Blocks
- **Question Blocks** — Bump from below to spawn coins
- **Brick Blocks** — Breakable by head-bump

### Level Elements
- **Platforms** — Floating platforms at various heights
- **Pipes** — Classic green pipes (decorative)
- **Staircase** — Steps leading to the flag pole
- **Flag Pole** — Level completion trigger

## Platformer Primitives

This example uses the following reusable game nodes:

| Node | Purpose |
|------|---------|
| `game::sprite` | Billboard sprites from texture atlases |
| `game::tilemap` | Tile-based level geometry |
| `game::collectible` | Overlap-based pickups (coin, gem, health, powerup, key) |
| `game::interactable` | Bump/hit responsive blocks (breakable, bumpable, switchable) |
| `game::patrol` | 2D enemy AI patterns (walk_reverse, walk_fall, follow, flee) |
| `game::warp` | Teleport triggers with direction requirements |
| `game::level_end` | Level completion trigger |
