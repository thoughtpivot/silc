# MegaStructure FPS (ADR-012)

WebGPU-only `game::` cinematic FPS vertical slice proving the Silc game kernel:

- **Godot:** nested zone/entity trees, signals, groups
- **Unity:** prefabs, data assets, kit mesh assets, spawn overrides
- **Unreal:** mode / pawn / controller / first-person weapons / encounters
- **Spine:** CPython bake (PBR kit + textures), Bun HTTP host, Go SQLite, optional Silclm cognition

## Level

Five rooms (Security Lobby, Ops Control, Research Lab, Barracks Lounge, Reactor Hall), four walkways (Glass Bridge, Industrial Catwalk, Service Corridor, Exterior Skywalk), and a Rooftop Courtyard. Furniture uses the compiler-owned modular kit (`floor_2x2`, `wall_*`, desks, crates, cover, …).

## Weapons

1. **Vanguard AR** — hitscan automatic  
2. **Breach-12** — pellet shotgun  
3. **Arc Carbine** — plasma projectile + splash  
4. **Longshot Railgun** — charged penetrating beam  

## Hostiles

Suppressor / Flanker / Breacher archetypes with deterministic BT + waypoint nav, plus async Silclm `game::mind` directives (falls back when the model is offline).

```bash
silc build main.silc
silc main.silc
# → http://127.0.0.1:18140 (WebGPU required)
```

Controls: WASD move · mouse look (click to capture) · LMB fire · Space jump · Shift sprint · R reload · `1`–`4` weapons · `F1` overlay.
