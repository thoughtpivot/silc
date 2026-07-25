//! Silc 0.2.0 code generation: inspectable stubs and runnable dual-surface apps.

mod ui_lower;

use sil_core::{
    infer_graph, ApiRoute, Contract, ExecutableGraph, ExecutionMode, Module, ProcessorOp, Program,
    ResourceKind, Target,
};
use sil_ipc::{ABI_VERSION, PROTOCOL_VERSION};
use sil_router::RouteDecision;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const IPC_DIR: &str = "ipc";
const SUPERVISOR_SOCKET: &str = "ipc/supervisor.sock";
const DEFAULT_DB_PATH: &str = "ipc/app.db";

const APP_WORKER_TS: &str = include_str!("../templates/app_worker.ts");
const PROCESSOR_WORKER_PY: &str = include_str!("../templates/processor_worker.py");
const STORE_WORKER_GO: &str = include_str!("../templates/store_worker.go");
const LLM_REQUIREMENTS: &str = include_str!("../templates/llm_requirements.txt");
const STORE_GOMOD: &str = include_str!("../templates/go.mod");
const SERVICE_HTTP_GO: &str = include_str!("../templates/service_http_worker.go");
const SERVICE_HTTP_GOMOD: &str = include_str!("../templates/service_http_go.mod");
const UI_WEB_PACKAGE_JSON: &str = include_str!("../templates/ui_web_package.json");
const UI_WEB_BUN_LOCK: &str = include_str!("../templates/ui_web_bun.lock");
const UI_WEB_INDEX_HTML: &str = include_str!("../templates/ui_web_index.html");
const UI_WEB_TAILWIND_CONFIG: &str = include_str!("../templates/ui_web_tailwind.config.js");
const UI_WEB_MAIN_TSX: &str = include_str!("../templates/ui_web_main.tsx");
const UI_WEB_THEME_CSS: &str = include_str!("../templates/ui_web_theme.css");
const UI_WEB_UTILS_TS: &str = include_str!("../templates/ui_web_utils.ts");
const UI_WEB_BUTTON_TSX: &str = include_str!("../templates/ui_web_button.tsx");
const UI_WEB_INPUT_TSX: &str = include_str!("../templates/ui_web_input.tsx");
const UI_WEB_LABEL_TSX: &str = include_str!("../templates/ui_web_label.tsx");
const UI_WEB_TEXTAREA_TSX: &str = include_str!("../templates/ui_web_textarea.tsx");
const UI_WEB_CARD_TSX: &str = include_str!("../templates/ui_web_card.tsx");
const UI_WEB_RADIO_GROUP_TSX: &str = include_str!("../templates/ui_web_radio_group.tsx");
const UI_WEB_APP_BAR_TSX: &str = include_str!("../templates/ui_web_app_bar.tsx");
const UI_WEB_LOGO_TSX: &str = include_str!("../templates/ui_web_logo.tsx");
const UI_WEB_SIDE_PANEL_TSX: &str = include_str!("../templates/ui_web_side_panel.tsx");
const UI_WEB_NAV_ITEM_TSX: &str = include_str!("../templates/ui_web_nav_item.tsx");
const UI_WEB_TOOLBAR_TSX: &str = include_str!("../templates/ui_web_toolbar.tsx");
const UI_WEB_CHAT_THREAD_TSX: &str = include_str!("../templates/ui_web_chat_thread.tsx");
const UI_WEB_CHAT_COMPOSER_TSX: &str = include_str!("../templates/ui_web_chat_composer.tsx");
const UI_WEB_HISTORY_PANEL_TSX: &str = include_str!("../templates/ui_web_history_panel.tsx");
const UI_WEB_SEARCH_INPUT_TSX: &str = include_str!("../templates/ui_web_search_input.tsx");
const UI_WEB_PRODUCT_GRID_TSX: &str = include_str!("../templates/ui_web_product_grid.tsx");

