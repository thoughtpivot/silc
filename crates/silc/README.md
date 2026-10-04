# silc

ThoughtPivot [Silc](https://github.com/thoughtpivot/silc) compiler CLI.

Silc is an intent language and compiler for applications, real-time scenes,
pipelines, and loops. This crate is the `silc` binary.

```bash
cargo install silc
silc --help
```

Library crates in this workspace (`sil-core`, `sil-lexer`, `sil-parser`,
`sil-router`, `sil-codegen`, `sil-ipc`, `sil-rlm`, `sil-training`) are published
so the CLI can be built from crates.io. Install the compiler with
`cargo install silc`.
