use std::path::{Path, PathBuf};

use sil_codegen::loop_lower::{
    lower_loop_plan, synthesize_loop_surface, LOOP_APPROVALS_TABLE, LOOP_INBOX_ROUTE,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example_source() -> String {
    std::fs::read_to_string(repo_root().join("examples/domains/aec/rfiChaseApp/main.silc"))
        .expect("read examples/domains/aec/rfiChaseApp/main.silc")
}

fn example_program() -> sil_core::Program {
    let program = sil_parser::parse(&example_source()).expect("parse example");
    program.validate().expect("validate example");
    program
}

const DIGEST_SCHEDULE: &str =
    r#"loop::schedule(:cron("0 5 * * *"), :tz("UTC"), :catch_up("12h")),"#;

fn digest_source() -> &'static str {
    include_str!("fixtures/loop_digest.silc")
}

fn fallback_program() -> sil_core::Program {
    parse_valid(include_str!("fixtures/loop_fallback.silc"))
}

fn parse_valid(source: &str) -> sil_core::Program {
    let program = sil_parser::parse(source).expect("parse fixture");
    program.validate().expect("validate fixture");
    program
}

fn digest_program() -> sil_core::Program {
    parse_valid(digest_source())
}

/// The digest fixture with a manual trigger: every loop manual, no `app` — a command.
fn command_program() -> sil_core::Program {
    let source = digest_source();
    assert!(source.contains(DIGEST_SCHEDULE));
    parse_valid(&source.replace(DIGEST_SCHEDULE, "loop::manual(),"))
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("silc-{label}-{nonce}"))
}

#[test]
fn rfi_chase_plan_matches_golden() {
    let program = synthesize_loop_surface(&example_program()).expect("synthesize");
    let plan = lower_loop_plan(&program, "test");
    let rendered = serde_json::to_string_pretty(&plan).unwrap() + "\n";
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/rfi_chase_plan.json");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, &rendered).unwrap();
    }
    let expected = std::fs::read_to_string(&golden)
        .expect("golden missing; run with UPDATE_GOLDEN=1 to create it");
    assert_eq!(
        rendered, expected,
        "lowered plan drifted from the golden; rerun with UPDATE_GOLDEN=1 if intended"
    );
}

#[test]
fn plan_hash_is_stable_and_content_addressed() {
    let program = synthesize_loop_surface(&example_program()).expect("synthesize");
    let a = lower_loop_plan(&program, "test");
    let b = lower_loop_plan(&program, "test");
    assert_eq!(a["hash"], b["hash"]);
    let c = lower_loop_plan(&program, "other");
    assert_ne!(
        a["hash"], c["hash"],
        "compiler version is part of the plan identity"
    );
}

#[test]
fn surface_adds_inbox_route_and_tables() {
    let program = synthesize_loop_surface(&example_program()).expect("synthesize");
    let app = program.apps.first().expect("app");
    assert!(
        app.routes.iter().any(|r| r.path == LOOP_INBOX_ROUTE),
        "the author's app gains /loops"
    );
    assert!(program.resources.iter().any(|r| r.name == "LoopApprovals"));
    assert!(program.components.iter().any(|c| c.name == "LoopInbox"));
    let plan = lower_loop_plan(&program, "test");
    assert_eq!(plan["tables"]["approvals"], LOOP_APPROVALS_TABLE);
}

#[test]
fn surface_rejects_author_owned_inbox_route() {
    let source = example_source().replace(
        "route \"/\" => RfiBoard;",
        "route \"/\" => RfiBoard;\n    route \"/loops\" => RfiBoard;",
    );
    let program = sil_parser::parse(&source).expect("parse");
    program.validate().expect("validate");
    let err = synthesize_loop_surface(&program).expect_err("reserved route");
    assert!(err.contains("/loops"), "{err}");
}

