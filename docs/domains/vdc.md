# Virtual Design and Construction

Architecture, engineering, and construction (AEC) is a go-to-market for Silc,
not the language's identity. In this repository that domain is Virtual Design
and Construction (VDC): software that connects an interactive project
environment with the dashboards, records, and automation around it.

The compiler stays generic ([ADR-016](../ADR-016-generic-kernel-domain-layers.md)).
Construction vocabulary lives in authored programs. The examples are
[`rfiChaseApp`](../../examples/domains/aec/rfiChaseApp/) and
[`vdcWalkthrough`](../../examples/domains/aec/vdcWalkthrough/).

## Why this shape

VDC software rarely fits inside one framework. A useful construction workflow
may combine an interactive project model, a field dashboard, persistent project
records, document extraction, and automation. Teams commonly bridge dedicated
3D engines, web stacks, scripts, services, and databases to deliver one
experience.

Silc is designed around that full shape:

- `scene::` declares browser-native real-time scenes. Gameplay nodes in `game::` are optional and are not required for a walkthrough.
- `ui::` declares operational interfaces synthesized for web and terminal.
- Pipeline operations declare ingestion, extraction, local AI, and persistence.
- `loop::` declares scheduled, approval-gated work such as an RFI chase.
- One compiler owns the generated Bun, CPython, and Go runtime beneath them.

A `scene` program and a dual-surface `app` program remain distinct roots.

Today Silc ships the generic primitives behind these workflows. Interactive
GLTF scenes, physics, cameras, dual-surface applications, CRUD resources,
document extraction, scraping, and local AI pipelines are available now.
Native BIM semantics, construction-platform connectors, multi-user
coordination, live sensor ingestion, and complete production digital twins are
directional use cases.

## Workflows this domain composes

- **Model walkthroughs and coordination environments.** Load a GLTF project asset into a WebGPU scene with cameras, lighting, collision, and overlays.
- **Digital-twin foundations.** Combine that spatial context with application state, persistence, and compiler-owned runtime services.
- **Site logistics and sequencing.** Compose entities and prefabs for equipment, access paths, temporary works, and phases.
- **Field and project operations.** Dashboards, inspection tools, issue lists, document ledgers, and local assistants use `ui::` and resources.
- **Safety and training simulations.** The real-time kernel hosts orientation and scenario rehearsal. Gameplay nodes are available when a scenario needs them.
- **RFI chase.** [`rfiChaseApp`](../../examples/domains/aec/rfiChaseApp/) is a weekday `loop`: overdue RFIs, a silclm draft, a project-manager approval, and a keyed reminder.

## Project walkthrough

This scene is the generic kernel applied to a project environment. Replace the
GLTF path with an exported model. `game::pawn` and `game::mode` are the
gameplay layer's way to possess a viewer; the rest of the tree is `scene::`.

```silc
#!/usr/bin/env silc
@version("0.7.0")

scene ProjectWalkthrough {
    scene::scene(
        :title("Project Walkthrough"),
        :renderer(webgpu),
        scene::asset(
            :name("project_model"),
            :path("public/assets/project.glb"),
            :kind(gltf)
        ),
        scene::entity(
            :name("ProjectModel"),
            scene::mesh(:asset("project_model"))
        ),
        scene::entity(
            :name("Ground"),
            :y(0),
            scene::mesh(:shape(plane), :size(80), :color("#aeb8ae")),
            scene::collider(:shape(plane), :size(80))
        ),
        scene::entity(
            :name("Sun"),
            scene::light(:kind(directional), :intensity(1.1))
        ),
        scene::prefab(
            :name("Viewer"),
            scene::mesh(:shape(capsule), :size(1.8)),
            scene::collider(:shape(capsule), :size(1.8)),
            scene::movement(:style(first_person), :speed(4.5)),
            game::pawn()
        ),
        scene::spawn(:prefab("Viewer"), :x(0), :y(1), :z(6), :as_pawn),
        game::mode(:id("walkthrough"), :possess("Viewer")),
        scene::controller(:scheme(wasd_mouse)),
        scene::camera(:mode(first_person), :follow(pawn)),
        scene::environment(
            :fog_density(0.002),
            :fog_color("#d8dde2"),
            :sky_color("#9fb6cc"),
            :exposure(1.0)
        )
    )
}
```

The runnable copy is [`examples/domains/aec/vdcWalkthrough`](../../examples/domains/aec/vdcWalkthrough/).
