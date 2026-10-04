//! Documentation conformance: catalog lines, executable operations, vocabulary,
//! and AGENTS sync.
//!
//! Sources of truth remain the catalogs and `EXECUTABLE_OPS` in sil-core. The
//! AGENTS template must list every catalog entry, every executable operation,
//! and the generated game closed-enum paragraph. The root README is a
//! high-level white paper: it must list executable operations, state
//! dual-surface synthesis, and point agents at AGENTS.md for the full catalog.
//! Every example `AGENTS.md` must embed the template common block
//! byte-for-byte. Author-facing prose must use the full-word vocabulary from
//! `docs/GLOSSARY.md` and must not hard-code catalog sizes.

use std::fs;
use std::path::PathBuf;

use sil_core::{format_component_catalog_line, EXECUTABLE_OPS, UI_COMPONENT_CATALOG};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn read_workspace(rel: &str) -> String {
    fs::read_to_string(workspace_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Every tracked example directory (one `main.silc` + `AGENTS.md` each).
fn example_dirs() -> Vec<String> {
    let mut dirs: Vec<String> = fs::read_dir(workspace_root().join("examples"))
        .expect("read examples/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("main.silc").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    dirs.sort();
    assert!(
        dirs.len() >= 10,
        "expected the tracked example set, found {dirs:?}"
    );
    dirs
}

/// Markdown prose with fenced blocks and inline code removed, so vocabulary
/// and count scans only see author-facing sentences.
fn prose_only(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut in_fence = false;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push('\n');
            continue;
        }
        if in_fence {
            out.push('\n');
            continue;
        }
        let mut in_code = false;
        for ch in line.chars() {
            if ch == '`' {
                in_code = !in_code;
                out.push(' ');
            } else if in_code {
                out.push(' ');
            } else {
                out.push(ch);
            }
        }
        out.push('\n');
    }
    out
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Words in `text` with their byte offsets (letters, digits, underscore).
fn words(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, ch) in text.char_indices() {
        match (is_word_char(ch), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, &text[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, &text[s..]));
    }
    out
}

fn template_common_block(template: &str) -> &str {
    let begin = "<!-- BEGIN SILC_AGENTS_TEMPLATE -->";
    let end = "<!-- END SILC_AGENTS_TEMPLATE -->";
    let start = template
        .find(begin)
        .unwrap_or_else(|| panic!("missing {begin} in AGENTS template"));
    let finish = template
        .find(end)
        .unwrap_or_else(|| panic!("missing {end} in AGENTS template"))
        + end.len();
    &template[start..finish]
}

#[test]
fn game_catalog_lines_present_in_agents_template() {
    let template = read_workspace("crates/silc/templates/AGENTS.md");
    assert_eq!(
        sil_core::GAME_NODE_CATALOG.len(),
        58,
        "game catalog size changed; update docs and this assertion"
    );
    for spec in sil_core::GAME_NODE_CATALOG {
        let line = sil_core::format_game_catalog_line(spec);
        assert!(
            template.contains(&line),
            "AGENTS template missing catalog line for game::{}:\n{line}",
            spec.name
        );
    }
    assert!(
        template.contains("### Complete game::* catalog (ADR-012)"),
        "AGENTS must include the game catalog section"
    );
    let enums = sil_core::format_game_closed_enums_line();
    assert!(
        template.contains(&enums),
        "AGENTS template must carry the generated game closed-enum paragraph verbatim:\n{enums}"
    );
}

#[test]
fn loop_catalog_lines_present_in_agents_template() {
    let template = read_workspace("crates/silc/templates/AGENTS.md");
    for spec in sil_core::LOOP_NODE_CATALOG {
        let line = sil_core::format_loop_catalog_line(spec);
        assert!(
            template.contains(&line),
            "AGENTS template missing catalog line for loop::{}:\n{line}",
            spec.name
        );
    }
    assert!(
        template.contains("### Complete loop::* catalog (ADR-014)"),
        "AGENTS must include the loop catalog section"
    );
}

#[test]
fn ui_catalog_lines_present_in_agents_template() {
    let template = read_workspace("crates/silc/templates/AGENTS.md");
    let readme = read_workspace("README.md");

    assert_eq!(
        UI_COMPONENT_CATALOG.len(),
        39,
        "catalog size changed; update docs and this assertion"
    );

    for spec in UI_COMPONENT_CATALOG {
        let line = format_component_catalog_line(spec);
        assert!(
            template.contains(&line),
            "AGENTS template missing catalog line for ui::{}:\n{line}",
            spec.name
        );
        assert!(
            line.contains("surfaces: web+terminal"),
            "catalog line must declare dual-surface: {line}"
        );
    }

    // White-paper README points agents at the full catalog rather than
    // mirroring every option/event line.
    assert!(
        readme.contains("crates/silc/templates/AGENTS.md"),
        "README must link the AGENTS template for the full UI catalog"
    );
    assert!(
        readme.contains("UI catalog") || readme.contains("UI primitive"),
        "README must mention the UI catalog / primitives"
    );
}

#[test]
fn executable_ops_listed_in_template_and_readme() {
    let template = read_workspace("crates/silc/templates/AGENTS.md");
    let readme = read_workspace("README.md");

    for (ns, name) in EXECUTABLE_OPS {
        let op = format!("{ns}::{name}");
        assert!(
            template.contains(&op),
            "AGENTS template missing executable op {op}"
        );
        assert!(readme.contains(&op), "README missing executable op {op}");
    }
}

#[test]
fn closed_enums_and_dual_surface_invariant_documented() {
    let template = read_workspace("crates/silc/templates/AGENTS.md");
    let readme = read_workspace("README.md");

    // Full closed-enum tables live in AGENTS.md (canonical agent contract).
    assert!(
        template.contains("primary")
            && template.contains("secondary")
            && template.contains("destructive")
            && template.contains("ghost"),
        "AGENTS must list closed :variant values"
    );
    assert!(
        template.contains("default")
            && template.contains("muted")
            && template.contains("info")
            && template.contains("success")
            && template.contains("warning")
            && template.contains("danger"),
        "AGENTS must list closed :tone values"
    );
    assert!(
        template.contains("`sm`") && template.contains("`md`") && template.contains("`lg`"),
        "AGENTS must list closed :size values"
    );

    for (label, doc) in [("AGENTS", &template), ("README", &readme)] {
        assert!(
            doc.contains("ui::web") && doc.contains("ui::terminal"),
            "{label} must mention both surfaces"
        );
        assert!(
            doc.contains("synthesiz") && doc.contains("both"),
            "{label} must state dual-surface serving is synthesized for both surfaces"
        );
        assert!(
            !doc.contains("declare both surfaces in `serve()`")
                && !doc.contains("must declare both `ui::web`"),
            "{label} must not require author-declared serve()/ui surface ops"
        );
    }
}

#[test]
fn removed_author_ops_not_listed_as_runnable() {
    let template = read_workspace("crates/silc/templates/AGENTS.md");
    let readme = read_workspace("README.md");

    for (label, doc) in [("AGENTS", &template), ("README", &readme)] {
        let start = doc
            .find("Runnable operations (0.5.0)")
            .or_else(|| doc.find("### Executable operations"))
            .unwrap_or_else(|| panic!("{label}: missing runnable operations section"));
        let section = &doc[start..];
        // Author-facing list only — stop before the "Compiler-synthesized" note
        // (AGENTS) or stub-only / generated sections (README).
        let end = section
            .find("Compiler-synthesized")
            .or_else(|| section.find("Stub-only"))
            .or_else(|| section.find("### Generated"))
            .or_else(|| section.find("**Boundaries**"))
            .unwrap_or(section.len().min(1200));
        let author_ops = &section[..end];

        for forbidden in [
            "`ui::web`",
            "`ui::terminal`",
            "`ipc::publish`",
            "`store::sqlite`",
            "`store::commit`",
            "`resource::list`",
            "`resource::get`",
            "`resource::create`",
            "`resource::update`",
            "`resource::delete`",
        ] {
            assert!(
                !author_ops.contains(forbidden),
                "{label} author-facing runnable list must not include synthesized op {forbidden}"
            );
        }

        assert!(
            author_ops.contains("tensor::tokenize") && author_ops.contains("tensor::infer"),
            "{label} author-facing runnable list must include tensor ops"
        );
    }
}

#[test]
fn tracked_example_agents_embed_template_common_block() {
    let template = read_workspace("crates/silc/templates/AGENTS.md");
    let expected = template_common_block(&template);

    for app in example_dirs() {
        let agents = read_workspace(&format!("examples/{app}/AGENTS.md"));
        let actual = template_common_block(&agents);
        assert_eq!(
            actual, expected,
            "{app}/AGENTS.md common block must match crates/silc/templates/AGENTS.md byte-for-byte"
        );
        assert!(
            agents.len() > expected.len(),
            "{app}/AGENTS.md must append app-specific guidance after the template block"
        );
    }
}

#[test]
fn canonical_silc_sources_omit_runtime_plumbing() {
    let mut roots: Vec<String> = example_dirs()
        .into_iter()
        .map(|app| format!("examples/{app}/main.silc"))
        .collect();
    roots.extend(
        [
            "crates/silc/templates/main.silc",
            "crates/silc/tests/fixtures/scored_form.silc",
            "crates/silc/tests/fixtures/shopping_app.silc",
            "crates/silc/tests/fixtures/blog_app.silc",
            "crates/silc/tests/fixtures/data_pipeline.silc",
            "crates/silc/tests/fixtures/data_pipeline_runnable.silc",
            "crates/sil-router/tests/fixtures/data_pipeline.silc",
            "crates/sil-router/tests/fixtures/data_pipeline_runnable.silc",
        ]
        .map(String::from),
    );

    let forbidden = [
        "sink ",
        "method serve(",
        "ui::web(",
        "ui::terminal(",
        "ipc::publish",
        "store::sqlite",
        "store::commit",
        "resource::list",
        "resource::get",
        "resource::create",
        "resource::update",
        "resource::delete",
        "is storage",
        "has $.table",
        "@version(\"0.4.0\")",
        "@version(\"0.3.0\")",
        "@version(\"0.2.0\")",
    ];

    for rel in &roots {
        let src = read_workspace(rel);
        assert!(
            src.contains("@version(\"0.5.0\")"),
            "{rel} must declare @version(\"0.5.0\")"
        );
        for needle in forbidden {
            assert!(
                !src.contains(needle),
                "{rel} must not contain author runtime plumbing `{needle}`"
            );
        }
    }
}

/// Author-facing documents whose prose must use the full-word vocabulary from
/// `docs/GLOSSARY.md`: "operation" (not op) and "option" (not prop).
const VOCABULARY_DOCS: &[&str] = &[
    "README.md",
    "crates/silc/templates/AGENTS.md",
    "docs/SILC-LANGUAGE.md",
    "docs/GLOSSARY.md",
    "examples/README.md",
    "editors/vscode-silc/README.md",
];

#[test]
fn author_facing_prose_uses_full_word_vocabulary() {
    let banned = ["op", "ops", "prop", "props"];
    let mut offenders = Vec::new();
    for rel in VOCABULARY_DOCS {
        let doc = read_workspace(rel);
        let prose = prose_only(&doc);
        for (offset, word) in words(&prose) {
            if !banned.contains(&word) {
                continue;
            }
            // The glossary names the retired words in straight quotes when it
            // forbids them ("op"); that is the one place the abbreviation may appear.
            let quoted =
                prose[..offset].ends_with('"') && prose[offset + word.len()..].starts_with('"');
            if quoted {
                continue;
            }
            let line = prose[..offset].matches('\n').count() + 1;
            offenders.push(format!("{rel}:{line}: `{word}`"));
        }
    }
    assert!(
        offenders.is_empty(),
        "author-facing prose must say operation/option, not op/prop (see docs/GLOSSARY.md):\n{}",
        offenders.join("\n")
    );
}

/// Catalog sizes are pinned by tests, not prose. A sentence such as "39
/// primitives" or "58-node catalog" goes stale the moment a node is added.
#[test]
fn prose_does_not_hard_code_catalog_counts() {
    let docs = [
        "README.md",
        "crates/silc/templates/AGENTS.md",
        "docs/SILC-LANGUAGE.md",
        "docs/ARCHITECTURE.md",
        "docs/ADR-003-declarative-ui.md",
        "docs/ADR-012-webgpu-game-subject.md",
        "docs/ADR-014-loop-subject.md",
        "examples/README.md",
    ];
    let counted_nouns = [
        "primitive",
        "primitives",
        "node",
        "nodes",
        "builtin",
        "builtins",
        "component",
        "components",
    ];
    let mut offenders = Vec::new();
    for rel in docs {
        let doc = read_workspace(rel);
        let prose = prose_only(&doc);
        let toks = words(&prose);
        for (i, (offset, word)) in toks.iter().enumerate() {
            if word.is_empty() || !word.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            // Skip version components ("0.5.0") and ADR numbers ("ADR-012").
            let before = prose[..*offset].chars().last();
            if matches!(before, Some('.') | Some('-')) {
                continue;
            }
            let Some((next_offset, next)) = toks.get(i + 1) else {
                continue;
            };
            let gap = &prose[offset + word.len()..*next_offset];
            let adjacent = gap == " " || gap == "-";
            if adjacent && counted_nouns.contains(next) {
                let line = prose[..*offset].matches('\n').count() + 1;
                offenders.push(format!("{rel}:{line}: `{word}{gap}{next}`"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "catalog counts belong in tests, not prose:\n{}",
        offenders.join("\n")
    );
}
