//! Documentation conformance: catalog lines, executable ops, and AGENTS sync.
//!
//! Sources of truth remain `UI_COMPONENT_CATALOG` and `EXECUTABLE_OPS` in sil-core.
//! The AGENTS template must list every catalog entry and executable op. The root
//! README is a high-level white paper: it must list executable ops, state
//! dual-surface synthesis, and point agents at AGENTS.md for the full catalog.
//! Tracked example AGENTS.md files must embed the template common block
//! byte-for-byte.

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
fn ui_catalog_lines_present_in_agents_template() {
    let template = read_workspace("crates/silc/templates/AGENTS.md");
    let readme = read_workspace("README.md");

    assert_eq!(
        UI_COMPONENT_CATALOG.len(),
        38,
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
    // mirroring all 38 prop/event lines.
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
            .find("Runnable operations (0.4.0)")
            .or_else(|| doc.find("### Executable operations"))
            .unwrap_or_else(|| panic!("{label}: missing runnable ops section"));
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

    for app in ["chatApp", "inventoryApp", "scraperApp", "pipelineApp"] {
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
