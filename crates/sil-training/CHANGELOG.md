# Changelog

All notable changes to Silc are documented here.
Silc remains pre-1.0; this project follows SemVer 0.x with Conventional Commits.

## [0.8.0](https://github.com/thoughtpivot/silc/compare/sil-training-v0.7.1...sil-training-v0.8.0) - 2026-11-26

### Other

- release Silc 0.8.0 (THO-551) ([#34](https://github.com/thoughtpivot/silc/pull/34))
- derive release text from the version seam, drop stale "Silc 0.7.0" strings ([#31](https://github.com/thoughtpivot/silc/pull/31))
- match @version on major.minor, not the exact compiler version ([#27](https://github.com/thoughtpivot/silc/pull/27))

## [0.7.1](https://github.com/thoughtpivot/silc/compare/sil-training-v0.7.0...sil-training-v0.7.1) - 2026-11-25

### Other

- prepare the 0.7.0 crates.io release

## [0.7.0](https://github.com/thoughtpivot/silc/releases/tag/sil-training-v0.7.0) - 2026-10-05

### Changed

- Require `@version("0.7.0")` in Silc source. Examples and fixtures declare it.
- Publish this crate from release-plz when its version changes.


## [0.6.0](https://github.com/thoughtpivot/silc/releases/tag/sil-training-v0.6.0) - 2026-10-04

### Added

- add native loop subject with Go kernel, MCP reads, and 0.5.0 bump
- implement document extraction capabilities and file input support
- land 0.4.0 pipeline, tensor, and contract-syntax work
- add recursive silclm assist scaffold
- establish Silc as an independent intent language
- establish silclm training foundation

### Other

- Release Silc 0.6.0: scene kernel, generic positioning, example layout
- apply rustfmt to files that had drifted from the formatter
