//! Silc code generation for inspectable stubs and runnable v1 programs.

use sil_core::{infer_graph, ExecutableGraph, ExecutionMode, Module, Program, Target};
use sil_ipc::{ABI_VERSION, PROTOCOL_VERSION};
use sil_router::RouteDecision;
use std::fs;
use std::path::{Path, PathBuf};

const IPC_DIR: &str = "ipc";
const SUPERVISOR_SOCKET: &str = "ipc/supervisor.sock";

const FEEDBACK_TS: &str = include_str!("../templates/feedback_worker.ts");
const FEEDBACK_PY: &str = include_str!("../templates/feedback_worker.py");
const FEEDBACK_GO: &str = include_str!("../templates/feedback_worker.go");
const FEEDBACK_GOMOD: &str = include_str!("../templates/go.mod");

#[derive(Debug, Clone)]
pub struct EmitResult {
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub generated: Vec<PathBuf>,
    pub execution_mode: ExecutionMode,
    pub graph: Option<ExecutableGraph>,
    pub schema_id: u32,
}

pub fn emit(
    program: &Program,
    decisions: &[RouteDecision],
    source_path: &Path,
    runtime_root: &Path,
    compiler_version: &str,
) -> Result<EmitResult, String> {
    fs::create_dir_all(runtime_root)
        .map_err(|error| format!("create {}: {error}", runtime_root.display()))?;
    for target in ["go", "python", "typescript", "ipc"] {
        fs::create_dir_all(runtime_root.join(target))
            .map_err(|error| format!("create target directory: {error}"))?;
    }

    let graph = infer_graph(program)?;
    let mode = graph
        .as_ref()
        .map(|g| g.mode)
        .unwrap_or(ExecutionMode::Stub);
    let schema_id = schema_id(program);
    let mut generated = Vec::new();
    let mut modules_json = Vec::new();

    if mode == ExecutionMode::Runnable {
        let g = graph.as_ref().unwrap();
        emit_runnable(runtime_root, g, schema_id, &mut generated)?;
    } else {
        for module in &program.modules {
            let decision = decision_for(decisions, module)?;
            let file_name = format!(
                "{}.{}",
                snake_case(&module.name),
                extension(decision.target)
            );
            let path = runtime_root
                .join(decision.target.runtime_dir())
                .join(&file_name);
            fs::write(&path, render_stub(module, decision))
                .map_err(|error| format!("write {}: {error}", path.display()))?;
            generated.push(path.clone());
            modules_json.push(module_manifest_entry(
                module,
                decision,
                path.strip_prefix(runtime_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
            ));
        }
    }

    if mode == ExecutionMode::Runnable {
        for module in &program.modules {
            let decision = decision_for(decisions, module)?;
            let generated_path = match decision.target {
                Target::Bun => "typescript/worker.ts".to_string(),
                Target::Python => "python/worker.py".to_string(),
                Target::Go => "go/worker.go".to_string(),
            };
            modules_json.push(module_manifest_entry(module, decision, generated_path));
        }
    }

    let manifest_path = runtime_root.join("manifest.json");
    let mut manifest = serde_json::json!({
        "manifest_version": 2,
        "language": "Silc",
        "compiler": "silc",
        "compiler_version": compiler_version,
        "source": source_path,
        "source_version": program.version,
        "execution_mode": match mode {
            ExecutionMode::Stub => "stub",
            ExecutionMode::Runnable => "runnable",
        },
        "schema_id": schema_id,
        "ipc_dir": IPC_DIR,
        "protocol_version": PROTOCOL_VERSION,
        "abi_version": ABI_VERSION,
        "modules": modules_json,
    });
    if let Some(g) = &graph {
        manifest["graph"] = serde_json::json!({
            "service": g.service,
            "processor": g.processor,
            "sink": g.sink,
        });
        manifest["http_port"] = serde_json::json!(g.http_port);
        manifest["sqlite_table"] = serde_json::json!(g.sqlite_table);
        manifest["entrypoints"] = serde_json::json!({
            "bun": "typescript/worker.ts",
            "python": "python/worker.py",
            "go_source": "go/worker.go",
            "go_binary": "go/worker",
            "supervisor_socket": SUPERVISOR_SOCKET,
        });
        manifest["engines"] = serde_json::json!({
            "bun": {"path": null, "version": "1.2.19"},
            "python": {"path": null, "version": "3.12.11"},
            "go": {"path": null, "version": "1.24.5"},
        });
    }

    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|error| format!("write {}: {error}", manifest_path.display()))?;

    Ok(EmitResult {
        root: runtime_root.to_path_buf(),
        manifest: manifest_path,
        generated,
        execution_mode: mode,
        graph,
        schema_id,
    })
}

