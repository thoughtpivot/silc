# Silc Language (VS Code / Cursor)

Native TextMate grammar for `.silc` sources. Replaces the temporary Raku
(`source.perl.6`) association, which left Silc's declaration keywords —
`contract`, `component`, `resource`, `app`, `processor`, `emit`, `seed`,
`route` — and `ui::` calls and `:colon(pairs)` completely uncolored.

## Install

```bash
./editors/vscode-silc/install.sh
```

The script packages the folder as a VSIX and installs it with the `cursor`
CLI (falling back to `code`). Reload the window afterwards via
`Developer: Reload Window`. Set `SILC_EDITOR_CLI` to override CLI detection.

Confirm it took effect by opening any `.silc` file and checking that the
language indicator in the status bar reads **Silc**.

## What is highlighted

| Silc construct | Example | Scope |
| --- | --- | --- |
| Declarations | `contract`, `component`, `resource`, `app`, `service`, `processor`, `sink`, `task`, `subset`, `class` | `storage.type.declaration.silc` |
| Members | `has`, `method`, `query`, `mutation`, `seed`, `slot`, `emit`, `state` | `keyword.other.member.silc` |
| Control | `when`, `else`, `for`, `await`, `route` | `keyword.control.silc` |
| Modifiers | `is`, `of`, `where` | `storage.modifier.silc` |
| Builtin types | `Str`, `UUID`, `Bool`, `Int`, `num32`, `num64`, `int32`, `int64` | `support.type.builtin.silc` |
| Namespaced calls | `ui::stack`, `llm::complete`, `scrape::site` | `support.class.namespace.silc` + `support.function.builtin.silc` |
| Colon pairs | `:label("Save")`, `:sortable` | `entity.other.attribute-name.silc` |
| Attributes | `$.title`, `$record` | `variable.other.member.silc` |
| Feed / arrows | `==>`, `=>`, `->` | `keyword.operator.feed.silc`, `keyword.operator.arrow.silc` |
| Annotations | `@version("0.4.0")` | `entity.name.function.decorator.silc` |
| Unit literals | `250ms`, `512MB`, `100rps` | `constant.numeric.unit.silc` |

## Maintaining

Keywords must stay in sync with the lexer in
`crates/sil-lexer/src/lib.rs` and the builtin type list in
`crates/sil-core/src/program.rs`. Bump `version` in `package.json` and
re-run `install.sh` after editing the grammar.
