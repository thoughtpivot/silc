# Spec deviations

- **SSR post stage**: Toggle present in manifest/overlay; Babylon 9.16 DefaultRenderingPipeline has no SSR hook — stage is reserved for a future custom pass.
- **Cloth simulation**: CPU Verlet with 32 particles per cloth region instead of GPU cloth — acceptable per SPEC; keeps WebGPU shader budget for terrain and snow.
- **Distant mountains**: Procedural ridgeline ring with matte fog tint instead of impostor billboards — cheaper, reads correctly at demo scale.
- **Spell water**: Ribbon mesh + particle spray instead of screen-space fluid — full SSFR exceeds 90 FPS budget on target hardware.
- **HDRI**: Procedural hemispherical sky + directional sun instead of vendored HDRI — no CDN/runtime fetch; vendored path documented in ASSETS.md.
- **4096² deformation RT**: Default 2048² R16F with 2 cm texels at 80 m extent — 4096 optional via manifest; 2048 fits warm-up time budget.
- **Shell fur**: 24 shells with procedural strand noise — below SPEC 20–40 range lower bound when GPU-bound; tunable via character manifest.
- **Polyglot spine (ADR-012)**: Bun serves WebGPU + UDS; CPython bakes height buffers at compile time into `public/baked/`; Go owns SQLite `game_saves` (+ Bun creates settings/telemetry tables on the same DB). Hot loop stays in-browser.
