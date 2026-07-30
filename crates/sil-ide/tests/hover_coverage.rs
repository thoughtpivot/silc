//! Permanent hover coverage matrix across Silc intent catalogs.
//!
//! Asserts that catalogued author-facing surfaces resolve to teaching hover
//! (not `NONE`). Example caret probes cover game, UI, ops, and builtins.

use sil_core::{
    format_game_catalog_line, game_closed_value_doc, game_prop_doc, lookup_game_node,
    EXECUTABLE_OPS, GAME_NODE_CATALOG, KNOWN_NAMESPACES, UI_COMPONENT_CATALOG,
};
use sil_ide::{
    builtin_type_doc, keyword_doc, resolve_hover, Document, BUILTIN_TYPE_NAMES, KEYWORD_NAMES,
};

fn workspace_file(rel: &str) -> String {
    let path = format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn hover_at(src: &str, offset: u32) -> String {
    let doc = Document::open("file://coverage.silc", 1, src);
    resolve_hover(&doc, offset)
        .unwrap_or_else(|| panic!("NONE hover at offset {offset} (parse_error={:?})", doc.parse_error))
        .markdown
}

fn hover_on_member(src: &str, ns_member: &str) -> String {
    // Place caret on the member after `ns::`
    let offset = src
        .find(ns_member)
        .unwrap_or_else(|| panic!("missing {ns_member}")) as u32;
    let after_ns = ns_member.find("::").map(|i| i + 2).unwrap_or(0) as u32;
    hover_at(src, offset + after_ns)
}

#[test]
fn keyword_and_type_catalogs_have_docs() {
    for kw in KEYWORD_NAMES {
        let doc = keyword_doc(kw).unwrap_or_else(|| panic!("missing keyword_doc for {kw}"));
        assert!(doc.len() > 60, "{kw} doc too short");
    }
    for ty in BUILTIN_TYPE_NAMES {
        let doc = builtin_type_doc(ty).unwrap_or_else(|| panic!("missing builtin_type_doc for {ty}"));
        assert!(doc.len() > 40, "{ty} doc too short");
    }
}

#[test]
fn every_ui_component_has_description() {
    for spec in UI_COMPONENT_CATALOG {
        assert!(
            spec.description.len() > 40,
            "ui::{} description too short",
            spec.name
        );
        for prop in spec.props {
            let prose = sil_core::prop_doc(spec.name, prop.name);
            assert!(
                prose.is_some(),
                "ui::{} :{} missing prop_doc",
                spec.name,
                prop.name
            );
        }
        for ev in spec.events {
            assert!(
                sil_core::event_doc(spec.name, ev.name).is_some(),
                "ui::{} event {} missing event_doc",
                spec.name,
                ev.name
            );
        }
    }
}

#[test]
fn every_game_node_and_prop_has_docs() {
    assert_eq!(GAME_NODE_CATALOG.len(), 44);
    for spec in GAME_NODE_CATALOG {
        assert!(spec.description.len() > 40, "game::{} thin", spec.name);
        let _ = format_game_catalog_line(spec);
        for prop in spec.props {
            let prose = game_prop_doc(spec.name, prop.name).expect("game_prop_doc");
            assert!(prose.len() > 20, "game::{} :{} thin", spec.name, prop.name);
            for value in prop.closed_values {
                assert!(
                    game_closed_value_doc(value).is_some(),
                    "missing closed doc for {value}"
                );
            }
        }
    }
}

#[test]
fn every_executable_op_has_specific_hover_prose() {
    for (ns, name) in EXECUTABLE_OPS {
        let snippet = format!(
            r#"@version("0.4.0")
processor P {{
    method run() {{
        {ns}::{name}();
    }}
}}
"#
        );
        // processor may reject some ops — fall back to raw token hover via open
        let doc = Document::open("file://op.silc", 1, &snippet);
        let needle = format!("{ns}::{name}");
        if let Some(pos) = snippet.find(&needle) {
            let offset = (pos + ns.len() + 2) as u32;
            if let Some(hover) = resolve_hover(&doc, offset) {
                assert!(
                    hover.markdown.contains("executable op")
                        || hover.markdown.contains("Runnable"),
                    "{ns}::{name} hover weak:\n{}",
                    hover.markdown
                );
                assert!(
                    !hover.markdown.contains("prefer the executable set over stub-only"),
                    "{ns}::{name} should not use generic fallback prose:\n{}",
                    hover.markdown
                );
            } else {
                // Keyword-ns ops must resolve even when parse fails
                panic!("NONE hover for {ns}::{name} (parse_error={:?})", doc.parse_error);
            }
        }
    }
    let _ = KNOWN_NAMESPACES;
}

#[test]
fn arena_game_node_hovers() {
    let src = workspace_file("examples/arenaGameApp/main.silc");
    for node in [
        "scene",
        "entity",
        "prefab",
        "spawn",
        "data",
        "mode",
        "pawn",
        "controller",
        "camera",
        "ability",
        "mesh",
        "light",
        "collider",
        "movement",
        "attribute",
        "signal",
        "group",
        "particle_emitter",
        "dynamic_light",
        "camera_impulse",
        "post_process",
        "overlay",
    ] {
        let needle = format!("game::{node}");
        let md = hover_on_member(&src, &needle);
        assert!(
            md.contains("game node") && md.contains(&format!("game::{node}")),
            "expected game node hover for {node}:\n{md}"
        );
        assert!(
            lookup_game_node(node).unwrap().description.len() > 40,
            "{node} description"
        );
        assert!(
            md.contains(lookup_game_node(node).unwrap().description)
                || md.len() > 80,
            "thin hover for {node}:\n{md}"
        );
    }
}

#[test]
fn arena_game_prop_and_enum_hovers() {
    let src = workspace_file("examples/arenaGameApp/main.silc");

    let title_off = (src.find(":title(").unwrap() + 1) as u32;
    let md = hover_at(&src, title_off);
    assert!(md.contains("game prop") && md.contains("title"), "{md}");

    let as_pawn_off = (src.find(":as_pawn").unwrap() + 1) as u32;
    let md = hover_at(&src, as_pawn_off);
    assert!(md.contains("game prop") && md.contains("as_pawn"), "{md}");

    let capsule_off = src.find(":shape(capsule)").unwrap() + ":shape(".len();
    let md = hover_at(&src, capsule_off as u32);
    assert!(
        md.contains("game value") || md.contains("capsule") || md.contains("Capsule"),
        "capsule enum hover:\n{md}"
    );

    let webgpu_off = src.find(":renderer(webgpu)").unwrap() + ":renderer(".len();
    let md = hover_at(&src, webgpu_off as u32);
    assert!(md.contains("webgpu") || md.contains("WebGPU"), "{md}");

    let game_ns = src.find("game::scene").unwrap() as u32;
    let md = hover_at(&src, game_ns);
    assert!(
        md.contains("namespace") || md.contains("WebGPU"),
        "game:: qualifier should prefer namespace doc:\n{md}"
    );
}

#[test]
fn service_http_keyword_namespace_hover() {
    let src = r#"@version("0.4.0")
service Api {
    method boot() {
        service::http(:port(8080));
    }
}
"#;
    let doc = Document::open("file://svc.silc", 1, src);
    let offset = (src.find("service::http").unwrap() + "service::".len()) as u32;
    let hover = resolve_hover(&doc, offset).expect("service::http hover");
    assert!(
        hover.markdown.contains("executable op") || hover.markdown.contains("HTTP"),
        "{}",
        hover.markdown
    );
}

#[test]
fn blog_ui_slot_variant_and_event_hovers() {
    let src = workspace_file("examples/blogApp/main.silc");

    if let Some(pos) = src.find(":app_bar(") {
        let md = hover_at(&src, (pos + 1) as u32);
        assert!(
            md.contains("ui slot") || md.contains("app_bar"),
            "app_bar slot:\n{md}"
        );
    }

    if let Some(pos) = src.find(":variant(primary)") {
        let md = hover_at(&src, (pos + ":variant(".len()) as u32);
        assert!(
            md.contains("primary") || md.contains("ui value"),
            "variant primary:\n{md}"
        );
    }

    if let Some(pos) = src.find(":on(click(") {
        let md = hover_at(&src, (pos + ":on(".len()) as u32);
        assert!(
            md.contains("ui event") || md.contains("click") || md.contains("activates"),
            "click event:\n{md}"
        );
    }
}

#[test]
fn doc_extract_and_op_prop_hover() {
    let src = workspace_file("examples/dataExtractorApp/main.silc");
    if let Some(pos) = src.find("doc::extract") {
        let md = hover_at(&src, (pos + "doc::".len()) as u32);
        assert!(
            md.contains("executable op") && md.contains("extract"),
            "{md}"
        );
        assert!(
            md.contains("ADR-011") || md.contains("upload") || md.contains("file_input"),
            "doc::extract should be specific:\n{md}"
        );
    }
    if let Some(pos) = src.find(":into(") {
        let md = hover_at(&src, (pos + 1) as u32);
        assert!(
            md.contains("op prop") || md.contains("contract") || md.contains("into"),
            "into prop:\n{md}"
        );
    }
}

#[test]
fn unit_literal_and_vec_hover() {
    let src = r#"@version("0.4.0")
contract C {
    has Vec[num32; 384] $.embedding;
}
component X {
    method render() {
        ui::text(:text("hi"));
    }
    method tick() {
        $.wait = 250ms;
        $.rate = 90fps;
        $.span = 8cm;
    }
}
"#;
    let doc = Document::open("file://units.silc", 1, src);
    let vec_off = src.find("Vec[").unwrap() as u32;
    let hover = resolve_hover(&doc, vec_off).expect("Vec hover");
    assert!(hover.markdown.contains("Vec") || hover.markdown.contains("vector"), "{}", hover.markdown);

    for needle in ["250ms", "90fps", "8cm"] {
        let off = src.find(needle).unwrap() as u32;
        let hover = resolve_hover(&doc, off).expect(needle);
        assert!(
            hover.markdown.contains("unit literal") || hover.markdown.contains(needle),
            "{needle}: {}",
            hover.markdown
        );
    }
}

#[test]
fn navigate_submit_new_builtin_hovers() {
    let src = r#"@version("0.4.0")
contract Item { has Str $.id; has Str $.title; }
component Page {
    method go() {
        navigate("/home");
        submit();
        Item.new(:id("1"), :title("t"));
    }
    method render() { ui::text(:text("x")); }
}
"#;
    let doc = Document::open("file://builtins.silc", 1, src);
    for (needle, expect) in [
        ("navigate(", "navigate"),
        ("submit()", "submit"),
        (".new(", "new"),
    ] {
        let base = src.find(needle).unwrap();
        let off = if needle.starts_with('.') {
            (base + 1) as u32
        } else {
            base as u32
        };
        let hover = resolve_hover(&doc, off).unwrap_or_else(|| panic!("NONE for {needle}"));
        assert!(
            hover.markdown.contains(expect),
            "{needle} -> {}",
            hover.markdown
        );
    }
}
