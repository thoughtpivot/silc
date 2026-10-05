# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.

## [0.7.0](https://github.com/thoughtpivot/silc/releases/tag/sil-codegen-v0.7.0) - 2026-10-05

### Changed

- Require `@version("0.7.0")` in Silc source. Examples and fixtures declare it.
- Publish this crate from release-plz when its version changes.


## [0.6.0](https://github.com/thoughtpivot/silc/releases/tag/sil-codegen-v0.6.0) - 2026-10-04

### Added

- *(loop)* introduce loop command functionality with CLI support
- *(examples)* add CLI-driven whatToDoTodayApp loop; parse first JSON object in loop asks
- *(oneThingApp)* record one action per run; bump editor extension to 0.3.0
- *(ui)* keep resource queries live with focus refetch and visible polling
- add native loop subject with Go kernel, MCP reads, and 0.5.0 bump
- add environment sky layers, configurable HUD scores, and trigger collectibles
- expand game functionality with new components and asset management
- introduce platformer support with new game components and systems
- cinematic FPS megastructure with modular kit, physics, and AI cognition
- introduce game support with WebGPU integration
- enhance UI alert component with auto-dismiss functionality
- implement document extraction capabilities and file input support
- add Silc LSP semantic hover and editor install docs
- land 0.4.0 pipeline, tensor, and contract-syntax work
- add blog example with resource seeds and table select
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

- *(THO-120)* accept blank strings as valid Str in loop::ask
- *(ui)* keep single-token table cells and headers on one line
- stop terminal surface crashes on conditionals and route swaps
- accept bare domains in scraper app
- make generated page layouts scroll independently
- restore chat Enter-to-send
- harden multi-session chat UX
- scope chat history by session
- complete chat runtime pipeline
- align web asset paths and chat props

### Other

- Fix text::score pass-through in the Python worker; make the scored_form CI step hit the app it starts
- Release Silc 0.6.0: scene kernel, generic positioning, example layout
- Unify node specs, register operations, and retire sink/serve
- apply rustfmt to files that had drifted from the formatter
- Add grocery inventory portal with AI search
- Add declarative UI and local LLM portals
- Apply ThoughtPivot branding to generated apps
- Add declarative HTTP services
- Adopt React UI substrate and runtime strengths
- Add declarative Vue web UI substrate
- Implement Silc-owned runtimes and runnable portal
- Implement Silc parse-route-emit MVP
- Scaffold SIL compiler and runtime architecture
