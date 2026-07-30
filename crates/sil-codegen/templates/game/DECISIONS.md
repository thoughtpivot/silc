# Spec decisions (game kernel)

- **Kernel synthesis**: Godot entity tree + signals/groups; Unity prefabs, spawn overrides, and `game::data` assets; Unreal mode/pawn/controller ownership. Authors write `.silc` only.
- **Babylon adapter**: WebGPU-only (`WebGPUEngine`). Mesh/light/camera wrap Babylon nodes; no WebGL fallback.
- **CPython bake**: Always-on asset registry (`game_bake.json`) resolving data refs, collider hulls, spawns, signals, procedural PBR textures, kit metadata, and materials — not heightfield octaves.
- **Megastructure FPS vertical slice**: Havok physics + Recast nav + procedural PBR kit (compiler-owned, no CDN). First-person camera (`game::camera :mode(first_person)`). Async Silclm cognition for NPC `game::mind` refs.
- **Bun host**: Serves Vite `dist/`, `manifest.json`, baked assets; HTTP for settings/saves/telemetry/runs.
- **Go store**: Owns SQLite migrations for `game_saves`, `game_runs`, `game_events`, `game_settings` on the shared DB.
- **Physics**: Havok-backed capsule/box colliders from baked hulls (thin plane fallback for arena sandboxes).
- **Post**: Babylon `DefaultRenderingPipeline` subset (bloom/tonemap/sharpen/grain); SSR reserved.
- **Hot loop**: Stays in-browser; spine is bake/host/persist.
