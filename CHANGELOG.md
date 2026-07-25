# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.

## [0.2.0] - 2026-07-25

### Breaking

- Removed profile-selected applications (`PortalKind` Feedback / LlmChat / Inventory).
- Removed `is view` and Contract-left-of-`ui::web` portal binding.
- Removed the separate repository-root component-source `stdlib/` directory and
  resolver. Out-of-box UI capability is compiler-owned in the primitive catalog
  and codegen templates; author components remain in application source.
- Replaced profile worker templates with unified Bun / Python / Go workers.
- Replaced the previous example suite with dual-surface component-driven apps.

### Added

- Author-defined `is component` with typed props, reactive state, slots, emits,
  handlers, queries, and render templates.
- `is resource` query/mutation methods backed by Contracts and SQLite.
- `is app` routes plus required dual-surface `serve()` (`ui::web` + `ui::terminal`).
- Expression language for props, conditionals, collection rendering, and handlers.
- Manifest v3 capabilities / actions / surfaces / processor metadata.
- Semantic release automation via Conventional Commits and `release-plz`.
- Expanded compiler-owned UI primitives and dual-surface codegen templates.

### Examples

- `examples/components.silc`
- `examples/scored_form.silc`
- `examples/chat_assistant.silc`
- `examples/shopping_app.silc`
- `examples/http_api.silc`
- `examples/data_pipeline.silc`
