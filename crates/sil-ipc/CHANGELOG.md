# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.

## [0.7.0](https://github.com/thoughtpivot/silc/releases/tag/sil-ipc-v0.7.0) - 2026-10-05

### Changed

- Require `@version("0.7.0")` in Silc source. Examples and fixtures declare it.
- Publish this crate from release-plz when its version changes.


## [0.6.0](https://github.com/thoughtpivot/silc/releases/tag/sil-ipc-v0.6.0) - 2026-10-04

### Added

- add native loop subject with Go kernel, MCP reads, and 0.5.0 bump
- land 0.4.0 pipeline, tensor, and contract-syntax work
- ground generated chat assistants
- add grounded inventory example app

### Fixed

- scope chat history by session
- complete chat runtime pipeline

### Other

- apply rustfmt to files that had drifted from the formatter
- Add declarative UI and local LLM portals
- Implement Silc-owned runtimes and runnable portal
- Implement Silc parse-route-emit MVP
- Scaffold SIL compiler and runtime architecture