/// Compiler-pinned React version for ui::web (must match ui_web_package.json).
pub const UI_WEB_REACT_VERSION: &str = "18.3.1";
pub const UI_WEB_TAILWIND_VERSION: &str = "3.4.17";
pub const UI_WEB_SUBSTRATE: &str = "react";
/// Compiler-owned Gin adapter for `service::http`.
pub const SERVICE_HTTP_ADAPTER: &str = "gin-v1";
pub const SERVICE_HTTP_GIN_VERSION: &str = "1.10.0";

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
        emit_runnable(runtime_root, program, g, schema_id, &mut generated)?;
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
        let g = graph.as_ref().unwrap();
        for module in &program.modules {
            let decision = decision_for(decisions, module)?;
            let generated_path = match decision.target {
                Target::Bun => "typescript/worker.ts".to_string(),
                Target::Python => "python/worker.py".to_string(),
                Target::Go if g.is_api_only() => "go/api/worker.go".to_string(),
                Target::Go => "go/worker.go".to_string(),
            };
            modules_json.push(module_manifest_entry(module, decision, generated_path));
        }
    }

    let manifest_path = runtime_root.join("manifest.json");
    let mut manifest = serde_json::json!({
        "manifest_version": 3,
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
        let mut surfaces = Vec::new();
        if g.capabilities.web {
            surfaces.push("web");
        }
        if g.capabilities.terminal {
            surfaces.push("terminal");
        }
        manifest["graph"] = serde_json::json!({
            "service": g.service,
            "processor": g.processor,
            "sink": g.sink,
            "api_only": g.is_api_only(),
            "processor_op": g.processor_op.as_str(),
            "capabilities": {
                "web": g.capabilities.web,
                "terminal": g.capabilities.terminal,
                "score": g.capabilities.score,
                "llm": g.capabilities.llm,
                "history": g.capabilities.history,
                "resources": g.capabilities.resources,
            },
            "app": g.app_name,
            "root_component": g.root_component,
            "model_ref": g.model_ref,
            "actions": actions_json(g),
            "resource_tables": g.resource_tables.iter().map(|(name, table)| {
                serde_json::json!({ "resource": name, "table": table })
            }).collect::<Vec<_>>(),
            "surfaces": surfaces,
        });
        manifest["http_port"] = serde_json::json!(g.http_port);
        manifest["http_route"] = serde_json::json!(g.http_route);
        manifest["terminal_port"] = serde_json::json!(g.terminal_port);
        manifest["sqlite_table"] = serde_json::json!(g.sqlite_table);
        if g.has_ui() {
            manifest["ui"] = serde_json::json!({
                "profile": "web",
                "surfaces": ["web", "terminal"],
                "substrate": UI_WEB_SUBSTRATE,
                "terminal_substrate": if g.terminal_port.is_some() { "bun-tcp-telnet" } else { "disabled" },
                "react_version": UI_WEB_REACT_VERSION,
                "tailwind_version": UI_WEB_TAILWIND_VERSION,
                "app": g.app_name,
                "root_component": g.root_component,
                "assets": [
                    "typescript/src/main.tsx",
                    "typescript/src/App.tsx",
                    "typescript/terminal.ts",
                    "typescript/src/theme.css",
                    "typescript/src/lib/utils.ts",
                    "typescript/src/components/ui/button.tsx",
                    "typescript/src/components/ui/input.tsx",
                    "typescript/src/components/ui/label.tsx",
                    "typescript/src/components/ui/textarea.tsx",
                    "typescript/src/components/ui/card.tsx",
                    "typescript/src/components/ui/radio-group.tsx",
                    "typescript/src/components/ui/app-bar.tsx",
                    "typescript/src/components/ui/logo.tsx",
                    "typescript/src/components/ui/side-panel.tsx",
                    "typescript/src/components/ui/nav-item.tsx",
                    "typescript/src/components/ui/toolbar.tsx",
                    "typescript/src/components/ui/chat-thread.tsx",
                    "typescript/src/components/ui/chat-composer.tsx",
                    "typescript/src/components/ui/history-panel.tsx",
                    "typescript/src/components/ui/search-input.tsx",
                    "typescript/src/components/ui/product-grid.tsx",
                    "typescript/tailwind.config.js",
                    "typescript/index.html",
                    "typescript/package.json",
                    "typescript/bun.lock",
                    "typescript/dist/index.html",
                    "typescript/dist/assets/app.js",
                    "typescript/dist/assets/theme.css",
                ],
                "dependencies": {
                    "react": UI_WEB_REACT_VERSION,
                    "react-dom": UI_WEB_REACT_VERSION,
                    "tailwindcss": UI_WEB_TAILWIND_VERSION,
                    "clsx": "2.1.1",
                    "tailwind-merge": "2.6.0",
                },
                "provenance": "compiler-owned ui::web + ui::terminal → React/Tailwind/Bun",
                "catalog": sil_core::catalog_component_names(),
            });
        }
        if g.has_api() {
            manifest["services"] = serde_json::json!({
                "profile": "http",
                "adapter": SERVICE_HTTP_ADAPTER,
                "engine": "go",
                "gin_version": SERVICE_HTTP_GIN_VERSION,
                "port": g.api_port(),
                "routes": g.api_routes.iter().map(|r| serde_json::json!({
                    "port": r.port,
                    "path": r.path,
                    "method": r.method,
                    "contract": r.contract,
                })).collect::<Vec<_>>(),
                "provenance": "compiler-owned service::http → Go/Gin",
            });
        }
        let mut entrypoints = serde_json::Map::new();
        if g.has_ui() {
            entrypoints.insert("bun".into(), serde_json::json!("typescript/worker.ts"));
            entrypoints.insert("python".into(), serde_json::json!("python/worker.py"));
            entrypoints.insert("go_source".into(), serde_json::json!("go/worker.go"));
            entrypoints.insert("go_binary".into(), serde_json::json!("go/worker"));
            entrypoints.insert(
                "ui_web_entry".into(),
                serde_json::json!("typescript/src/main.tsx"),
            );
            entrypoints.insert(
                "terminal".into(),
                serde_json::json!("typescript/terminal.ts"),
            );
            entrypoints.insert(
                "supervisor_socket".into(),
                serde_json::json!(SUPERVISOR_SOCKET),
            );
        }
        if g.has_api() {
            entrypoints.insert("api_source".into(), serde_json::json!("go/api/worker.go"));
            entrypoints.insert("api_binary".into(), serde_json::json!("go/api/worker"));
        }
        manifest["entrypoints"] = serde_json::Value::Object(entrypoints);
        manifest["engines"] = serde_json::json!({
            "bun": {"path": null, "version": "1.2.18"},
            "python": {"path": null, "version": "3.12.12"},
            "go": {"path": null, "version": "1.23.6"},
        });
        manifest["actions"] = actions_json(g);
        manifest["processor"] = serde_json::json!(g.processor_op.as_str());
        if g.has_ui() {
            manifest["surfaces"] = serde_json::json!(["web", "terminal"]);
        }
        manifest["capabilities"] = serde_json::json!({
            "web": g.capabilities.web,
            "terminal": g.capabilities.terminal,
            "score": g.capabilities.score,
            "llm": g.capabilities.llm,
            "history": g.capabilities.history,
            "resources": g.capabilities.resources,
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
    program: &Program,
    graph: &ExecutableGraph,
    schema_id: u32,
    generated: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if graph.has_ui() {
        emit_ui_app(root, program, graph, schema_id, generated)?;
    }
    if graph.has_api() {
        emit_service_http(root, program, graph, schema_id, generated)?;
    }
    Ok(())
}

fn emit_ui_app(
    root: &Path,
    program: &Program,
    graph: &ExecutableGraph,
    schema_id: u32,
    generated: &mut Vec<PathBuf>,
) -> Result<(), String> {
    // Drop prior package members so `go build .` cannot see stale sources.
    clear_dir_sources(&root.join("go"), &["go"])?;
    clear_dir_sources(&root.join("typescript"), &["ts", "tsx", "js"])?;
    clear_dir_sources(&root.join("python"), &["py"])?;
    let ts_src = root.join("typescript/src");
    let ts_dist = root.join("typescript/dist");
    if ts_src.is_dir() {
        fs::remove_dir_all(&ts_src)
            .map_err(|error| format!("clear {}: {error}", ts_src.display()))?;
    }
    fs::create_dir_all(ts_src.join("components/ui"))
        .map_err(|error| format!("create {}: {error}", ts_src.display()))?;
    fs::create_dir_all(ts_src.join("lib"))
        .map_err(|error| format!("create {}: {error}", ts_src.display()))?;
    fs::create_dir_all(&ts_dist)
        .map_err(|error| format!("create {}: {error}", ts_dist.display()))?;
    clear_dir_sources(&ts_dist, &["js", "css", "html", "map"])?;

    let app_tsx = ui_lower::render_web_app(program, graph);
    let terminal_ts = ui_lower::render_terminal_module(program, graph);

    let mut files = vec![
        (
            root.join("typescript/worker.ts"),
            render_template(APP_WORKER_TS, graph, schema_id),
        ),
        (root.join("typescript/terminal.ts"), terminal_ts),
        (
            root.join("typescript/package.json"),
            UI_WEB_PACKAGE_JSON.to_string(),
        ),
        (
            root.join("typescript/bun.lock"),
            UI_WEB_BUN_LOCK.to_string(),
        ),
        (
            root.join("typescript/tailwind.config.js"),
            UI_WEB_TAILWIND_CONFIG.to_string(),
        ),
        (
            root.join("typescript/index.html"),
            UI_WEB_INDEX_HTML.to_string(),
        ),
        (
            root.join("typescript/src/main.tsx"),
            UI_WEB_MAIN_TSX.to_string(),
        ),
        (root.join("typescript/src/App.tsx"), app_tsx),
        (
            root.join("typescript/src/theme.css"),
            UI_WEB_THEME_CSS.to_string(),
        ),
        (
            root.join("typescript/src/lib/utils.ts"),
            UI_WEB_UTILS_TS.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/button.tsx"),
            UI_WEB_BUTTON_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/input.tsx"),
            UI_WEB_INPUT_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/label.tsx"),
            UI_WEB_LABEL_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/textarea.tsx"),
            UI_WEB_TEXTAREA_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/card.tsx"),
            UI_WEB_CARD_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/radio-group.tsx"),
            UI_WEB_RADIO_GROUP_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/app-bar.tsx"),
            UI_WEB_APP_BAR_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/logo.tsx"),
            UI_WEB_LOGO_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/side-panel.tsx"),
            UI_WEB_SIDE_PANEL_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/nav-item.tsx"),
            UI_WEB_NAV_ITEM_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/toolbar.tsx"),
            UI_WEB_TOOLBAR_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/chat-thread.tsx"),
            UI_WEB_CHAT_THREAD_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/chat-composer.tsx"),
            UI_WEB_CHAT_COMPOSER_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/history-panel.tsx"),
            UI_WEB_HISTORY_PANEL_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/search-input.tsx"),
            UI_WEB_SEARCH_INPUT_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/product-grid.tsx"),
            UI_WEB_PRODUCT_GRID_TSX.to_string(),
        ),
        (
            root.join("python/worker.py"),
            render_template(PROCESSOR_WORKER_PY, graph, schema_id),
        ),
        (
            root.join("go/worker.go"),
            render_template(STORE_WORKER_GO, graph, schema_id),
        ),
        (root.join("go/go.mod"), STORE_GOMOD.to_string()),
    ];
    if graph.needs_llm() {
        files.push((
            root.join("python/requirements.txt"),
            LLM_REQUIREMENTS.to_string(),
        ));
    }
    for (path, contents) in files {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        fs::write(&path, contents).map_err(|error| format!("write {}: {error}", path.display()))?;
        generated.push(path);
    }
    Ok(())
}

fn emit_service_http(
    root: &Path,
    program: &Program,
    graph: &ExecutableGraph,
    schema_id: u32,
    generated: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let api_dir = root.join("go/api");
    fs::create_dir_all(&api_dir)
        .map_err(|error| format!("create {}: {error}", api_dir.display()))?;
    clear_dir_sources(&api_dir, &["go"])?;

    let contracts_used: BTreeSet<&str> = graph
        .api_routes
        .iter()
        .map(|r| r.contract.as_str())
        .collect();
    let mut structs = String::new();
    let mut stores = String::new();
    for name in &contracts_used {
        let contract = program
            .contracts
            .iter()
            .find(|c| c.name == *name)
            .ok_or_else(|| format!("missing Contract `{name}` for service::http"))?;
        structs.push_str(&render_go_contract_struct(contract));
        structs.push('\n');
        let store_name = format!("store{}", contract.name);
        stores.push_str(&format!(
            "var {store_name} = struct {{\n\tmu    sync.Mutex\n\titems []{}\n}}{{\n\titems: []{}{{}},\n}}\n\n",
            contract.name, contract.name
        ));
    }

    let mut routes = String::new();
    for route in &graph.api_routes {
        routes.push_str(&render_go_route(route));
    }

    let port = graph.api_port().unwrap_or(8080);
    let body = SERVICE_HTTP_GO
        .replace("__STRUCTS__", structs.trim_end())
        .replace("__STORES__", stores.trim_end())
        .replace("__ROUTES__", &routes)
        .replace("__SCHEMA_ID__", &schema_id.to_string())
        .replace("__PORT__", &port.to_string());

    let worker_path = api_dir.join("worker.go");
    let gomod_path = api_dir.join("go.mod");
    fs::write(&worker_path, body)
        .map_err(|error| format!("write {}: {error}", worker_path.display()))?;
    fs::write(&gomod_path, SERVICE_HTTP_GOMOD)
        .map_err(|error| format!("write {}: {error}", gomod_path.display()))?;
    generated.push(worker_path);
    generated.push(gomod_path);
    Ok(())
}

fn render_go_contract_struct(contract: &Contract) -> String {
    let mut out = format!("type {} struct {{\n", contract.name);
    for field in &contract.fields {
        let go_ty = go_type_for(field.ty.name());
        let json = field.name.as_str();
        out.push_str(&format!(
            "\t{} {} `json:\"{}\"`\n",
            pascal_case(&field.name),
            go_ty,
            json
        ));
    }
    out.push_str("}\n");
    out
}

fn go_type_for(ty: &str) -> &'static str {
    match ty {
        "Int" | "Int32" | "int32" => "int32",
        "Int64" | "int64" => "int64",
        "num32" | "Float32" => "float32",
        "num64" | "Float64" | "Num" => "float64",
        "Bool" | "bool" => "bool",
        _ => "string",
    }
}

fn render_go_route(route: &ApiRoute) -> String {
    let store = format!("store{}", route.contract);
    let path = route.path.replace('"', "");
    match route.method.as_str() {
        "GET" => format!(
            "\tr.GET(\"{path}\", func(c *gin.Context) {{\n\t\t{store}.mu.Lock()\n\t\tdefer {store}.mu.Unlock()\n\t\tc.JSON(http.StatusOK, {store}.items)\n\t}})\n"
        ),
        "POST" => format!(
            "\tr.POST(\"{path}\", func(c *gin.Context) {{\n\t\tvar item {}\n\t\tif err := c.ShouldBindJSON(&item); err != nil {{\n\t\t\tc.JSON(http.StatusBadRequest, gin.H{{\"error\": err.Error()}})\n\t\t\treturn\n\t\t}}\n\t\t{store}.mu.Lock()\n\t\t{store}.items = append({store}.items, item)\n\t\t{store}.mu.Unlock()\n\t\tc.JSON(http.StatusCreated, item)\n\t}})\n",
            route.contract
        ),
        other => format!("\t// unsupported method {other} for {path}\n"),
    }
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

fn actions_json(graph: &ExecutableGraph) -> serde_json::Value {
    serde_json::Value::Array(
        graph
            .actions
            .iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.id,
                    "resource": a.resource,
                    "method": a.method,
                    "http_method": a.http_method,
                    "path": a.path,
                    "kind": match a.kind {
                        ResourceKind::Query => "query",
                        ResourceKind::Mutation => "mutation",
                    },
                })
            })
            .collect(),
    )
}

