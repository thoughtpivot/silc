# Assets

Procedural-first runtime. Terrain height, snow normals, deformation buffers, robe geometry, fur shells, and spell VFX are generated at runtime — no binary meshes or textures are required for the default demo.

## Vendored assets path

When the Silc compiler or author supplies hand-authored assets, place them under:

```
public/assets/
  hdri/       — CC0 environment maps (Poly Haven)
  textures/   — CC0 snow/ice detail scans (ambientCG, Poly Haven)
  models/     — optional character rig overrides
```

Document every third-party file here with source URL and licence before commit.

## Current status

No vendored binary assets ship in the template. The snow shader uses procedural multi-scale normals; atmosphere uses a procedural sky dome. Replace with vendored HDRIs and detail normals when art tuning demands it.
