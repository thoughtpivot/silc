# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.

## [0.8.0](https://github.com/thoughtpivot/silc/compare/sil-parser-v0.7.1...sil-parser-v0.8.0) - 2026-11-26

### Added

- *(loop)* THO-553 nearest-match loop::find ([#30](https://github.com/thoughtpivot/silc/pull/30))
- *(loop)* type-check Contract.new field values (THO-578) ([#23](https://github.com/thoughtpivot/silc/pull/23))
- *(loop)* add loop::fallback (THO-559 Part 1) ([#25](https://github.com/thoughtpivot/silc/pull/25))

### Other

- release Silc 0.8.0 (THO-551) ([#34](https://github.com/thoughtpivot/silc/pull/34))
- scrape::site, render, select, and text::score in loops ([#33](https://github.com/thoughtpivot/silc/pull/33))
- embedding step in loop::let ([#32](https://github.com/thoughtpivot/silc/pull/32))
- derive release text from the version seam, drop stale "Silc 0.7.0" strings ([#31](https://github.com/thoughtpivot/silc/pull/31))
- match @version on major.minor, not the exact compiler version ([#27](https://github.com/thoughtpivot/silc/pull/27))
- :select and :into on scrape::page loop reads ([#26](https://github.com/thoughtpivot/silc/pull/26))
- loop::ask multi-value :from ([#22](https://github.com/thoughtpivot/silc/pull/22))
- embedding store and query nearest ([#28](https://github.com/thoughtpivot/silc/pull/28))

## [0.7.1](https://github.com/thoughtpivot/silc/compare/sil-parser-v0.7.0...sil-parser-v0.7.1) - 2026-11-25

### Other

- prepare the 0.7.0 crates.io release

## [0.7.0](https://github.com/thoughtpivot/silc/releases/tag/sil-parser-v0.7.0) - 2026-10-05

### Changed

- Require `@version("0.7.0")` in Silc source. Examples and fixtures declare it.
- Publish this crate from release-plz when its version changes.


## [0.6.0](https://github.com/thoughtpivot/silc/releases/tag/sil-parser-v0.6.0) - 2026-10-04

### Added

- *(loop)* introduce loop command functionality with CLI support
- add native loop subject with Go kernel, MCP reads, and 0.5.0 bump
- cinematic FPS megastructure with modular kit, physics, and AI cognition
- introduce game support with WebGPU integration
- add Silc LSP semantic hover and editor install docs
- add blog example with resource seeds and table select
- establish Silc as an independent intent language
- release Silc 0.2.0 component model

### Other

- Release Silc 0.6.0: scene kernel, generic positioning, example layout
- Unify node specs, register operations, and retire sink/serve
- say operation and option, not op and prop
- Add declarative UI and local LLM portals
- Implement Silc parse-route-emit MVP
- Scaffold SIL compiler and runtime architecture
