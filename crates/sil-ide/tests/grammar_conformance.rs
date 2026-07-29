//! Keep the VS Code TextMate grammar aligned with the lexer and hover catalogs.
//!
//! When you add a keyword or unit suffix to `sil-lexer`, update
//! `editors/vscode-silc/syntaxes/silc.tmLanguage.json` (and usually
//! `KEYWORD_NAMES` / unit docs) so this test stays green.

use std::fs;
use std::path::PathBuf;

use sil_ide::{BUILTIN_TYPE_NAMES, KEYWORD_NAMES};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn grammar_text() -> String {
    let path = workspace_root().join("editors/vscode-silc/syntaxes/silc.tmLanguage.json");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Unit suffixes from `sil-lexer` `UnitLiteral` regex.
const UNIT_SUFFIXES: &[&str] = &[
    "ms", "s", "MB", "GB", "rps", "ops", "cm", "m", "deg", "fps", "px",
];

#[test]
fn textmate_grammar_covers_lexer_keywords() {
    let grammar = grammar_text();
    for kw in KEYWORD_NAMES {
        assert!(
            grammar.contains(kw),
            "TextMate grammar missing keyword `{kw}` — update silc.tmLanguage.json"
        );
    }
}

#[test]
fn textmate_grammar_covers_builtin_types() {
    let grammar = grammar_text();
    for ty in BUILTIN_TYPE_NAMES {
        assert!(
            grammar.contains(ty),
            "TextMate grammar missing builtin type `{ty}`"
        );
    }
}

#[test]
fn textmate_grammar_covers_unit_suffixes() {
    let grammar = grammar_text();
    let unit_rule = grammar
        .lines()
        .find(|l| l.contains("ms|s|MB") || l.contains("UnitLiteral") || l.contains("[0-9]+(?:"))
        .or_else(|| {
            // Fall back: any line with the unit-literal match pattern.
            grammar.lines().find(|l| l.contains("rps|ops"))
        })
        .expect("unit-literal match rule missing from TextMate grammar");

    for suffix in UNIT_SUFFIXES {
        assert!(
            unit_rule.contains(suffix),
            "TextMate unit-literal rule missing suffix `{suffix}` in:\n{unit_rule}"
        );
    }
}

#[test]
fn textmate_grammar_lists_game_as_declaration() {
    let grammar = grammar_text();
    assert!(
        grammar.contains("app|game|service") || grammar.contains("|game|"),
        "TextMate declaration keyword list must include `game`"
    );
}
