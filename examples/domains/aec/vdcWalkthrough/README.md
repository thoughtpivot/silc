# vdcWalkthrough

A project-environment scene on the generic `scene::` kernel. `game::pawn` and `game::mode` possess a viewer; the rest of the tree is kernel nodes.

Replace `public/assets/project.glb` with an exported GLTF model before running. The compiler does not ship a construction asset.

```bash
silc build main.silc
silc main.silc
```

WebGPU only (default port 18140). There is no terminal surface.

The domain write-up, including why this is not compiler vocabulary, is [docs/domains/vdc.md](../../../docs/domains/vdc.md). The weekday RFI loop is [`rfiChaseApp`](../rfiChaseApp/).