#[test]
fn emit_writes_pinned_plan_kernel_and_manifest_section() {
    let program = example_program();
    let decisions = sil_router::route_program(&program);
    let out = temp_dir("loop-emit");
    let result = sil_codegen::emit(&program, &decisions, Path::new("main.silc"), &out, "test")
        .expect("emit");
    let report = result.loop_report.expect("loop report");
    assert!(report.contains("RfiChase"), "{report}");

    let plan: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("loop/plan.json")).unwrap())
            .unwrap();
    let hash = plan["hash"].as_str().expect("hash").to_string();
    assert!(out.join(format!("loop/plans/{hash}.json")).is_file());
    assert!(out.join("go/loop/kernel.go").is_file());
    assert!(out.join("go/loop/go.mod").is_file());

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&result.manifest).unwrap()).unwrap();
    assert_eq!(manifest["loop"]["plan_hash"], hash);
    assert_eq!(manifest["loop"]["inbox_route"], LOOP_INBOX_ROUTE);
    assert_eq!(manifest["loop"]["ask"], true);

    let graph = result.graph.expect("graph");
    assert!(graph.has_loops());
    assert!(graph.needs_llm(), "loop::ask provisions silclm");

    // Runs, notices, and the author's board must refresh on their own: the
    // inbox and every resource query poll while visible and refetch on focus.
    let app_tsx = std::fs::read_to_string(out.join("typescript/src/App.tsx")).unwrap();
    assert!(
        app_tsx.contains("__useLiveQuery(\"/api/loop_runs\", setRuns)")
            && app_tsx.contains("visibilitychange")
            && app_tsx.contains("addEventListener(\"focus\""),
        "loop surface queries must stay live"
    );
    let _ = std::fs::remove_dir_all(&out);
}

/// The Silc-provisioned Go toolchain, if this machine has one.
fn silc_go() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SILC_TEST_GO") {
        return Some(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")?;
    let runtimes = PathBuf::from(home).join(".silc/runtimes");
    for platform in std::fs::read_dir(runtimes).ok()?.flatten() {
        let go_root = platform.path().join("go");
        for version in std::fs::read_dir(&go_root).into_iter().flatten().flatten() {
            let bin = version.path().join("go/bin/go");
            if bin.is_file() {
                return Some(bin);
            }
        }
    }
    None
}

