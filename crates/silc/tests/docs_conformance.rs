//! Documentation conformance: catalog lines, executable ops, and AGENTS sync.
//!
//! Sources of truth remain `UI_COMPONENT_CATALOG` and `EXECUTABLE_OPS` in sil-core.
//! The AGENTS template and root README must list every entry; tracked example
//! AGENTS.md files must embed the template common block byte-for-byte.

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
fn ui_catalog_lines_present_in_template_and_readme() {
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
            readme.contains(&line),
            "README missing catalog line for ui::{}:\n{line}",
            spec.name
        );
        assert!(
            line.contains("surfaces: web+terminal"),
            "catalog line must declare dual-surface: {line}"
        );
    }
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

    for doc in [&template, &readme] {
        assert!(
            doc.contains("primary")
                && doc.contains("secondary")
                && doc.contains("destructive")
                && doc.contains("ghost"),
            "docs must list closed :variant values"
        );
        assert!(
            doc.contains("default")
                && doc.contains("muted")
                && doc.contains("info")
                && doc.contains("success")
                && doc.contains("warning")
                && doc.contains("danger"),
            "docs must list closed :tone values"
        );
        assert!(
            doc.contains("`sm`") && doc.contains("`md`") && doc.contains("`lg`"),
            "docs must list closed :size values"
        );
        assert!(
            doc.contains("ui::web") && doc.contains("ui::terminal"),
            "docs must mention both surfaces"
        );
        assert!(
            doc.contains("must") && doc.contains("both"),
            "docs must state dual-surface requirement"
        );
    }
}

#[test]
fn tracked_example_agents_embed_template_common_block() {
    let template = read_workspace("crates/silc/templates/AGENTS.md");
    let expected = template_common_block(&template);

    for app in ["chatApp", "inventoryApp"] {
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
