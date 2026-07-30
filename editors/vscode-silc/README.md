# Silc Language (VS Code / Cursor)

Native TextMate grammar plus a Rust language server (`sil-lsp`) for `.silc`
sources. Replaces the temporary Raku (`source.perl.6`) association and adds
semantic hover for Silc-specific primitives, resource methods, contracts,
components, and more.

## Install

```bash
./editors/vscode-silc/install.sh
```

The script:

1. Builds `sil-lsp` in release mode
2. Compiles the TypeScript language client
3. Bundles a host-platform server binary into the VSIX
4. Installs the extension with the `cursor` CLI (falling back to `code`)

Reload the window afterwards via **Developer: Reload Window**. Set
`SILC_EDITOR_CLI` to override CLI detection.

Confirm it took effect by opening any `.silc` file — the language indicator
should read **Silc**, and hovering symbols should show Markdown tooltips.

## Hover coverage

Hover works on:

| Target | Example |
| --- | --- |
| Resource methods | `Articles.list()` |
| Query bindings | `query $.articles = …` |
| Contracts / fields | `Article`, `$article.title` |
| Components / props / state / handlers | `AdminPage`, `$.q`, parameters |
| UI primitives & props | `ui::table`, `:sortable` |
| Executable ops | `llm::complete`, `scrape::page` |
| Keywords, operators, builtin types | `query`, `==>`, `Str` |

## Development

```bash
# Build the language server
cargo build -p sil-lsp --release

# Compile the client
cd editors/vscode-silc && npm install && npm run compile
```

Point the editor at a local binary without reinstalling:

- Setting: `silc.languageServerPath`
- Example: `/absolute/path/to/silc/target/release/sil-lsp`

## What is highlighted

| Silc construct | Example | Scope |
| --- | --- | --- |
| Declarations | `contract`, `component`, `resource`, `app`, `game`, `service`, `processor`, `sink`, `task`, `subset`, `class` | `storage.type.declaration.silc` |
| Members | `has`, `method`, `query`, `mutation`, `seed`, `slot`, `emit`, `state` | `keyword.other.member.silc` |
| Control | `when`, `else`, `for`, `await`, `route` | `keyword.control.silc` |
| Modifiers | `is`, `of`, `where` | `storage.modifier.silc` |
| Builtin types | `Str`, `UUID`, `Bool`, `Int`, `num32`, `num64`, `int32`, `int64`, `Vec` | `support.type.builtin.silc` |
| Namespaced calls | `ui::stack`, `llm::complete`, `scrape::site`, `game::scene` | `support.class.namespace.silc` + `support.function.builtin.silc` |
| Colon pairs | `:label("Save")`, `:sortable` | `entity.other.attribute-name.silc` |
| Attributes | `$.title`, `$record` | `variable.other.member.silc` |
| Feed / arrows | `==>`, `=>`, `->` | `keyword.operator.feed.silc`, `keyword.operator.arrow.silc` |
| Annotations | `@version("0.4.0")` | `entity.name.function.decorator.silc` |
| Unit literals | `250ms`, `512MB`, `90fps`, `8cm`, `14deg` | `constant.numeric.unit.silc` |

## Maintaining

Keywords must stay in sync with the lexer in
`crates/sil-lexer/src/lib.rs` and the builtin type list in
`crates/sil-core/src/program.rs`. Hover docs for keywords/types live in
`crates/sil-ide/src/docs.rs`. Bump `version` in `package.json` and re-run
`install.sh` after editing the grammar or client.
