use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn talk_today_source() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/talk_today.silc"),
    )
    .expect("talk_today fixture")
}

fn talk_today_program() -> sil_core::Program {
    let program = sil_parser::parse(&talk_today_source()).expect("parse");
    program.validate().expect("validate");
    program
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("silc-{label}-{nonce}"))
}

#[test]
fn talk_today_emits_mcp_tools_worker_and_manifest() {
    let program = talk_today_program();
    let decisions = sil_router::route_program(&program);
    let out = temp_dir("mcp-talk");
    let result = sil_codegen::emit(&program, &decisions, Path::new("main.silc"), &out, "test")
        .expect("emit");

    let report = result.mcp_report.expect("mcp report");
    assert!(report.contains("SILC_MCP_TOKEN"), "{report}");
    assert!(report.contains("talks_list"), "{report}");
    assert!(report.contains("talk_today_run"), "{report}");

    let worker = std::fs::read_to_string(out.join("typescript/worker.ts")).unwrap();
    assert!(worker.contains("const MCP_ENABLED = true"), "MCP enabled");
    assert!(worker.contains("\"talks_list\""), "talks_list tool");
    assert!(worker.contains("\"talk_today_run\""), "talk_today_run tool");
    assert!(worker.contains("async function handleMcp"), "handler");
    assert!(
        !worker.contains("__MCP_TOOLS_JSON__")
            && !worker.contains("__MCP_AUTH_ENV__")
            && !worker.contains("__MCP_PATH__"),
        "placeholders must be replaced"
    );

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&result.manifest).unwrap()).unwrap();
    let tools = manifest["mcp"]["tools"].as_array().expect("mcp.tools");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"talks_list"), "{names:?}");
    assert!(names.contains(&"talk_today_run"), "{names:?}");
    assert_eq!(manifest["mcp"]["path"], "/mcp");
    assert_eq!(manifest["mcp"]["auth_env"], "SILC_MCP_TOKEN");

    let plan: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("loop/plan.json")).unwrap())
            .unwrap();
    let plan_names: Vec<&str> = plan["mcp"]["tools"]
        .as_array()
        .expect("plan mcp.tools")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(plan_names.contains(&"talks_list"), "{plan_names:?}");
    assert!(plan_names.contains(&"talk_today_run"), "{plan_names:?}");

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn mcp_tool_list_change_changes_plan_hash() {
    let program = sil_codegen::loop_lower::synthesize_loop_surface(&talk_today_program())
        .expect("synthesize");
    let a = sil_codegen::loop_lower::lower_loop_plan(&program, "test");

    // Drop the create mutation from the author resource so the tool list shrinks.
    let mut altered = program.clone();
    if let Some(resource) = altered.resources.iter_mut().find(|r| r.name == "Talks") {
        resource.methods.retain(|m| m.name != "create");
    }
    let b = sil_codegen::loop_lower::lower_loop_plan(&altered, "test");
    assert_ne!(
        a["hash"], b["hash"],
        "plan hash must cover the MCP tool list"
    );
}

#[test]
fn rfi_chase_also_gets_mcp_surface() {
    let source =
        std::fs::read_to_string(repo_root().join("examples/domains/aec/rfiChaseApp/main.silc"))
            .unwrap();
    let program = sil_parser::parse(&source).unwrap();
    program.validate().unwrap();
    let decisions = sil_router::route_program(&program);
    let out = temp_dir("mcp-rfi");
    let result = sil_codegen::emit(&program, &decisions, Path::new("main.silc"), &out, "test")
        .expect("emit");
    let report = result.mcp_report.expect("mcp report");
    assert!(report.contains("rfis_list"), "{report}");
    assert!(report.contains("rfi_chase_run"), "{report}");
    let _ = std::fs::remove_dir_all(&out);
}