fn render_template(template: &str, graph: &ExecutableGraph, schema_id: u32) -> String {
    let actions =
        serde_json::to_string_pretty(&actions_json(graph)).unwrap_or_else(|_| "[]".into());
    let resource_tables = serde_json::to_string(
        &graph
            .resource_tables
            .iter()
            .map(|(_, table)| table.clone())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());
    let routes = match &graph.app {
        Some(app) => serde_json::to_string(
            &app.routes
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "path": r.path,
                        "component": r.component,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| "[]".into()),
        None => "[]".into(),
    };
    let has_llm = if graph.needs_llm() { "true" } else { "false" };
    let processor = if graph.processor_op == ProcessorOp::None {
        "none"
    } else {
        graph.processor_op.as_str()
    };

    template
        .replace("__PORT__", &graph.http_port.to_string())
        .replace("__ROUTE__", &graph.http_route)
        .replace(
            "__TERMINAL_PORT__",
            &graph.terminal_port.unwrap_or_default().to_string(),
        )
        .replace("__SOCKET_PATH__", SUPERVISOR_SOCKET)
        .replace("__DB_PATH__", DEFAULT_DB_PATH)
        .replace("__TABLE__", &graph.sqlite_table)
        .replace("__SCHEMA_ID__", &schema_id.to_string())
        .replace("__PROCESSOR_OP__", processor)
        .replace("__HAS_LLM__", has_llm)
        .replace("__ACTIONS_JSON__", &actions)
        .replace("__RESOURCE_TABLES__", &resource_tables)
        .replace("__ROUTES_JSON__", &routes)
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
                        "  async {name}(): Promise<void> {{\n    // TODO: operation is not executable in Silc 0.2.0\n  }}"
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
                        "    def {name}(self):\n        # TODO: operation is not executable in Silc 0.2.0\n        pass"
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
                        "func (m *{}) {}() {{\n\t// TODO: operation is not executable in Silc 0.2.0\n}}",
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

    fn parse_emit(source: &str, label: &str) -> (Program, EmitResult, PathBuf) {
        let program = sil_parser::parse(source).expect("parse");
        program.validate().expect("validate");
        let decisions = sil_router::route_program(&program);
        let output = output_dir(label);
        let source_path = PathBuf::from(format!("{label}.silc"));
        let result = emit(&program, &decisions, &source_path, &output, "test").expect("emit");
        (program, result, output)
    }

    const STUB_SOURCE: &str = r#"
@version("0.2.0")
class Payload { has Str $.text; }
class Ingress is service {
    method fetch() { $url ==> http::get() ==> html::extract_body() }
}
class Engine is processor {
    method run(Payload $p) { $p.text ==> tensor::tokenize() }
}
class Cache is sink is latency(2ms) {
    method persist(Payload $p) { $p ==> store::upsert_primary() }
}
"#;

    const FEEDBACK_SOURCE: &str = r#"
@version("0.2.0")
class FeedbackRecord {
    has Str $.author;
    has Str $.text;
}
class FeedbackPage is component {
    has state Str $.author = "";
    has state Str $.text = "";
    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Feedback"))),
            ui::heading(:text("Share feedback")),
            ui::form(:on(submit(on_submit)),
                ui::text_input(:field(author), :label("Author")),
                ui::textarea(:field(text), :label("Text")),
                ui::button(:label("Send"), :variant(primary), :submit)
            )
        )
    }
    method on_submit() {
        submit();
    }
}
class FeedbackApp is app {
    route "/" => FeedbackPage;
    method serve() {
        ui::web(:root(FeedbackApp), :port(18080), :route("/"))
            ==> ui::terminal(:port(18023))
    }
}
class TextAnalyzer is processor {
    method analyze(FeedbackRecord $record) {
        $record.text ==> text::score()
    }
}
class FeedbackDb is sink is storage(SQLite) {
    method persist(FeedbackRecord $record) {
        $record ==> ipc::publish() ==> store::sqlite(:table(feedback)) ==> store::commit()
    }
}
"#;

    const CHAT_SOURCE: &str = r#"