fn emit_runnable(
    root: &Path,
    graph: &ExecutableGraph,
    schema_id: u32,
    generated: &mut Vec<PathBuf>,
) -> Result<(), String> {
    // Drop prior package members so `go build .` cannot see stale sources
    // (e.g. an older feedback_worker.go next to worker.go).
    clear_dir_sources(&root.join("go"), &["go"])?;
    clear_dir_sources(&root.join("typescript"), &["ts"])?;
    clear_dir_sources(&root.join("python"), &["py"])?;

    let files = [
        (
            root.join("typescript/worker.ts"),
            render_template(FEEDBACK_TS, graph, schema_id),
        ),
        (
            root.join("python/worker.py"),
            render_template(FEEDBACK_PY, graph, schema_id),
        ),
        (
            root.join("go/worker.go"),
            render_template(FEEDBACK_GO, graph, schema_id),
        ),
        (root.join("go/go.mod"), FEEDBACK_GOMOD.to_string()),
    ];
    for (path, contents) in files {
        fs::write(&path, contents).map_err(|error| format!("write {}: {error}", path.display()))?;
        generated.push(path);
    }
    Ok(())
}

fn clear_dir_sources(dir: &Path, extensions: &[&str]) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let matches = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| extensions.contains(&ext));
        if matches {
            fs::remove_file(&path).map_err(|e| format!("remove stale {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

fn render_template(template: &str, graph: &ExecutableGraph, schema_id: u32) -> String {
    template
        .replace("__PORT__", &graph.http_port.to_string())
        .replace("__TABLE__", &graph.sqlite_table)
        .replace("__SCHEMA_ID__", &schema_id.to_string())
        .replace("__SOCKET_PATH__", SUPERVISOR_SOCKET)
}

fn module_manifest_entry(
    module: &Module,
    decision: &RouteDecision,
    generated: String,
) -> serde_json::Value {
    serde_json::json!({
        "name": module.name,
        "kind": module.kind.as_str(),
        "target": decision.target.as_str(),
        "provenance": decision.provenance,
        "generated": generated,
        "methods": module.methods.iter().map(|m| &m.name).collect::<Vec<_>>(),
    })
}

fn decision_for<'a>(
    decisions: &'a [RouteDecision],
    module: &Module,
) -> Result<&'a RouteDecision, String> {
    decisions
        .iter()
        .find(|decision| decision.module == module.name)
        .ok_or_else(|| format!("missing route decision for {}", module.name))
}

fn schema_id(program: &Program) -> u32 {
    let mut hash = 2_166_136_261u32;
    for contract in &program.contracts {
        for byte in contract.name.as_bytes() {
            hash = (hash ^ *byte as u32).wrapping_mul(16_777_619);
        }
        for field in &contract.fields {
            for byte in field.name.as_bytes() {
                hash = (hash ^ *byte as u32).wrapping_mul(16_777_619);
            }
            for byte in field.ty.name().as_bytes() {
                hash = (hash ^ *byte as u32).wrapping_mul(16_777_619);
            }
        }
    }
    hash
}

fn render_stub(module: &Module, decision: &RouteDecision) -> String {
    let methods = module
        .methods
        .iter()
        .map(|method| method.name.as_str())
        .collect::<Vec<_>>();
    match decision.target {
        Target::Bun => {
            let bodies = methods
                .iter()
                .map(|name| {
                    format!(
                        "  async {name}(): Promise<void> {{\n    // TODO: operation is not executable in Silc v1\n  }}"
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            format!(
                "// AUTO-GENERATED BY silc — DO NOT EDIT\n// {}\nexport class {} {{\n{}\n}}\n",
                decision.provenance, module.name, bodies
            )
        }
        Target::Python => {
            let bodies = methods
                .iter()
                .map(|name| {
                    format!(
                        "    def {name}(self):\n        # TODO: operation is not executable in Silc v1\n        pass"
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            format!(
                "# AUTO-GENERATED BY silc — DO NOT EDIT\n# {}\nclass {}:\n{}\n",
                decision.provenance, module.name, bodies
            )
        }
        Target::Go => {
            let bodies = methods
                .iter()
                .map(|name| {
                    format!(
                        "func (m *{}) {}() {{\n\t// TODO: operation is not executable in Silc v1\n}}",
                        module.name,
                        pascal_case(name)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            format!(
                "// AUTO-GENERATED BY silc — DO NOT EDIT\n// {}\npackage generated\n\ntype {} struct {{}}\n\n{}\n",
                decision.provenance, module.name, bodies
            )
        }
    }
}

fn extension(target: Target) -> &'static str {
    match target {
        Target::Go => "go",
        Target::Python => "py",
        Target::Bun => "ts",
    }
}

fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && index > 0 {
            out.push('_');
        }
        out.extend(ch.to_lowercase());
    }
    out
}

fn pascal_case(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn output_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("silc-{label}-{nonce}"))
    }

    #[test]
    fn parse_route_emit_all_examples() {
        let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        for name in [
            "article_pipeline.silc",
            "sensor_alert.silc",
            "csv_summary.raku",
            "url_health.silc",
            "log_anomaly.raku",
        ] {
            let source_path = examples.join(name);
            let source = fs::read_to_string(&source_path).expect("read example");
            let program = sil_parser::parse(&source).expect("parse example");
            program.validate().expect("validate example");
            let decisions = sil_router::route_program(&program);
            let output = output_dir(name);
            let result = emit(&program, &decisions, &source_path, &output, "test").expect("emit");
            assert_eq!(result.execution_mode, ExecutionMode::Stub);
            let manifest = fs::read_to_string(&result.manifest).unwrap();
            assert!(manifest.contains("\"execution_mode\": \"stub\""));
            fs::remove_dir_all(output).ok();
        }
    }

    #[test]
    fn emits_runnable_feedback_workers() {
        let source_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/feedback_portal.silc");
        let source = fs::read_to_string(&source_path).unwrap();
        let program = sil_parser::parse(&source).expect("parse feedback");
        program.validate().expect("validate feedback");
        let decisions = sil_router::route_program(&program);
        let output = output_dir("feedback");
        let result = emit(&program, &decisions, &source_path, &output, "test").unwrap();
        assert_eq!(result.execution_mode, ExecutionMode::Runnable);

        let ts = fs::read_to_string(output.join("typescript/worker.ts")).unwrap();
        let py = fs::read_to_string(output.join("python/worker.py")).unwrap();
        let go = fs::read_to_string(output.join("go/worker.go")).unwrap();
        for source in [&ts, &py, &go] {
            assert!(!source.contains("TODO"));
            assert!(!source.contains("__PORT__"));
            assert!(!source.contains("__TABLE__"));
            assert!(!source.contains("__SCHEMA_ID__"));
            assert!(!source.contains("__SOCKET_PATH__"));
        }
        assert!(ts.contains("HELLO"));
        assert!(ts.contains(r#"role: "bun""#));
        assert!(ts.contains("/submit"));
        assert!(ts.contains(r#"type: "INGEST""#));
        assert!(py.contains(r#""role": "python""#));
        assert!(py.contains(r#"!= "python""#));
        assert!(py.contains("hashlib"));
        assert!(go.contains(r#""role": "go""#));
        assert!(go.contains("journal_mode=WAL"));
        assert!(go.contains("feedback"));

        let manifest = fs::read_to_string(&result.manifest).unwrap();
        assert!(manifest.contains("\"execution_mode\": \"runnable\""));
        assert!(manifest.contains("\"manifest_version\": 2"));
        assert!(manifest.contains("\"http_port\": 18080"));
        assert!(manifest.contains("\"sqlite_table\": \"feedback\""));
        assert!(manifest.contains("\"ipc_dir\": \"ipc\""));
        assert!(manifest.contains("\"WebPortal\""));
        assert!(manifest.contains("\"TextAnalyzer\""));
        assert!(manifest.contains("\"FeedbackDb\""));
        assert!(manifest.contains("\"engines\""));
        fs::remove_dir_all(output).ok();
    }
}
