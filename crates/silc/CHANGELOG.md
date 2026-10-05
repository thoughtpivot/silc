# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.

## [0.7.0](https://github.com/thoughtpivot/silc/releases/tag/silc-v0.7.0) - 2026-10-05

### Changed

- Require `@version("0.7.0")` in Silc source. Examples and fixtures declare it.
- Publish this crate from release-plz when its version changes.


## [0.6.0](https://github.com/thoughtpivot/silc/releases/tag/silc-v0.6.0) - 2026-10-04

### Added

- *(loop)* introduce loop command functionality with CLI support
- *(examples)* add CLI-driven whatToDoTodayApp loop; parse first JSON object in loop asks
- add native loop subject with Go kernel, MCP reads, and 0.5.0 bump
- expand game functionality with new components and asset management
- introduce platformer support with new game components and systems
- cinematic FPS megastructure with modular kit, physics, and AI cognition
- introduce game support with WebGPU integration
- enhance UI alert component with auto-dismiss functionality
- implement document extraction capabilities and file input support
- add Silc LSP semantic hover and editor install docs
- enhance terminal support and documentation for silc
- land 0.4.0 pipeline, tensor, and contract-syntax work
- add blog example with resource seeds and table select
- upgrade silclm to Llama 3.2 3B
- add recursive silclm assist scaffold
- establish Silc as an independent intent language
- catalog scrapes with SilcLM summaries
- add first-class scrape primitives
- add dual-surface UI primitive kit
- add searchable inventory data tables
- ground generated chat assistants
- add grounded inventory example app
- establish silclm training foundation
- release Silc 0.2.0 component model

### Fixed

- drop unreleased pipeline/tensor supervisor code from previous commit
- resolve OpenTUI terminal entry relative to worker cwd
- support long silclm downloads
- harden recursive assist tool loop
- attach OpenTUI from terminal sessions
- harden multi-session chat UX
- scope chat history by session
- complete chat runtime pipeline
- align web asset paths and chat props

### Other

- Fix text::score pass-through in the Python worker; make the scored_form CI step hit the app it starts
- Release Silc 0.6.0: scene kernel, generic positioning, example layout
- Unify node specs, register operations, and retire sink/serve
- cli_init asserts the catalog heading, not a hard-coded count
- apply rustfmt to files that had drifted from the formatter
- enforce vocabulary, generated enums, and count-free prose
- fix catalog-count, version, and module-map drift; adopt option vocabulary
- recast README as Silc white paper
- align ADRs with Silc 0.4 architecture
- format terminal fallback message
- publish complete Silc API contract
- refresh Silc 0.2.0 init guidance
- Add grocery inventory portal with AI search
- Add declarative UI and local LLM portals
- Add declarative HTTP services
- Adopt React UI substrate and runtime strengths
- Add declarative Vue web UI substrate
- Implement Silc-owned runtimes and runnable portal
- Add Silc project initialization tooling
- Implement Silc parse-route-emit MVP
- Scaffold SIL compiler and runtime architecture