@version("0.2.0")
class ChatRecord {
    has Str $.prompt;
    has Str $.reply;
}
class ChatPage is component {
    has state Str $.prompt = "";
    has state Str $.active_session = "session-a";
    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Chat"))),
            :side_panel(ui::side_panel(
                ui::nav_item(:label("Home"), :to("/"))
            )),
            ui::chat_history(:title("Chat History"), :collapsible),
            when $.active_session {
                ui::text(:text("Session selected"))
            },
            ui::chat(
                :value($.prompt),
                :session($.active_session),
                :on(send(on_send))
            )
        )
    }
    method on_send() {
        Assistant.complete();
    }
}
class ChatApp is app {
    route "/" => ChatPage;
    method serve() {
        ui::web(:root(ChatApp), :port(18090), :route("/"))
            ==> ui::terminal(:port(18023))
    }
}
class Assistant is processor {
    has Str $.model_ref = "silclm";
    method complete(ChatRecord $record) {
        $record.prompt ==> llm::complete(:model("silclm"))
    }
}
class ChatDb is sink is storage(SQLite) {
    method persist(ChatRecord $record) {
        $record ==> ipc::publish() ==> store::sqlite(:table(chats)) ==> store::commit()
    }
}
"#;

    const RESOURCE_SOURCE: &str = r#"
