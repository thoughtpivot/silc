# silc

ThoughtPivot [Silc](https://github.com/thoughtpivot/silc) compiler CLI.

Silc is an open-source loop language and compiler. Declare the loop; Silc picks
the runtime. This crate is the `silc` binary. It also compiles applications,
real-time scenes, and data pipelines from the same source language.
See [silc.thoughtpivot.com](https://silc.thoughtpivot.com).

Requires Rust 1.86 or newer.

```bash
cargo install --locked silc
silc --help
```

Library crates in this workspace (`sil-core`, `sil-lexer`, `sil-parser`,
`sil-router`, `sil-codegen`, `sil-ipc`, `sil-rlm`, `sil-training`) are published
so the CLI can be built from crates.io. Install the compiler with
`cargo install --locked silc`.