#[test]
fn loop_kernel_replay_traces() {
    let Some(go) = silc_go() else {
        eprintln!("skipping loop kernel replay traces: no Silc Go toolchain under ~/.silc");
        return;
    };
    let dir = temp_dir("loop-kernel");
    std::fs::create_dir_all(&dir).unwrap();
    let (kernel, go_mod) = sil_codegen::loop_lower::loop_kernel_sources();
    std::fs::write(dir.join("kernel.go"), kernel).unwrap();
    std::fs::write(dir.join("go.mod"), go_mod).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/loop_kernel/kernel_test.go"),
        dir.join("kernel_test.go"),
    )
    .unwrap();
    let program = synthesize_loop_surface(&example_program()).expect("synthesize");
    let plan = lower_loop_plan(&program, "test");
    std::fs::write(
        dir.join("plan.json"),
        serde_json::to_string_pretty(&plan).unwrap(),
    )
    .unwrap();
    let digest = synthesize_loop_surface(&digest_program()).expect("synthesize digest");
    std::fs::write(
        dir.join("digest_plan.json"),
        serde_json::to_string_pretty(&lower_loop_plan(&digest, "test")).unwrap(),
    )
    .unwrap();
    let command = command_program();
    assert_eq!(
        synthesize_loop_surface(&command).expect("synthesize command"),
        command,
        "a command program gets no inbox"
    );
    std::fs::write(
        dir.join("command_plan.json"),
        serde_json::to_string_pretty(&lower_loop_plan(&command, "test")).unwrap(),
    )
    .unwrap();

    std::fs::write(
        dir.join("fallback_plan.json"),
        serde_json::to_string_pretty(&lower_loop_plan(&fallback_program(), "test")).unwrap(),
    )
    .unwrap();

    let embed = synthesize_loop_surface(&parse_valid(include_str!(
        "../../silc/tests/fixtures/embedding_app.silc"
    )))
    .expect("synthesize embedding fixture");
    std::fs::write(
        dir.join("embed_plan.json"),
        serde_json::to_string_pretty(&lower_loop_plan(&embed, "test")).unwrap(),
    )
    .unwrap();

    let go_cmd = |args: &[&str]| {
        std::process::Command::new(&go)
            .current_dir(&dir)
            .args(args)
            .env("GOTOOLCHAIN", "local")
            .output()
            .expect("run Silc Go")
    };
    let tidy = go_cmd(&["mod", "tidy"]);
    assert!(
        tidy.status.success(),
        "go mod tidy failed:\n{}",
        String::from_utf8_lossy(&tidy.stderr)
    );
    let vet = go_cmd(&["vet", "."]);
    assert!(
        vet.status.success(),
        "go vet failed:\n{}",
        String::from_utf8_lossy(&vet.stderr)
    );
    let test = go_cmd(&["test", "-count=1", "."]);
    assert!(
        test.status.success(),
        "loop kernel replay traces failed:\n{}\n{}",
        String::from_utf8_lossy(&test.stdout),
        String::from_utf8_lossy(&test.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fallback_lowers_attempts_accept_and_otherwise() {
    let plan = lower_loop_plan(&fallback_program(), "test");
    let each = &plan["loops"][0]["steps"][1];
    assert_eq!(each["op"], "each");
    let fallback = &each["steps"][0];
    assert_eq!(fallback["op"], "fallback");
    assert_eq!(fallback["path"], "1.each.0");
    assert_eq!(fallback["as"], "page");
    assert_eq!(fallback["accept"]["k"], "bin");
    let attempts = fallback["attempts"].as_array().expect("attempts");
    assert_eq!(attempts.len(), 2);
    for (i, attempt) in attempts.iter().enumerate() {
        assert_eq!(attempt["op"], "read");
        assert_eq!(attempt["read_op"], "scrape::page");
        assert_eq!(attempt["retry"], 0);
        assert_eq!(attempt["path"], format!("1.each.0.try{i}"));
        assert!(
            attempt.get("as").is_none(),
            "attempts bind through the fallback"
        );
    }
    assert_eq!(fallback["otherwise"][0]["op"], "skip");
    assert_eq!(plan["loops"][0]["bounds"]["reads"], 10 * 2);
}

#[test]
fn embedding_plan_names_feed_and_nearest_tables() {
    let program = synthesize_loop_surface(&parse_valid(include_str!(
        "../../silc/tests/fixtures/embedding_app.silc"
    )))
    .expect("synthesize");
    let plan = lower_loop_plan(&program, "test");
    let embedding = &plan["embedding"];
    assert_eq!(embedding["model"], "minilm-l6-v2");
    assert_eq!(embedding["dimension"], 384);
    assert_eq!(embedding["feeds"]["files"]["input"], "text_excerpt");
    assert_eq!(embedding["feeds"]["files"]["output"], "vector_embedding");
    assert_eq!(embedding["nearest"]["files"], "vector_embedding");
    for lp in plan["loops"].as_array().unwrap() {
        assert_eq!(lp["bounds"]["model_calls"], 0, "embeds are not model calls");
    }
    let digest = synthesize_loop_surface(&digest_program()).expect("synthesize digest");
    assert!(lower_loop_plan(&digest, "test").get("embedding").is_none());
}

#[test]
fn reemit_keeps_previous_pinned_plans() {
    let out = temp_dir("loop-pin");
    let program = example_program();
    let decisions = sil_router::route_program(&program);
    sil_codegen::emit(&program, &decisions, Path::new("main.silc"), &out, "test").expect("emit");
    let first: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("loop/plan.json")).unwrap())
            .unwrap();

    let changed_source = example_source().replace(":max(200)", ":max(150)");
    let changed = sil_parser::parse(&changed_source).expect("parse");
    changed.validate().expect("validate");
    let decisions = sil_router::route_program(&changed);
    sil_codegen::emit(&changed, &decisions, Path::new("main.silc"), &out, "test").expect("emit");
    let second: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("loop/plan.json")).unwrap())
            .unwrap();

    assert_ne!(first["hash"], second["hash"]);
    for plan in [&first, &second] {
        let hash = plan["hash"].as_str().unwrap();
        assert!(
            out.join(format!("loop/plans/{hash}.json")).is_file(),
            "pinned plan {hash} survives a rebuild"
        );
    }
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn ask_from_comma_list_and_bracket_list_lower_to_the_same_plan() {
    let source = |from: &str| {
        format!(
            r#"@version("0.7.1")
contract Release {{
    has Str $.id;
}}
contract Seen {{
    has Str $.id;
}}
contract Impact {{
    has Str $.text;
}}
resource Releases for Release {{
    query list;
}}
resource SeenItems for Seen {{
    query list;
}}
loop Watch {{
    loop::flow(
        loop::manual(),
        loop::find(:as(release), :from(Releases.list), :max(5)),
        loop::find(:as(seen), :from(SeenItems.list), :max(5)),
        loop::ask(:as(impact), :into(Impact), :retry(2), :from({from}), :prompt("What changed?")),
        loop::gate(:that($impact.text != ""), :reason("never notify on an empty answer")),
        loop::notify(:to("owner"), :key("watch:{{$today}}"), :text("{{$impact.text}}"))
    )
}}
"#
        )
    };
    let plan = |from: &str| lower_loop_plan(&parse_valid(&source(from)), "test");
    let comma = plan("$release, $seen");
    let bracket = plan("[$release, $seen]");
    assert_eq!(comma["hash"], bracket["hash"]);
    assert_eq!(comma, bracket);
    let single = plan("$release");
    assert_ne!(comma["hash"], single["hash"]);
    assert!(
        serde_json::to_string(&comma)
            .unwrap()
            .contains(r#""k":"list""#),
        "the ask context lowers to a list expression"
    );
}