@version("0.2.0")
class Product {
    has Str $.name;
    has num64 $.price;
}
class Products is resource {
    has Str $.table = "products";
    query list() -> Product {
        Product ==> resource::list()
    }
    mutation create(Product $item) -> Product {
        $item ==> resource::create()
    }
}
class ShopPage is component {
    query $.products = Products.list();
    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Shop"))),
            ui::heading(:text("Products")),
            ui::collection(:items($.products),
                for $.products -> $product {
                    ui::card(ui::heading(:text($.product.name)))
                }
            )
        )
    }
}
class ShopApp is app {
    route "/" => ShopPage;
    method serve() {
        ui::web(:root(ShopApp), :port(18088), :route("/"))
            ==> ui::terminal(:port(18023))
    }
}
"#;

    const API_SOURCE: &str = r#"
@version("0.2.0")
class FeedbackRecord {
    has UUID $.id;
    has Str $.author;
    has Str $.text;
}
class FeedbackApi is service {
    method list(:$port = 18081) {
        FeedbackRecord ==> service::http(:port(18081), :route("/api/feedback"), :method(GET))
    }
    method create(:$port = 18081) {
        FeedbackRecord ==> service::http(:port(18081), :route("/api/feedback"), :method(POST))
    }
}
"#;

    #[test]
    fn emits_stub_modules() {
        let (_program, result, output) = parse_emit(STUB_SOURCE, "stub");
        assert_eq!(result.execution_mode, ExecutionMode::Stub);
        let manifest = fs::read_to_string(&result.manifest).unwrap();
        assert!(manifest.contains("\"execution_mode\": \"stub\""));
        assert!(manifest.contains("\"manifest_version\": 3"));
        assert!(output.join("typescript/ingress.ts").is_file());
        assert!(output.join("python/engine.py").is_file());
        assert!(output.join("go/cache.go").is_file());
        fs::remove_dir_all(output).ok();
    }

    #[test]
    fn emits_runnable_feedback_app() {
        let (_program, result, output) = parse_emit(FEEDBACK_SOURCE, "feedback");
        assert_eq!(result.execution_mode, ExecutionMode::Runnable);
        let graph = result.graph.as_ref().unwrap();
        assert_eq!(graph.processor_op, ProcessorOp::Score);
        assert!(graph.capabilities.web);
        assert!(graph.capabilities.terminal);
        assert_eq!(graph.sqlite_table, "feedback");

        let ts = fs::read_to_string(output.join("typescript/worker.ts")).unwrap();
        let py = fs::read_to_string(output.join("python/worker.py")).unwrap();
        let go = fs::read_to_string(output.join("go/worker.go")).unwrap();
        let app = fs::read_to_string(output.join("typescript/src/App.tsx")).unwrap();
        let terminal = fs::read_to_string(output.join("typescript/terminal.ts")).unwrap();
        let button =
            fs::read_to_string(output.join("typescript/src/components/ui/button.tsx")).unwrap();
        let theme = fs::read_to_string(output.join("typescript/src/theme.css")).unwrap();
        let pkg = fs::read_to_string(output.join("typescript/package.json")).unwrap();

        for source in [&ts, &py, &go] {
            assert!(!source.contains("TODO"));
            assert!(!source.contains("__PORT__"));
            assert!(!source.contains("__ROUTE__"));
            assert!(!source.contains("__TABLE__"));
            assert!(!source.contains("__SCHEMA_ID__"));
            assert!(!source.contains("__SOCKET_PATH__"));
            assert!(!source.contains("__DB_PATH__"));
            assert!(!source.contains("__PROCESSOR_OP__"));
            assert!(!source.contains("__HAS_LLM__"));
            assert!(!source.contains("__ACTIONS_JSON__"));
            assert!(!source.contains("__RESOURCE_TABLES__"));
            assert!(!source.contains("__ROUTES_JSON__"));
        }
        assert!(ts.contains("HELLO"));
        assert!(ts.contains(r#"role: "bun""#));
        assert!(ts.contains("/submit"));
        assert!(ts.contains(r#"type: "INGEST""#));
        assert!(ts.contains("dist"));
        assert!(ts.contains("text.score"));
        assert!(py.contains("text.score"));
        assert!(go.contains("feedback") || go.contains("SILC_TABLE"));
        assert!(app.contains("function FeedbackPage"));
        assert!(app.contains("function App"));
        assert!(app.contains("AppBar"));
        assert!(app.contains("/submit"));
        assert!(app.contains("setAuthor"));
        assert!(
            app.contains("h-screen overflow-hidden")
                && app.contains("min-h-0 flex-1 flex")
                && app.contains("min-h-0 min-w-0 flex-1 overflow-y-auto")
                && !app.contains("min-h-screen"),
            "ui::page must lower to a viewport-height shell with a scrollable main"
        );
        let app_bar =
            fs::read_to_string(output.join("typescript/src/components/ui/app-bar.tsx")).unwrap();
        assert!(
            app_bar.contains("sticky top-0 z-20") && app_bar.contains("shrink-0"),
            "app bar must stay pinned inside the page shell"
        );
        let side_panel =
            fs::read_to_string(output.join("typescript/src/components/ui/side-panel.tsx")).unwrap();
        assert!(
            side_panel.contains("h-full") && side_panel.contains("overflow-y-auto"),
            "side panel must scroll independently"
        );
        assert!(terminal.contains("ROUTES"));
        assert!(button.contains("ShadCN-style Button") || button.contains("Button"));
        assert!(theme.contains("@tailwind"));
        assert!(pkg.contains(UI_WEB_REACT_VERSION));

        let manifest = fs::read_to_string(&result.manifest).unwrap();
        assert!(manifest.contains("\"execution_mode\": \"runnable\""));
        assert!(manifest.contains("\"manifest_version\": 3"));
        assert!(!manifest.contains("portal_kind"));
        assert!(manifest.contains("\"http_port\": 18080"));
        assert!(manifest.contains("\"sqlite_table\": \"feedback\""));
        assert!(manifest.contains("\"surfaces\""));
        assert!(manifest.contains("\"capabilities\""));
        assert!(manifest.contains("text.score") || manifest.contains("\"processor\""));
        assert!(manifest.contains("\"terminal_port\": 18023"));
        assert!(manifest.contains("FeedbackApp") || manifest.contains("FeedbackPage"));
        fs::remove_dir_all(output).ok();
    }

    #[test]
    fn emits_chat_app_with_llm_processor() {
        let (_program, result, output) = parse_emit(CHAT_SOURCE, "chat");
        let graph = result.graph.as_ref().unwrap();
        assert_eq!(graph.processor_op, ProcessorOp::LlmComplete);
        assert!(graph.needs_llm());
        assert_eq!(graph.model_ref.as_deref(), Some("silclm"));

        let app = fs::read_to_string(output.join("typescript/src/App.tsx")).unwrap();
        assert!(app.contains("ChatComposer") || app.contains("chat"));
        assert!(
            app.contains("<ChatComposer") && app.contains("onSubmit=") && app.contains("id="),
            "ChatComposer must receive id and onSubmit, got:\n{app}"
        );
        assert!(
            !app.contains("onSend="),
            "ChatComposer must not use onSend prop"
        );
        assert!(
            app.contains("ChatThread") && app.contains("messages={messages}"),
            "chat UI must render ChatThread with messages state, got:\n{app}"
        );
        assert!(
            app.contains("setMessages") && app.contains("setThinking"),
            "chat UI must track messages and thinking state"
        );
        assert!(app.contains("/history"), "chat UI must load /history");
        assert!(
            app.contains("__chatComplete") && !app.contains("items={[]}"),
            "chat UI must use __chatComplete and must not hardcode empty history"
        );
        assert!(
            app.contains("AbortController")
                && app.contains("historyRequestRef")
                && app.contains("activeSessionRef"),
            "session history must abort and ignore stale responses"
        );
        assert!(
            app.contains("pendingId")
                && app.contains("pending: true")
                && app.contains("capturedSession"),
            "chat send must be optimistic and guarded by its captured session"
        );
        assert!(
            app.contains("setChatError")
                && app.contains("setHistoryError")
                && app.contains("draft || prompt"),
            "chat failures must be visible and restore the draft"
        );
        assert!(
            app.contains("focusKey={active_session}"),
            "selecting a session must focus the chat composer"
        );
        assert!(
            app.contains("(active_session) ?"),
            "string session state must lower as a truthy conditional"
        );
        assert!(app.contains("HistoryPanel") || app.contains("Chat History"));
        assert!(app.contains("items={messages}"));
        assert!(app.contains("/complete") || app.contains("on_send"));
        assert!(app.contains("AppBar"));
        assert!(app.contains("SidePanel"));
        assert!(output.join("python/requirements.txt").is_file());
        let py = fs::read_to_string(output.join("python/worker.py")).unwrap();
        assert!(
            py.contains("SILC_LLM_N_CTX")
                && py.contains("DEFAULT_N_CTX = 8192")
                && py.contains("n_ctx=N_CTX")
                && !py.contains("n_ctx=2048"),
            "silclm worker must read SILC_LLM_N_CTX with an 8K default"
        );
        let ts = fs::read_to_string(output.join("typescript/worker.ts")).unwrap();
        assert!(ts.contains("/complete"));
        assert!(ts.contains("/history"));
        assert!(ts.contains("llm.complete") || ts.contains("true"));
        assert!(
            ts.contains("text: prompt") || ts.contains("text:"),
            "complete ingest must send ControlFrame::Ingest.text"
        );
        assert!(ts.contains("INGEST_TIMEOUT_MS") || ts.contains("180_000"));
        assert!(
            ts.contains("json_extract(payload, '$.session_id') = ?") && ts.contains("LIMIT 50"),
            "history must filter by session in SQLite before LIMIT"
        );
        let history_panel =
            fs::read_to_string(output.join("typescript/src/components/ui/history-panel.tsx"))
                .unwrap();
        assert!(history_panel.contains("HistoryPanel") || history_panel.contains("collapsed"));
        let composer =
            fs::read_to_string(output.join("typescript/src/components/ui/chat-composer.tsx"))
                .unwrap();
        assert!(composer.contains("focusKey") && composer.contains(".focus()"));
        assert!(
            composer.contains("isComposing") && !composer.contains("requestSubmit"),
            "Enter must call onSubmit directly with IME/empty/submitting guards"
        );
        let thread =
            fs::read_to_string(output.join("typescript/src/components/ui/chat-thread.tsx"))
                .unwrap();
        assert!(
            thread.contains("Loading history")
                && thread.contains("role=\"alert\"")
                && thread.contains("message.pending")
        );
        let manifest = fs::read_to_string(&result.manifest).unwrap();
        assert!(manifest.contains("llm.complete") || manifest.contains("\"llm\": true"));
        assert!(manifest.contains("silclm"));
        fs::remove_dir_all(output).ok();
    }

    #[test]
    fn emits_resource_app_with_actions() {
        let (_program, result, output) = parse_emit(RESOURCE_SOURCE, "shop");
        let graph = result.graph.as_ref().unwrap();
        assert_eq!(graph.processor_op, ProcessorOp::None);
        assert!(graph.capabilities.resources);
        assert!(!graph.actions.is_empty());

        let ts = fs::read_to_string(output.join("typescript/worker.ts")).unwrap();
        assert!(ts.contains("products") || ts.contains("/api/"));
        assert!(ts.contains("\"none\"") || ts.contains("PROCESSOR"));
        let app = fs::read_to_string(output.join("typescript/src/App.tsx")).unwrap();
        assert!(app.contains("ShopPage"));
        assert!(app.contains("/api/products") || app.contains("products"));
        let py = fs::read_to_string(output.join("python/worker.py")).unwrap();
        assert!(py.contains("none") || py.contains("PROCESSOR"));
        let manifest = fs::read_to_string(&result.manifest).unwrap();
        assert!(manifest.contains("\"actions\""));
        assert!(manifest.contains("Products.list") || manifest.contains("/api/products"));
        assert!(manifest.contains("\"resources\": true") || manifest.contains("products"));
        assert!(!output.join("python/requirements.txt").is_file());
        fs::remove_dir_all(output).ok();
    }

    #[test]
    fn emits_runnable_service_http_gin_worker() {
        let (_program, result, output) = parse_emit(API_SOURCE, "feedback-api");
        assert_eq!(result.execution_mode, ExecutionMode::Runnable);
        assert!(result.graph.as_ref().unwrap().is_api_only());

        let go = fs::read_to_string(output.join("go/api/worker.go")).unwrap();
        let gomod = fs::read_to_string(output.join("go/api/go.mod")).unwrap();
        assert!(!go.contains("__PORT__"));
        assert!(!go.contains("__STRUCTS__"));
        assert!(!go.contains("__ROUTES__"));
        assert!(go.contains("type FeedbackRecord struct"));
        assert!(go.contains("r.GET(\"/api/feedback\""));
        assert!(go.contains("r.POST(\"/api/feedback\""));
        assert!(go.contains("github.com/gin-gonic/gin"));
        assert!(go.contains("/health"));
        assert!(gomod.contains("github.com/gin-gonic/gin"));
        assert!(!output.join("typescript/worker.ts").is_file());
        assert!(!output.join("python/worker.py").is_file());

        let manifest = fs::read_to_string(&result.manifest).unwrap();
        assert!(manifest.contains("\"adapter\": \"gin-v1\""));
        assert!(manifest.contains("\"api_only\": true"));
        assert!(manifest.contains("service::http") || manifest.contains("\"manifest_version\": 3"));
        assert!(manifest.contains("go/api/worker.go"));
        assert!(!manifest.contains("portal_kind"));
        fs::remove_dir_all(output).ok();
    }

    #[test]
    fn rejects_legacy_is_view() {
        let source = r#"
@version("0.2.0")
class BadView is view {
    method render() { ui::page() }
}
"#;
        let err = sil_parser::parse(source).unwrap_err();
        assert!(err.message.contains("is view") || err.message.contains("view"));
    }
}
