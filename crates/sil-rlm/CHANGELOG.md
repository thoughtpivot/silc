# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.

## [0.8.0](https://github.com/thoughtpivot/silc/compare/sil-rlm-v0.7.1...sil-rlm-v0.8.0) - 2026-11-26

### Added

- *(loop)* add loop::fallback (THO-559 Part 1) ([#25](https://github.com/thoughtpivot/silc/pull/25))

### Fixed

- build model programs on gcc-only hosts, bump llama-cpp-python to 0.3.38 (THO-560) ([#24](https://github.com/thoughtpivot/silc/pull/24))

### Other

- release Silc 0.8.0 (THO-551) ([#34](https://github.com/thoughtpivot/silc/pull/34))
- scrape::site, render, select, and text::score in loops ([#33](https://github.com/thoughtpivot/silc/pull/33))
- embedding step in loop::let ([#32](https://github.com/thoughtpivot/silc/pull/32))
- derive release text from the version seam, drop stale "Silc 0.7.0" strings ([#31](https://github.com/thoughtpivot/silc/pull/31))
- match @version on major.minor, not the exact compiler version ([#27](https://github.com/thoughtpivot/silc/pull/27))
- :select and :into on scrape::page loop reads ([#26](https://github.com/thoughtpivot/silc/pull/26))
- loop::ask multi-value :from ([#22](https://github.com/thoughtpivot/silc/pull/22))

## [0.7.1](https://github.com/thoughtpivot/silc/compare/sil-rlm-v0.6.0...sil-rlm-v0.7.1) - 2026-11-25

### Added

- release 0.7.0 and update examples to the new source version

### Other

- prepare the 0.7.0 crates.io release

## [0.7.0](https://github.com/thoughtpivot/silc/releases/tag/sil-rlm-v0.7.0) - 2026-10-05

### Changed

- Require `@version("0.7.0")` in Silc source. Examples and fixtures declare it.
- Publish this crate from release-plz when its version changes.


## [0.6.0](https://github.com/thoughtpivot/silc/releases/tag/sil-rlm-v0.6.0) - 2026-10-04

### Added

- add native loop subject with Go kernel, MCP reads, and 0.5.0 bump
- introduce platformer support with new game components and systems
- cinematic FPS megastructure with modular kit, physics, and AI cognition
- introduce game support with WebGPU integration
- enhance UI alert component with auto-dismiss functionality
- implement document extraction capabilities and file input support
- land 0.4.0 pipeline, tensor, and contract-syntax work
- add blog example with resource seeds and table select
- add recursive silclm assist scaffold

### Fixed

- support long silclm downloads
- harden recursive assist tool loop

### Other

- Release Silc 0.6.0: scene kernel, generic positioning, example layout
- say operation and option, not op and prop
- lead the README with VDC while keeping Silc generic
