//! Silc 0.4.0 code generation: inspectable stubs and runnable dual-surface apps.
//!
//! Pipeline vocabulary: **codegen** renders target source from the validated
//! semantic model; **emit** writes those artifacts into `.runtime/`. Dual-surface
//! UI goes through [`ui_lower`] — the sole named lower pass (AST → React /
//! OpenTUI adapters). Game programs go through [`game_lower`] (AST → JSON
//! manifest) plus template copy. Worker and contract paths use template render
//! + emit, not lowering.

mod game_lower;
mod ui_lower;

use sil_core::{
    infer_graph, ApiRoute, Contract, ExecutableGraph, ExecutionMode, Module, ProcessorOp, Program,
    ResourceKind, SubsetPredicate, Target, TypeExpr,
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
const PIPELINE_WORKER_TS: &str = include_str!("../templates/pipeline_worker.ts");
const PROCESSOR_WORKER_PY: &str = include_str!("../templates/processor_worker.py");
const STORE_WORKER_GO: &str = include_str!("../templates/store_worker.go");
const GAME_STORE_WORKER_GO: &str = include_str!("../templates/game_store_worker.go");
const GAME_BAKE_WORKER_PY: &str = include_str!("../templates/game_bake_worker.py");
const GAME_COGNITION_WORKER_PY: &str = include_str!("../templates/game_cognition_worker.py");
const LLM_REQUIREMENTS: &str = include_str!("../templates/llm_requirements.txt");
/// Compiler-owned Python dependencies for a generated tensor runtime.
pub const TENSOR_REQUIREMENTS: &str = include_str!("../templates/tensor_requirements.txt");
const STORE_GOMOD: &str = include_str!("../templates/go.mod");
const SERVICE_HTTP_GO: &str = include_str!("../templates/service_http_worker.go");
const SERVICE_HTTP_GOMOD: &str = include_str!("../templates/service_http_go.mod");
const SCRAPE_CRAWL_GO: &str = include_str!("../templates/scrape_crawl_worker.go");
const SCRAPE_CRAWL_GOMOD: &str = include_str!("../templates/scrape_crawl_go.mod");
const SCRAPE_BROWSER_PY: &str = include_str!("../templates/scrape_browser_worker.py");
const SCRAPE_REQUIREMENTS: &str = include_str!("../templates/scrape_requirements.txt");
const DOC_EXTRACT_PY: &str = include_str!("../templates/doc_extract_worker.py");
const DOC_REQUIREMENTS: &str = include_str!("../templates/doc_requirements.txt");

pub const SCRAPE_BUN_ADAPTER: &str = "bun-fetch-v1";
pub const SCRAPE_COLLY_ADAPTER: &str = "go-colly-v1";
pub const SCRAPE_PLAYWRIGHT_ADAPTER: &str = "python-playwright-v1";
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
const UI_WEB_DATA_TABLE_TSX: &str = include_str!("../templates/ui_web_data_table.tsx");
const UI_WEB_SELECT_TSX: &str = include_str!("../templates/ui_web_select.tsx");
const UI_WEB_CHECKBOX_TSX: &str = include_str!("../templates/ui_web_checkbox.tsx");
const UI_WEB_SWITCH_TSX: &str = include_str!("../templates/ui_web_switch.tsx");
const UI_WEB_FIELD_TSX: &str = include_str!("../templates/ui_web_field.tsx");
const UI_WEB_BADGE_TSX: &str = include_str!("../templates/ui_web_badge.tsx");
const UI_WEB_ALERT_TSX: &str = include_str!("../templates/ui_web_alert.tsx");
const UI_WEB_DIVIDER_TSX: &str = include_str!("../templates/ui_web_divider.tsx");
const UI_WEB_SECTION_TSX: &str = include_str!("../templates/ui_web_section.tsx");
const UI_WEB_FOOTER_TSX: &str = include_str!("../templates/ui_web_footer.tsx");
const UI_WEB_DESCRIPTION_LIST_TSX: &str = include_str!("../templates/ui_web_description_list.tsx");
const UI_WEB_TABS_TSX: &str = include_str!("../templates/ui_web_tabs.tsx");
const UI_WEB_DIALOG_TSX: &str = include_str!("../templates/ui_web_dialog.tsx");
const UI_WEB_LOADING_TSX: &str = include_str!("../templates/ui_web_loading.tsx");
const UI_WEB_EMPTY_TSX: &str = include_str!("../templates/ui_web_empty.tsx");
const UI_TERMINAL_RUNTIME_TS: &str = include_str!("../templates/ui_terminal_runtime.ts");
const UI_TERMINAL_COMPONENTS_TS: &str = include_str!("../templates/ui_terminal_components.ts");
const UI_TERMINAL_MAIN_TS: &str = include_str!("../templates/ui_terminal_main.ts");

/// Compiler-pinned React version for ui::web (must match ui_web_package.json).
pub const UI_WEB_REACT_VERSION: &str = "18.3.1";
pub const UI_WEB_TAILWIND_VERSION: &str = "3.4.17";
pub const UI_WEB_SUBSTRATE: &str = "react";
pub const UI_TERMINAL_SUBSTRATE: &str = "opentui";
pub const UI_TERMINAL_OPENTUI_VERSION: &str = "0.4.5";
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
        emit_runnable(
            runtime_root,
            program,
            g,
            schema_id,
            compiler_version,
            &mut generated,
        )?;
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
                "scrape": g.capabilities.scrape,
                "doc": g.capabilities.doc,
            },
            "scrape": scrape_manifest(g),
            "doc": doc_manifest(g),
            "app": g.app_name,
            "root_component": g.root_component,
            "model_ref": g.model_ref,
            "embedding_dim": g.embedding_dim,
            "tensor_device": g.tensor_device,
            "tensor_input_field": g.tensor_input_field,
            "tensor_output_field": g.tensor_output_field,
            "actions": actions_json(g),
            "resource_tables": g.resource_tables.iter().map(|(name, table)| {
                serde_json::json!({ "resource": name, "table": table })
            }).collect::<Vec<_>>(),
            "resource_seeds": resource_seeds_json(program),
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
                "terminal_substrate": if g.terminal_port.is_some() {
                    UI_TERMINAL_SUBSTRATE
                } else {
                    "disabled"
                },
                "terminal_fallback": "bun-tcp-telnet",
                "opentui_version": UI_TERMINAL_OPENTUI_VERSION,
                "react_version": UI_WEB_REACT_VERSION,
                "tailwind_version": UI_WEB_TAILWIND_VERSION,
                "app": g.app_name,
                "root_component": g.root_component,
                "assets": [
                    "typescript/src/main.tsx",
                    "typescript/src/App.tsx",
                    "typescript/terminal.ts",
                    "typescript/terminal_main.ts",
                    "typescript/src/TerminalApp.tsx",
                    "typescript/src/components/terminal/runtime.ts",
                    "typescript/src/components/terminal/components.ts",
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
                    "typescript/src/components/ui/data-table.tsx",
                    "typescript/src/components/ui/select.tsx",
                    "typescript/src/components/ui/checkbox.tsx",
                    "typescript/src/components/ui/switch.tsx",
                    "typescript/src/components/ui/field.tsx",
                    "typescript/src/components/ui/badge.tsx",
                    "typescript/src/components/ui/alert.tsx",
                    "typescript/src/components/ui/divider.tsx",
                    "typescript/src/components/ui/section.tsx",
                    "typescript/src/components/ui/footer.tsx",
                    "typescript/src/components/ui/description-list.tsx",
                    "typescript/src/components/ui/tabs.tsx",
                    "typescript/src/components/ui/dialog.tsx",
                    "typescript/src/components/ui/loading.tsx",
                    "typescript/src/components/ui/empty.tsx",
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
                    "@opentui/core": UI_TERMINAL_OPENTUI_VERSION,
                },
                "provenance": "compiler-owned ui::web (React/Tailwind) + ui::terminal (OpenTUI) via Bun",
                "catalog": sil_core::catalog_component_names(),
            });
        }
        if g.has_game() {
            manifest["game"] = serde_json::json!({
                "profile": "webgpu",
                "surfaces": ["web"],
                "substrate": "babylon-webgpu",
                "babylon_version": "9.16.2",
                "vite_version": "8.1.5",
                "title": g.game.title,
                "target_fps": g.game.target_fps,
                "app": g.app_name,
                "engines": ["bun", "cpython", "go"],
                "bake": "python/game_bake_worker.py",
                "artifacts": [
                    "typescript/package.json",
                    "typescript/bun.lock",
                    "typescript/public/manifest.json",
                    "typescript/public/baked/bake.json",
                    "typescript/src/main.ts",
                    "typescript/dist/index.html",
                    "python/game_bake_worker.py",
                    "python/bake_plan.json",
                    "go/worker.go",
                ],
                "provenance": "compiler-owned game::scene → Bun WebGPU + CPython bake + Go SQLite (ADR-012)",
                "catalog": sil_core::catalog_game_node_names(),
            });
            if let Some(graph_obj) = manifest.get_mut("graph") {
                if let Some(obj) = graph_obj.as_object_mut() {
                    obj.insert("game".into(), serde_json::json!(true));
                    obj.insert(
                        "capabilities".into(),
                        serde_json::json!({
                            "web": false,
                            "terminal": false,
                            "webgpu": true,
                            "score": false,
                            "llm": false,
                            "history": false,
                            "resources": true,
                            "scrape": false,
                            "doc": false,
                        }),
                    );
                }
            }
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
        if g.has_game() {
            entrypoints.insert("bun".into(), serde_json::json!("typescript/worker.ts"));
            entrypoints.insert(
                "game_web_entry".into(),
                serde_json::json!("typescript/src/main.ts"),
            );
            entrypoints.insert(
                "game_manifest".into(),
                serde_json::json!("typescript/public/manifest.json"),
            );
            entrypoints.insert(
                "python_bake".into(),
                serde_json::json!("python/game_bake_worker.py"),
            );
            entrypoints.insert("go_source".into(), serde_json::json!("go/worker.go"));
            entrypoints.insert("go_binary".into(), serde_json::json!("go/worker"));
            entrypoints.insert(
                "supervisor_socket".into(),
                serde_json::json!(SUPERVISOR_SOCKET),
            );
        }
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
                serde_json::json!("typescript/terminal_main.ts"),
            );
            entrypoints.insert(
                "terminal_cli".into(),
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
        if g.has_scrape() {
            if g.needs_scrape_crawl() {
                entrypoints.insert(
                    "go_crawl_source".into(),
                    serde_json::json!("go/crawl/worker.go"),
                );
                entrypoints.insert(
                    "go_crawl_binary".into(),
                    serde_json::json!("go/crawl/worker"),
                );
            }
            if g.needs_scrape_browser() {
                entrypoints.insert(
                    "python_browser".into(),
                    serde_json::json!("python/browser_worker.py"),
                );
            }
        }
        if g.has_doc() {
            entrypoints.insert(
                "python_doc_extract".into(),
                serde_json::json!("python/doc_extract_worker.py"),
            );
        }
        if g.is_pipeline_only() {
            entrypoints.insert(
                "pipeline_ingress".into(),
                serde_json::json!("typescript/worker.ts"),
            );
            entrypoints.insert("python".into(), serde_json::json!("python/worker.py"));
            entrypoints.insert("go_source".into(), serde_json::json!("go/worker.go"));
            entrypoints.insert("go_binary".into(), serde_json::json!("go/worker"));
            manifest["pipeline"] = serde_json::json!({
                "input": {"type": "object", "required": ["url"]},
                "output": {"table": g.sqlite_table, "embedding_field": g.tensor_output_field},
                "model": g.model_ref,
                "dimension": g.embedding_dim,
                "device": g.tensor_device,
                "slot_capacity": 65_536,
            });
        }
        manifest["entrypoints"] = serde_json::Value::Object(entrypoints);
        if g.has_scrape() {
            manifest["scrape"] = scrape_manifest(g);
        }
        if g.has_doc() {
            manifest["doc"] = doc_manifest(g);
        }
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
            "scrape": g.capabilities.scrape,
            "doc": g.capabilities.doc,
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
    compiler_version: &str,
    generated: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if graph.has_game() {
        emit_game(root, program, graph, schema_id, compiler_version, generated)?;
        return Ok(());
    }
    if graph.has_ui() {
        emit_ui_app(root, program, graph, schema_id, compiler_version, generated)?;
    }
    if graph.has_api() {
        emit_service_http(root, program, graph, schema_id, generated)?;
    }
    if graph.is_pipeline_only() {
        emit_pipeline(root, program, graph, schema_id, compiler_version, generated)?;
    }
    Ok(())
}

fn emit_game(
    root: &Path,
    program: &Program,
    graph: &ExecutableGraph,
    schema_id: u32,
    compiler_version: &str,
    generated: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let game = graph
        .game_decl
        .as_ref()
        .ok_or_else(|| "game graph missing game_decl".to_string())?;
    let game_manifest = game_lower::lower_game(game)?;
    let bake_plan = game_lower::bake_plan_from_manifest(&game_manifest);

    clear_dir_sources(&root.join("go"), &["go"])?;
    clear_dir_sources(&root.join("python"), &["py"])?;
    let ts_dir = root.join("typescript");
    if ts_dir.is_dir() {
        fs::remove_dir_all(&ts_dir)
            .map_err(|error| format!("clear {}: {error}", ts_dir.display()))?;
    }
    fs::create_dir_all(&ts_dir).map_err(|error| format!("create {}: {error}", ts_dir.display()))?;
    fs::create_dir_all(root.join("go"))
        .map_err(|error| format!("create go: {error}"))?;
    fs::create_dir_all(root.join("python"))
        .map_err(|error| format!("create python: {error}"))?;

    let template_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates/game");
    copy_game_templates(&template_root, &ts_dir, generated)?;

    let lock_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates/game_bun.lock");
    if lock_src.is_file() {
        let lock_dst = ts_dir.join("bun.lock");
        fs::copy(&lock_src, &lock_dst)
            .map_err(|error| format!("copy bun.lock: {error}"))?;
        generated.push(lock_dst);
    }

    let manifest_path = ts_dir.join("public/manifest.json");
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut manifest_body = serde_json::to_string_pretty(&game_manifest)
        .map_err(|error| format!("serialize game manifest: {error}"))?;
    manifest_body.push('\n');
    fs::write(&manifest_path, &manifest_body)
        .map_err(|error| format!("write {}: {error}", manifest_path.display()))?;
    generated.push(manifest_path);

    let bake_plan_path = root.join("python/bake_plan.json");
    let mut bake_body = serde_json::to_string_pretty(&bake_plan)
        .map_err(|error| format!("serialize bake plan: {error}"))?;
    bake_body.push('\n');
    fs::write(&bake_plan_path, &bake_body)
        .map_err(|error| format!("write {}: {error}", bake_plan_path.display()))?;
    generated.push(bake_plan_path);

    let bake_py = root.join("python/game_bake_worker.py");
    fs::write(
        &bake_py,
        GAME_BAKE_WORKER_PY.replace("__COMPILER_VERSION__", compiler_version),
    )
    .map_err(|error| format!("write {}: {error}", bake_py.display()))?;
    generated.push(bake_py);

    let cognition_py = root.join("python/game_cognition_worker.py");
    fs::write(
        &cognition_py,
        GAME_COGNITION_WORKER_PY.replace("__COMPILER_VERSION__", compiler_version),
    )
    .map_err(|error| format!("write {}: {error}", cognition_py.display()))?;
    generated.push(cognition_py);

    let go_files = [
        (
            root.join("go/worker.go"),
            render_template(
                GAME_STORE_WORKER_GO,
                program,
                graph,
                schema_id,
                compiler_version,
            ),
        ),
        (root.join("go/go.mod"), STORE_GOMOD.to_string()),
    ];
    for (path, contents) in go_files {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        fs::write(&path, contents).map_err(|error| format!("write {}: {error}", path.display()))?;
        generated.push(path);
    }

    let index_path = ts_dir.join("index.html");
    if index_path.is_file() {
        let title = game_manifest
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Silc Game");
        let html = fs::read_to_string(&index_path)
            .map_err(|error| format!("read {}: {error}", index_path.display()))?;
        let html = html.replace("<title>Silc Game</title>", &format!("<title>{title}</title>"));
        let html = html.replace(
            "<!-- AUTO-GENERATED BY silc — DO NOT EDIT -->",
            &format!("<!-- AUTO-GENERATED BY silc {compiler_version} — DO NOT EDIT -->"),
        );
        fs::write(&index_path, html)
            .map_err(|error| format!("write {}: {error}", index_path.display()))?;
    }

    Ok(())
}

fn copy_game_templates(
    from: &Path,
    to: &Path,
    generated: &mut Vec<PathBuf>,
) -> Result<(), String> {
    fn walk(
        from: &Path,
        to: &Path,
        generated: &mut Vec<PathBuf>,
    ) -> Result<(), String> {
        let entries = fs::read_dir(from)
            .map_err(|error| format!("read {}: {error}", from.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("read dir entry: {error}"))?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == "node_modules"
                || name_str == "dist"
                || name_str == ".gitignore"
                || name_str == "DECISIONS.md"
                || name_str == "ASSETS.md"
            {
                continue;
            }
            let src = entry.path();
            let dst = to.join(&name);
            let ft = entry
                .file_type()
                .map_err(|error| format!("stat {}: {error}", src.display()))?;
            if ft.is_dir() {
                fs::create_dir_all(&dst)
                    .map_err(|error| format!("create {}: {error}", dst.display()))?;
                walk(&src, &dst, generated)?;
            } else if ft.is_file() {
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| format!("create {}: {error}", parent.display()))?;
                }
                fs::copy(&src, &dst)
                    .map_err(|error| format!("copy {} → {}: {error}", src.display(), dst.display()))?;
                generated.push(dst);
            }
        }
        Ok(())
    }
    walk(from, to, generated)
}

fn emit_pipeline(
    root: &Path,
    program: &Program,
    graph: &ExecutableGraph,
    schema_id: u32,
    compiler_version: &str,
    generated: &mut Vec<PathBuf>,
) -> Result<(), String> {
    clear_dir_sources(&root.join("go"), &["go"])?;
    clear_dir_sources(&root.join("typescript"), &["ts", "tsx", "js"])?;
    clear_dir_sources(&root.join("python"), &["py"])?;
    let files = [
        (
            root.join("typescript/worker.ts"),
            render_template(
                PIPELINE_WORKER_TS,
                program,
                graph,
                schema_id,
                compiler_version,
            ),
        ),
        (
            root.join("python/worker.py"),
            render_template(
                PROCESSOR_WORKER_PY,
                program,
                graph,
                schema_id,
                compiler_version,
            ),
        ),
        (
            root.join("python/tensor_requirements.txt"),
            TENSOR_REQUIREMENTS.to_string(),
        ),
        (
            root.join("go/worker.go"),
            render_template(STORE_WORKER_GO, program, graph, schema_id, compiler_version),
        ),
        (root.join("go/go.mod"), STORE_GOMOD.to_string()),
    ];
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

fn emit_ui_app(
    root: &Path,
    program: &Program,
    graph: &ExecutableGraph,
    schema_id: u32,
    compiler_version: &str,
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
    fs::create_dir_all(ts_src.join("components/terminal"))
        .map_err(|error| format!("create {}: {error}", ts_src.display()))?;
    fs::create_dir_all(ts_src.join("lib"))
        .map_err(|error| format!("create {}: {error}", ts_src.display()))?;
    fs::create_dir_all(&ts_dist)
        .map_err(|error| format!("create {}: {error}", ts_dist.display()))?;
    clear_dir_sources(&ts_dist, &["js", "css", "html", "map"])?;

    let app_tsx = ui_lower::render_web_app(program, graph, compiler_version);
    let terminal_ts = ui_lower::render_terminal_module(program, graph, compiler_version);
    let terminal_app = ui_lower::render_terminal_app(program, graph, compiler_version);

    let mut files = vec![
        (
            root.join("typescript/worker.ts"),
            render_template(APP_WORKER_TS, program, graph, schema_id, compiler_version),
        ),
        (root.join("typescript/terminal.ts"), terminal_ts),
        (
            root.join("typescript/terminal_main.ts"),
            UI_TERMINAL_MAIN_TS.to_string(),
        ),
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
        (root.join("typescript/src/TerminalApp.tsx"), terminal_app),
        (
            root.join("typescript/src/theme.css"),
            UI_WEB_THEME_CSS.to_string(),
        ),
        (
            root.join("typescript/src/lib/utils.ts"),
            UI_WEB_UTILS_TS.to_string(),
        ),
        (
            root.join("typescript/src/components/terminal/runtime.ts"),
            UI_TERMINAL_RUNTIME_TS.to_string(),
        ),
        (
            root.join("typescript/src/components/terminal/components.ts"),
            UI_TERMINAL_COMPONENTS_TS.to_string(),
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
            root.join("typescript/src/components/ui/data-table.tsx"),
            UI_WEB_DATA_TABLE_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/select.tsx"),
            UI_WEB_SELECT_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/checkbox.tsx"),
            UI_WEB_CHECKBOX_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/switch.tsx"),
            UI_WEB_SWITCH_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/field.tsx"),
            UI_WEB_FIELD_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/badge.tsx"),
            UI_WEB_BADGE_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/alert.tsx"),
            UI_WEB_ALERT_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/divider.tsx"),
            UI_WEB_DIVIDER_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/section.tsx"),
            UI_WEB_SECTION_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/footer.tsx"),
            UI_WEB_FOOTER_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/description-list.tsx"),
            UI_WEB_DESCRIPTION_LIST_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/tabs.tsx"),
            UI_WEB_TABS_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/dialog.tsx"),
            UI_WEB_DIALOG_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/loading.tsx"),
            UI_WEB_LOADING_TSX.to_string(),
        ),
        (
            root.join("typescript/src/components/ui/empty.tsx"),
            UI_WEB_EMPTY_TSX.to_string(),
        ),
        (
            root.join("python/worker.py"),
            render_template(
                PROCESSOR_WORKER_PY,
                program,
                graph,
                schema_id,
                compiler_version,
            ),
        ),
        (
            root.join("go/worker.go"),
            render_template(STORE_WORKER_GO, program, graph, schema_id, compiler_version),
        ),
        (root.join("go/go.mod"), STORE_GOMOD.to_string()),
    ];
    if graph.needs_llm() {
        files.push((
            root.join("python/requirements.txt"),
            LLM_REQUIREMENTS.to_string(),
        ));
    }
    if graph.has_scrape() {
        if graph.needs_scrape_crawl() {
            files.push((
                root.join("go/crawl/worker.go"),
                SCRAPE_CRAWL_GO.replace("__COMPILER_VERSION__", compiler_version),
            ));
            files.push((root.join("go/crawl/go.mod"), SCRAPE_CRAWL_GOMOD.to_string()));
        }
        if graph.needs_scrape_browser() {
            files.push((
                root.join("python/browser_worker.py"),
                SCRAPE_BROWSER_PY.replace("__COMPILER_VERSION__", compiler_version),
            ));
            files.push((
                root.join("python/scrape_requirements.txt"),
                SCRAPE_REQUIREMENTS.to_string(),
            ));
        }
    }
    if graph.has_doc() {
        files.push((
            root.join("python/doc_extract_worker.py"),
            DOC_EXTRACT_PY.replace("__COMPILER_VERSION__", compiler_version),
        ));
        files.push((
            root.join("python/doc_requirements.txt"),
            DOC_REQUIREMENTS.to_string(),
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
    let mut needs_strings = false;
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
        if contract_has_subset_fields(program, contract) {
            needs_strings = true;
        }
    }

    let mut routes = String::new();
    for route in &graph.api_routes {
        routes.push_str(&render_go_route(program, route));
    }

    let imports = if needs_strings {
        "\t\"net/http\"\n\t\"os\"\n\t\"strings\"\n\t\"sync\"\n\n\t\"github.com/gin-gonic/gin\"\n"
    } else {
        "\t\"net/http\"\n\t\"os\"\n\t\"sync\"\n\n\t\"github.com/gin-gonic/gin\"\n"
    };

    let port = graph.api_port().unwrap_or(8080);
    let body = SERVICE_HTTP_GO
        .replace("__IMPORTS__", imports)
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

fn contract_has_subset_fields(program: &Program, contract: &Contract) -> bool {
    contract
        .fields
        .iter()
        .any(|f| subset_rule_for_field(program, &f.ty).is_some())
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

fn render_go_route(program: &Program, route: &ApiRoute) -> String {
    let store = format!("store{}", route.contract);
    let path = route.path.replace('"', "");
    match route.method.as_str() {
        "GET" => format!(
            "\tr.GET(\"{path}\", func(c *gin.Context) {{\n\t\t{store}.mu.Lock()\n\t\tdefer {store}.mu.Unlock()\n\t\tc.JSON(http.StatusOK, {store}.items)\n\t}})\n"
        ),
        "POST" => {
            let checks = render_go_subset_checks(program, &route.contract, "item");
            format!(
                "\tr.POST(\"{path}\", func(c *gin.Context) {{\n\t\tvar item {}\n\t\tif err := c.ShouldBindJSON(&item); err != nil {{\n\t\t\tc.JSON(http.StatusBadRequest, gin.H{{\"error\": err.Error()}})\n\t\t\treturn\n\t\t}}\n{checks}\t\t{store}.mu.Lock()\n\t\t{store}.items = append({store}.items, item)\n\t\t{store}.mu.Unlock()\n\t\tc.JSON(http.StatusCreated, item)\n\t}})\n",
                route.contract
            )
        }
        other => format!("\t// unsupported method {other} for {path}\n"),
    }
}

fn render_go_subset_checks(program: &Program, contract_name: &str, var: &str) -> String {
    let Some(contract) = program.contracts.iter().find(|c| c.name == contract_name) else {
        return String::new();
    };
    let mut out = String::new();
    for field in &contract.fields {
        let Some(subset) = program
            .subsets
            .iter()
            .find(|s| matches!(&field.ty, TypeExpr::Named(n) if n == &s.name))
        else {
            continue;
        };
        let Some(pred) = &subset.predicate else {
            continue;
        };
        let field_access = format!("{var}.{}", pascal_case(&field.name));
        let cond = pred.to_go_check(&field_access);
        out.push_str(&format!(
            "\t\tif !({cond}) {{\n\t\t\tc.JSON(http.StatusBadRequest, gin.H{{\"error\": \"field {} failed subset {}\"}})\n\t\t\treturn\n\t\t}}\n",
            field.name, subset.name
        ));
    }
    out
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

fn scrape_manifest(graph: &ExecutableGraph) -> serde_json::Value {
    let mut ops = Vec::new();
    if graph.scrape.page {
        ops.push("page");
    }
    if graph.scrape.site {
        ops.push("site");
    }
    if graph.scrape.select {
        ops.push("select");
    }
    if graph.scrape.render {
        ops.push("render");
    }
    if graph.scrape.extract {
        ops.push("extract");
    }
    serde_json::json!({
        "ops": ops,
        "js": graph.scrape.js.as_str(),
        "depth": graph.scrape.depth,
        "same_host": graph.scrape.same_host,
        "link_css": graph.scrape.link_css,
        "extract_into": graph.scrape.extract_into,
        "selects": graph.scrape.selects.iter().map(|s| {
            serde_json::json!({ "css": s.css, "as_field": s.as_field })
        }).collect::<Vec<_>>(),
        "adapters": {
            "static": SCRAPE_BUN_ADAPTER,
            "crawl": if graph.needs_scrape_crawl() {
                serde_json::Value::String(SCRAPE_COLLY_ADAPTER.into())
            } else {
                serde_json::Value::Null
            },
            "browser": if graph.needs_scrape_browser() {
                serde_json::Value::String(SCRAPE_PLAYWRIGHT_ADAPTER.into())
            } else {
                serde_json::Value::Null
            },
        },
        "provenance": "ADR-006 scrape::* fused substrates",
    })
}

fn scrape_table(graph: &ExecutableGraph) -> String {
    graph
        .resource_tables
        .iter()
        .map(|(_, table)| table.clone())
        .find(|t| t.contains("scrape") || t == "scraped_pages")
        .or_else(|| {
            graph
                .resource_tables
                .first()
                .map(|(_, table)| table.clone())
        })
        .unwrap_or_else(|| "scraped_pages".into())
}

fn doc_table(graph: &ExecutableGraph) -> String {
    graph
        .doc
        .table
        .clone()
        .or_else(|| {
            graph
                .resource_tables
                .iter()
                .map(|(_, table)| table.clone())
                .find(|t| t.contains("document") || t == "documents")
        })
        .or_else(|| {
            graph
                .resource_tables
                .first()
                .map(|(_, table)| table.clone())
        })
        .unwrap_or_else(|| "documents".into())
}

fn doc_manifest(graph: &ExecutableGraph) -> serde_json::Value {
    serde_json::json!({
        "extract": graph.doc.extract,
        "extract_into": graph.doc.extract_into,
        "table": doc_table(graph),
        "formats": ["pdf", "docx", "odt", "md", "txt", "html"],
        "adapters": {
            "upload": "bun-multipart-v1",
            "extract": "python-doc-extract-v1",
        },
        "provenance": "ADR-011 doc::* Python-native extract",
        "discard_originals": true,
    })
}

fn render_template(
    template: &str,
    program: &Program,
    graph: &ExecutableGraph,
    schema_id: u32,
    compiler_version: &str,
) -> String {
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
    let has_scrape = if graph.has_scrape() { "true" } else { "false" };
    let has_doc = if graph.has_doc() { "true" } else { "false" };
    let scrape_site = if graph.scrape.site { "true" } else { "false" };
    let scrape_same_host = if graph.scrape.same_host {
        "true"
    } else {
        "false"
    };
    let scrape_selects = serde_json::to_string(
        &graph
            .scrape
            .selects
            .iter()
            .map(|s| {
                serde_json::json!({
                    "css": s.css,
                    "as_field": s.as_field,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());
    let link_css = graph
        .scrape
        .link_css
        .clone()
        .unwrap_or_else(|| "a[href]".into());
    let subset_rules =
        serde_json::to_string_pretty(&subset_rules_json(program)).unwrap_or_else(|_| "{}".into());
    let resource_seeds =
        serde_json::to_string_pretty(&resource_seeds_json(program)).unwrap_or_else(|_| "[]".into());

    template
        .replace("__COMPILER_VERSION__", compiler_version)
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
        .replace("__HAS_SCRAPE__", has_scrape)
        .replace("__HAS_DOC__", has_doc)
        .replace("__SCRAPE_SITE__", scrape_site)
        .replace("__SCRAPE_JS__", graph.scrape.js.as_str())
        .replace("__SCRAPE_DEPTH__", &graph.scrape.depth.to_string())
        .replace("__SCRAPE_SAME_HOST__", scrape_same_host)
        .replace("__SCRAPE_LINK_CSS__", &link_css)
        .replace("__SCRAPE_SELECTS_JSON__", &scrape_selects)
        .replace("__SCRAPE_TABLE__", &scrape_table(graph))
        .replace("__DOC_TABLE__", &doc_table(graph))
        .replace("__ACTIONS_JSON__", &actions)
        .replace("__RESOURCE_TABLES__", &resource_tables)
        .replace("__RESOURCE_SEEDS_JSON__", &resource_seeds)
        .replace("__ROUTES_JSON__", &routes)
        .replace("__SUBSET_RULES_JSON__", &subset_rules)
}

/// Table → field → { kind, lit } for resource/API ingress subset checks.
fn subset_rules_json(program: &Program) -> serde_json::Value {
    let mut by_table = serde_json::Map::new();
    for resource in &program.resources {
        let Some(contract_name) = &resource.contract else {
            continue;
        };
        let Some(contract) = program.contracts.iter().find(|c| c.name == *contract_name) else {
            continue;
        };
        let mut fields = serde_json::Map::new();
        for field in &contract.fields {
            if let Some(rule) = subset_rule_for_field(program, &field.ty) {
                fields.insert(field.name.clone(), rule);
            }
        }
        if !fields.is_empty() {
            by_table.insert(resource.table_name(), serde_json::Value::Object(fields));
        }
    }
    serde_json::Value::Object(by_table)
}

/// Idempotent seed rows keyed by table for compiler-owned INSERT OR IGNORE.
fn resource_seeds_json(program: &Program) -> serde_json::Value {
    let mut out = Vec::new();
    for resource in &program.resources {
        let table = resource.table_name();
        for seed in &resource.seeds {
            let mut data = serde_json::Map::new();
            let mut id = None;
            let mut created_at = None;
            for (name, value) in &seed.fields {
                let json = expr_to_json_value(value);
                if name == "id" {
                    id = json.as_str().map(|s| s.to_string());
                }
                if name == "published_at" {
                    created_at = json.as_str().map(|s| s.to_string());
                }
                data.insert(name.clone(), json);
            }
            let Some(id) = id else {
                continue;
            };
            out.push(serde_json::json!({
                "table": table,
                "id": id,
                "created_at": created_at,
                "data": serde_json::Value::Object(data),
            }));
        }
    }
    serde_json::Value::Array(out)
}

fn expr_to_json_value(expr: &sil_core::Expr) -> serde_json::Value {
    match expr {
        sil_core::Expr::String(s) => serde_json::Value::String(s.clone()),
        sil_core::Expr::Number(n) => {
            if let Ok(i) = n.parse::<i64>() {
                serde_json::json!(i)
            } else if let Ok(f) = n.parse::<f64>() {
                serde_json::json!(f)
            } else {
                serde_json::Value::String(n.clone())
            }
        }
        sil_core::Expr::Bool(b) => serde_json::Value::Bool(*b),
        sil_core::Expr::List(items) => {
            serde_json::Value::Array(items.iter().map(expr_to_json_value).collect())
        }
        other => serde_json::Value::String(format!("{other:?}")),
    }
}

fn subset_rule_for_field(program: &Program, ty: &TypeExpr) -> Option<serde_json::Value> {
    let name = match ty {
        TypeExpr::Named(n) => n.as_str(),
        TypeExpr::Optional(inner) => return subset_rule_for_field(program, inner),
        _ => return None,
    };
    let subset = program.subsets.iter().find(|s| s.name == name)?;
    let pred = subset.predicate.as_ref()?;
    let (kind, lit) = match pred {
        SubsetPredicate::Contains(lit) => ("contains", lit.as_str()),
        SubsetPredicate::StartsWith(lit) => ("starts-with", lit.as_str()),
        SubsetPredicate::EndsWith(lit) => ("ends-with", lit.as_str()),
    };
    Some(serde_json::json!({
        "subset": subset.name,
        "kind": kind,
        "lit": lit,
    }))
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
                        "  async {name}(): Promise<void> {{\n    // TODO: operation is not executable in Silc 0.4.0\n  }}"
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
                        "    def {name}(self):\n        # TODO: operation is not executable in Silc 0.4.0\n        pass"
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
                        "func (m *{}) {}() {{\n\t// TODO: operation is not executable in Silc 0.4.0\n}}",
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

    /// The Go store worker and the TypeScript app worker write the same resource
    /// tables. When they disagreed on the column name, whichever created the
    /// table first won and the other's INSERT failed at runtime with
    /// "table guests has no column named data".
    #[test]
    fn store_and_app_workers_agree_on_the_resource_payload_column() {
        assert!(
            STORE_WORKER_GO.contains("INSERT OR REPLACE INTO %s (id, data)"),
            "store worker must write resource rows into `data`"
        );
        assert!(
            !STORE_WORKER_GO.contains("INSERT OR REPLACE INTO %s (id, payload)"),
            "store worker must not reintroduce a `payload` resource column"
        );
        assert!(
            APP_WORKER_TS.contains("INSERT INTO ${table} (id, data)"),
            "app worker must write resource rows into `data`"
        );
        // Both create resource tables with the columns the app worker updates.
        for column in ["data TEXT NOT NULL", "updated_at"] {
            assert!(
                STORE_WORKER_GO.contains(column),
                "store worker resource table missing `{column}`"
            );
            assert!(
                APP_WORKER_TS.contains(column),
                "app worker resource table missing `{column}`"
            );
        }
        // `app_events` is a separate shape both agree uses `payload`.
        assert!(STORE_WORKER_GO.contains("INSERT OR REPLACE INTO app_events (id, kind, payload)"));
        assert!(APP_WORKER_TS.contains("SELECT payload FROM app_events"));
    }

    /// Databases created before the column was unified must self-heal.
    #[test]
    fn app_worker_migrates_legacy_resource_tables() {
        assert!(APP_WORKER_TS.contains("RENAME COLUMN payload TO data"));
        assert!(APP_WORKER_TS.contains("migrateResourceTable(db, table)"));
    }

    fn output_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("silc-{label}-{nonce}"))
    }

    #[test]
    fn tensor_requirements_are_compiler_owned() {
        assert!(TENSOR_REQUIREMENTS.contains("onnxruntime=="));
        assert!(TENSOR_REQUIREMENTS.contains("tokenizers=="));
        assert!(TENSOR_REQUIREMENTS.contains("numpy=="));
        assert!(!TENSOR_REQUIREMENTS.contains("torch"));
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
@version("0.4.0")
contract Payload { has Str $.text; }
service Ingress {
    method fetch() { $url ==> http::get() ==> html::extract_body() }
}
processor Engine {
    method run(Payload $p) { $p.text ==> numpy::array() }
}
"#;

    const PIPELINE_SOURCE: &str = r#"
@version("0.4.0")
subset Uri of Str where { .contains("://") }
subset Emb384 of Vec[num32; 384];
contract ArticlePayload {
    has Uri $.url;
    has Str $.raw_content;
    has Emb384 $.vector_embedding;
}
service NetworkIngress {
    method fetch_article() {
        target_url ==> scrape::page(:js(false)) ==> scrape::extract(:into(ArticlePayload))
    }
}
processor EmbeddingEngine {
    method generate_vectors(ArticlePayload $article) {
        $article.raw_content ==> tensor::tokenize(:model("minilm-l6-v2"))
            ==> tensor::infer(:prefer(CPU))
    }
}
"#;

    #[test]
    fn emits_pipeline_only_bun_onnx_go_runtime() {
        let (_program, result, output) = parse_emit(PIPELINE_SOURCE, "tensor-pipeline");
        let graph = result.graph.as_ref().unwrap();
        assert!(graph.is_pipeline_only());
        assert_eq!(graph.processor_op, ProcessorOp::TensorInfer);

        let bun = fs::read_to_string(output.join("typescript/worker.ts")).unwrap();
        let python = fs::read_to_string(output.join("python/worker.py")).unwrap();
        let requirements =
            fs::read_to_string(output.join("python/tensor_requirements.txt")).unwrap();
        let go = fs::read_to_string(output.join("go/worker.go")).unwrap();
        let manifest = fs::read_to_string(&result.manifest).unwrap();
        assert!(bun.contains("fetchExtract"));
        assert!(bun.contains("maxExtractedBytes"));
        assert!(python.contains("CPUExecutionProvider"));
        assert!(python.contains("infer_embedding"));
        assert!(requirements.contains("onnxruntime=="));
        assert!(go.contains("modernc.org/sqlite"));
        assert!(manifest.contains("\"dimension\": 384"));
        assert!(manifest.contains("\"slot_capacity\": 65536"));
        for source in [&bun, &python, &go] {
            assert!(!source.contains("__SCHEMA_ID__"));
            assert!(!source.contains("__COMPILER_VERSION__"));
            assert!(source.contains("silc test"));
            assert!(!source.contains("TODO"));
        }
        fs::remove_dir_all(output).ok();
    }

    const FEEDBACK_SOURCE: &str = r#"
@version("0.4.0")
contract FeedbackRecord {
    has Str $.author;
    has Str $.text;
}
component FeedbackPage {
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
app FeedbackApp {
    route "/" => FeedbackPage;
}
processor TextAnalyzer {
    method analyze(FeedbackRecord $record) {
        $record.text ==> text::score()
    }
}
"#;

    const CHAT_SOURCE: &str = r#"
@version("0.4.0")
contract ChatRecord {
    has Str $.prompt;
    has Str $.reply;
}
component ChatPage {
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
                :context($.prompt),
                :persona("You are the test assistant, built on silclm."),
                :on(send(on_send))
            )
        )
    }
    method on_send() {
        Assistant.complete();
    }
}
app ChatApp {
    route "/" => ChatPage;
}
processor Assistant {
    method complete(ChatRecord $record) {
        $record.prompt ==> llm::complete()
    }
}
"#;

    const RESOURCE_SOURCE: &str = r#"
@version("0.4.0")
contract Product {
    has Str $.name;
    has num64 $.price;
}
resource Products for Product {
    query list;
    mutation create;
}
component ShopPage {
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
app ShopApp {
    route "/" => ShopPage;
}
"#;

    const API_SOURCE: &str = r#"
@version("0.4.0")
contract FeedbackRecord {
    has UUID $.id;
    has Str $.author;
    has Str $.text;
}
service FeedbackApi {
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
        assert!(!output.join("go").read_dir().unwrap().any(|e| {
            e.ok()
                .map(|e| e.path().extension().and_then(|x| x.to_str()) == Some("go"))
                .unwrap_or(false)
        }));
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
        assert_eq!(graph.sqlite_table, "feedback_records");

        let ts = fs::read_to_string(output.join("typescript/worker.ts")).unwrap();
        let py = fs::read_to_string(output.join("python/worker.py")).unwrap();
        let go = fs::read_to_string(output.join("go/worker.go")).unwrap();
        let app = fs::read_to_string(output.join("typescript/src/App.tsx")).unwrap();
        let terminal = fs::read_to_string(output.join("typescript/terminal.ts")).unwrap();
        let terminal_app =
            fs::read_to_string(output.join("typescript/src/TerminalApp.tsx")).unwrap();
        let terminal_runtime =
            fs::read_to_string(output.join("typescript/src/components/terminal/runtime.ts"))
                .unwrap();
        let button =
            fs::read_to_string(output.join("typescript/src/components/ui/button.tsx")).unwrap();
        let theme = fs::read_to_string(output.join("typescript/src/theme.css")).unwrap();
        let pkg = fs::read_to_string(output.join("typescript/package.json")).unwrap();
        assert!(
            terminal_app.contains("__silcNavigate") && terminal_app.contains("function App"),
            "OpenTUI TerminalApp must mirror routes with __silcNavigate"
        );
        assert!(
            terminal_runtime.contains("mountTerminalApp")
                && terminal_runtime.contains("@opentui/core"),
            "terminal runtime must mount OpenTUI"
        );
        assert!(
            terminal.contains("TERMINAL_SUBSTRATE") && pkg.contains("@opentui/core"),
            "manifest path must pin OpenTUI for ui::terminal"
        );
        assert!(
            output
                .join("typescript/src/components/ui/select.tsx")
                .is_file()
                && output
                    .join("typescript/src/components/ui/checkbox.tsx")
                    .is_file()
                && output
                    .join("typescript/src/components/ui/switch.tsx")
                    .is_file()
                && output
                    .join("typescript/src/components/ui/field.tsx")
                    .is_file()
                && output
                    .join("typescript/src/components/ui/badge.tsx")
                    .is_file()
                && output
                    .join("typescript/src/components/ui/alert.tsx")
                    .is_file()
                && output
                    .join("typescript/src/components/ui/divider.tsx")
                    .is_file()
                && output
                    .join("typescript/src/components/ui/section.tsx")
                    .is_file()
                && output
                    .join("typescript/src/components/ui/footer.tsx")
                    .is_file()
                && output
                    .join("typescript/src/components/ui/description-list.tsx")
                    .is_file()
                && output
                    .join("typescript/src/components/ui/tabs.tsx")
                    .is_file()
                && output
                    .join("typescript/src/components/ui/dialog.tsx")
                    .is_file(),
            "phase-1/2 web primitives must be emitted"
        );
        assert!(
            !output
                .join("typescript/src/components/ui/product-grid.tsx")
                .is_file(),
            "dead product-grid asset must not be emitted"
        );
        let terminal_components =
            fs::read_to_string(output.join("typescript/src/components/terminal/components.ts"))
                .unwrap();
        for export in [
            "SelectField",
            "Checkbox",
            "Switch",
            "Field",
            "Badge",
            "Alert",
            "Divider",
            "Section",
            "Footer",
            "DescriptionList",
            "Tabs",
            "Dialog",
            "DataTable",
        ] {
            assert!(
                terminal_components.contains(&format!("export function {export}")),
                "terminal components missing {export}"
            );
        }

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
            assert!(!source.contains("__HAS_SCRAPE__"));
            assert!(!source.contains("__SCRAPE_SITE__"));
            assert!(!source.contains("__SCRAPE_JS__"));
            assert!(!source.contains("__SCRAPE_DEPTH__"));
            assert!(!source.contains("__SCRAPE_SAME_HOST__"));
            assert!(!source.contains("__SCRAPE_LINK_CSS__"));
            assert!(!source.contains("__SCRAPE_SELECTS_JSON__"));
            assert!(!source.contains("__SCRAPE_TABLE__"));
            assert!(!source.contains("__ACTIONS_JSON__"));
            assert!(!source.contains("__SUBSET_RULES_JSON__"));
            assert!(!source.contains("__RESOURCE_TABLES__"));
            assert!(!source.contains("__RESOURCE_SEEDS_JSON__"));
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
        let nav_item =
            fs::read_to_string(output.join("typescript/src/components/ui/nav-item.tsx")).unwrap();
        assert!(
            nav_item.contains("onClick") && nav_item.contains("onClick={onClick}"),
            "NavItem must accept and wire the onClick prop so side-panel routing works"
        );
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
        assert!(manifest.contains("\"http_port\": 18088"));
        assert!(manifest.contains("\"sqlite_table\": \"feedback_records\""));
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
            app.contains("(__truthy(active_session)) ?"),
            "when conditions must lower through __truthy so empty collections are falsy"
        );
        assert!(
            app.contains("function __truthy"),
            "web app must define the shared __truthy helper"
        );
        let runtime =
            fs::read_to_string(output.join("typescript/src/components/terminal/runtime.ts"))
                .unwrap();
        assert!(
            runtime.contains("function renderComponent") && runtime.contains("function enterFrame"),
            "terminal hooks must be scoped per component instance; a shared state array \
             lets a route swap read another component's slots"
        );
        let terminal = fs::read_to_string(output.join("typescript/src/TerminalApp.tsx")).unwrap();
        for (surface, source) in [("web", &app), ("terminal", &terminal)] {
            let lines: Vec<&str> = source.lines().map(str::trim).collect();
            // A fragment-wrapped `null` is JSX *text*: React prints it and OpenTUI
            // throws "mount() received an invalid vnode".
            assert!(
                !lines.windows(2).any(|w| w[0] == "<>" && w[1] == "null"),
                "{surface} surface must lower an absent `else` to the JS value `null`, \
                 never a fragment-wrapped text child"
            );
            assert!(
                lines.iter().any(|line| *line == "null"),
                "{surface} surface must keep a bare `null` else branch"
            );
        }
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
        assert!(
            py.contains("compose_llm_prompt") && py.contains("record.pop(\"context\""),
            "silclm worker must ground prompts on context and strip it before persist"
        );
        assert!(
            py.contains("SILCLM_IDENTITY") && py.contains("record.pop(\"persona\""),
            "silclm worker must layer persona over the silclm identity and strip it before persist"
        );
        assert!(
            py.contains("context_is_empty") && py.contains("NO records"),
            "silclm worker must tell the model when the application context is empty"
        );
        assert!(
            app.contains("context: prompt") || app.contains("context:"),
            "chat :context must be included in /complete body"
        );
        assert!(
            app.contains("persona: \"You are the test assistant, built on silclm.\""),
            "chat :persona must be included in /complete body"
        );
        let ts = fs::read_to_string(output.join("typescript/worker.ts")).unwrap();
        assert!(ts.contains("/complete"));
        assert!(ts.contains("/history"));
        assert!(ts.contains("llm.complete") || ts.contains("true"));
        assert!(
            ts.contains("text: prompt") || ts.contains("text:"),
            "complete ingest must send ControlFrame::Ingest.text"
        );
        assert!(
            ts.contains("normalizeContext") && ts.contains("MAX_CONTEXT_CHARS"),
            "worker must bound chat context before INGEST"
        );
        assert!(
            ts.contains("normalizePersona") && ts.contains("MAX_PERSONA_CHARS"),
            "worker must bound chat persona before INGEST"
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

    const SCRAPE_SOURCE: &str = r#"
@version("0.4.0")
contract ScrapedPage {
    has Str $.id;
    has Str $.scrape_id;
    has Str $.scraped_at;
    has Str $.site;
    has Str $.url;
    has Str $.title;
    has Str $.snippet;
    has Str $.prompt;
    has Str $.summary;
    has Str $.summary_model;
    has Str $.depth;
    has Str $.status;
}
resource ScrapedPages for ScrapedPage {
    query list;
}
component Home {
    has state Str $.url = "";
    has state Str $.depth = "2";
    query $.pages = ScrapedPages.list();
    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Scraper"))),
            ui::form(:on(submit(on_submit)),
                ui::text_input(:field(url), :label("URL")),
                ui::button(:label("Scrape"), :variant("primary"), :submit)
            ),
            ui::table(
                :rows($.pages),
                :columns(["site", "title", "url", "scraped_at", "summary"]),
                :filter_column("site")
            )
        )
    }
    method on_submit() { submit(); ScrapedPages.list(); }
}
app ScraperApp {
    route "/" => Home;
}
service Crawler {
    method run() {
        target_url
            ==> scrape::site(:depth(2), :same_host(true), :js(false))
            ==> scrape::select(:css("title"), :as(title))
    }
}
processor Summarizer {
    method summarize(ScrapedPage $page) {
        $page.prompt ==> llm::complete()
    }
}
"#;

    #[test]
    fn emits_scrape_app_with_colly_sidecar() {
        let (_program, result, output) = parse_emit(SCRAPE_SOURCE, "scraper");
        let graph = result.graph.as_ref().unwrap();
        assert!(graph.has_scrape());
        assert!(graph.needs_llm());
        assert_eq!(graph.model_ref.as_deref(), Some("silclm"));
        assert!(graph.needs_scrape_crawl());
        assert!(!graph.needs_scrape_browser()); // :js(false)

        let ts = fs::read_to_string(output.join("typescript/worker.ts")).unwrap();
        assert!(ts.contains("HAS_SCRAPE = true") || ts.contains("const HAS_SCRAPE = true"));
        assert!(ts.contains("SCRAPE_SITE = true") || ts.contains("const SCRAPE_SITE = true"));
        assert!(ts.contains("runScrapeJob") || ts.contains("/scrape"));
        assert!(ts.contains("scraped_pages"));
        assert!(ts.contains("scrape_id"));
        assert!(ts.contains("scraped_at"));
        assert!(ts.contains("summarizeScrapedPage"));
        assert!(ts.contains("built on silclm"));
        assert!(ts.contains("summary_model"));
        assert!(ts.contains("SELECT id, data, created_at FROM"));
        assert!(!ts.contains("DELETE FROM ${SCRAPE_TABLE}"));
        assert!(!ts.contains("__HAS_SCRAPE__"));

        let app = fs::read_to_string(output.join("typescript/src/App.tsx")).unwrap();
        assert!(app.contains("filterColumn={\"site\"}"));
        let data_table =
            fs::read_to_string(output.join("typescript/src/components/ui/data-table.tsx")).unwrap();
        assert!(data_table.contains("facetOptions"));
        assert!(data_table.contains("aria-pressed"));

        assert!(output.join("go/crawl/worker.go").is_file());
        assert!(output.join("go/crawl/go.mod").is_file());
        let crawl = fs::read_to_string(output.join("go/crawl/worker.go")).unwrap();
        assert!(crawl.contains("gocolly/colly"));
        assert!(!output.join("python/browser_worker.py").is_file());
        assert!(output.join("python/requirements.txt").is_file());

        let manifest = fs::read_to_string(&result.manifest).unwrap();
        assert!(manifest.contains("go-colly-v1"));
        assert!(manifest.contains("\"scrape\": true") || manifest.contains("bun-fetch-v1"));
        assert!(manifest.contains("go/crawl/worker"));
        fs::remove_dir_all(output).ok();
    }

    const DOC_SOURCE: &str = r#"
@version("0.4.0")

contract Document {
    has Str $.title;
    has Str $.headings;
    has Str $.body;
    has Str $.tables;
    has Str $.filename;
    has Str $.mime;
    has Str $.format;
    has Str $.char_count;
}

resource Documents for Document {
    query list;
    mutation create;
    mutation delete;
}

component UploadPage {
    has state Str $.upload = "";
    method render() {
        ui::page(
            ui::heading(:text("Upload"), :level(1)),
            ui::form(
                :on(submit(on_submit)),
                ui::file_input(:field(upload), :label("Document"), :accept(".pdf,.docx,.odt,.md,.txt,.html")),
                ui::button(:label("Extract"), :variant("primary"), :submit)
            )
        )
    }
    method on_submit() { submit(); }
}

component DocumentsPage {
    query $.rows = Documents.list();
    method render() {
        ui::page(
            ui::heading(:text("Documents"), :level(1)),
            ui::table(:rows($.rows), :columns(["title", "filename", "format", "char_count"]))
        )
    }
}

app ExtractorApp {
    route "/" => UploadPage;
    route "/documents" => DocumentsPage;
}

service Extractor {
    method run() {
        $upload ==> doc::extract(:into(Document))
    }
}
"#;

    #[test]
    fn emits_doc_extract_upload_pipeline() {
        let (_program, result, output) = parse_emit(DOC_SOURCE, "doc_extractor");
        let graph = result.graph.as_ref().unwrap();
        assert!(graph.has_doc());
        assert_eq!(graph.doc.extract_into.as_deref(), Some("Document"));
        assert_eq!(graph.doc.table.as_deref(), Some("documents"));

        let ts = fs::read_to_string(output.join("typescript/worker.ts")).unwrap();
        assert!(ts.contains("HAS_DOC = true") || ts.contains("const HAS_DOC = true"));
        assert!(ts.contains("/upload"));
        assert!(ts.contains("runDocExtractJob"));
        assert!(ts.contains("DOC_TABLE = \"documents\"") || ts.contains("const DOC_TABLE = \"documents\""));
        assert!(!ts.contains("__HAS_DOC__"));
        assert!(!ts.contains("__DOC_TABLE__"));

        assert!(output.join("python/doc_extract_worker.py").is_file());
        assert!(output.join("python/doc_requirements.txt").is_file());
        let reqs = fs::read_to_string(output.join("python/doc_requirements.txt")).unwrap();
        assert!(reqs.contains("pypdf"));
        assert!(reqs.contains("python-docx"));

        let app = fs::read_to_string(output.join("typescript/src/App.tsx")).unwrap();
        assert!(app.contains("type=\"file\"") || app.contains("type='file'"));
        assert!(app.contains("/upload"));

        let manifest = fs::read_to_string(&result.manifest).unwrap();
        assert!(manifest.contains("python-doc-extract-v1"));
        assert!(manifest.contains("bun-multipart-v1"));
        assert!(manifest.contains("\"doc\": true") || manifest.contains("doc_extract_worker"));
        fs::remove_dir_all(output).ok();
    }

    #[test]
    fn rejects_legacy_class_declarators() {
        let source = r#"
@version("0.4.0")
class BadView is view {
    method render() { ui::page() }
}
"#;
        let err = sil_parser::parse(source).unwrap_err();
        assert!(
            err.message.contains("legacy `class`"),
            "expected migration diagnostic, got {err}"
        );
    }

    #[test]
    fn doc_extract_worker_parses_txt_and_md_fixtures() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates/fixtures/doc");
        let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates/doc_extract_worker.py");
        let python = which_python3();
        for (name, expect_title) in [("sample.txt", "Fixture Title"), ("sample.md", "Fixture Markdown")]
        {
            let path = fixtures.join(name);
            let output = std::process::Command::new(&python)
                .args([
                    worker.to_str().unwrap(),
                    "--path",
                    path.to_str().unwrap(),
                    "--filename",
                    name,
                    "--json",
                ])
                .output()
                .expect("spawn doc extract worker");
            assert!(
                output.status.success(),
                "extract {name} failed: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let json: serde_json::Value =
                serde_json::from_slice(&output.stdout).expect("extract json");
            assert_eq!(json["ok"], true);
            assert_eq!(json["title"], expect_title);
            assert!(json["body"].as_str().unwrap_or("").len() > 0);
            assert!(!json["char_count"].as_str().unwrap_or("").is_empty());
        }
    }

    fn which_python3() -> PathBuf {
        for candidate in ["python3", "python"] {
            if let Ok(output) = std::process::Command::new(candidate)
                .args(["-c", "import sys; print(sys.executable)"])
                .output()
            {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return PathBuf::from(path);
                    }
                }
            }
        }
        panic!("python3 required for doc extract fixture test");
    }

    #[test]
    fn emits_game_program_without_cdn_or_title_branching() {
        const GAME_SOURCE: &str =
            include_str!("../../../examples/arenaGameApp/main.silc");
        let (_program, _result, output) = parse_emit(GAME_SOURCE, "arena_game");

        let pkg = fs::read_to_string(output.join("typescript/package.json")).unwrap();
        assert!(
            pkg.contains("\"@babylonjs/core\": \"9.16.2\""),
            "package.json must pin babylon 9.16.2:\n{pkg}"
        );

        let template_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates/game");
        let mut scanned = Vec::new();
        scan_game_sources(&template_root, &mut scanned).expect("scan game templates");
        let joined = scanned.join("\n");
        for needle in [
            "unpkg.com",
            "jsdelivr",
            "cdnjs",
            "WebGL",
            "webgl",
            "SnowFlow",
            "SNOWFLOW",
            "title ===",
            "snowMaterial",
            "snow_surf",
        ] {
            assert!(
                !joined.contains(needle),
                "game templates must not contain `{needle}`"
            );
        }

        let manifest =
            fs::read_to_string(output.join("typescript/public/manifest.json")).unwrap();
        assert!(
            manifest.contains("\"title\": \"MEGASTRUCTURE\""),
            "arenaGameApp should lower cinematic FPS title"
        );
        assert!(manifest.contains("\"renderer\": \"webgpu\""));
        assert!(manifest.contains("\"prefabs\""));
        assert!(manifest.contains("\"toggle\": \"F1\""));
        assert!(manifest.contains("\"first_person\""));
        assert!(manifest.contains("VanguardAR") || manifest.contains("\"weapons\""));
        assert!(manifest.contains("SecurityLobby") || manifest.contains("\"zones\""));

        assert!(
            output.join("python/game_bake_worker.py").is_file(),
            "game emit must include CPython bake worker"
        );
        assert!(
            output.join("python/bake_plan.json").is_file(),
            "game emit must include bake plan"
        );
        let bake_plan = fs::read_to_string(output.join("python/bake_plan.json")).unwrap();
        assert!(bake_plan.contains("cpython-bake-v1"));
        assert!(bake_plan.contains("WalkDefault") || bake_plan.contains("prefabs"));
        assert!(
            output.join("go/worker.go").is_file(),
            "game emit must include Go store worker"
        );
        let go_worker = fs::read_to_string(output.join("go/worker.go")).unwrap();
        assert!(go_worker.contains("game_saves"));
        assert!(go_worker.contains("game_runs"));
        let root_manifest = fs::read_to_string(output.join("manifest.json")).unwrap();
        assert!(root_manifest.contains("python_bake"));
        assert!(root_manifest.contains("go_source"));
        assert!(root_manifest.contains("supervisor_socket"));

        fs::remove_dir_all(output).ok();
    }

    fn scan_game_sources(dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("read dir entry: {e}"))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "node_modules" || name == "dist" {
                continue;
            }
            let ft = entry
                .file_type()
                .map_err(|e| format!("stat {}: {e}", path.display()))?;
            if ft.is_dir() {
                scan_game_sources(&path, out)?;
            } else if ft.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(ext, "ts" | "tsx" | "js" | "json" | "html") {
                    let body = fs::read_to_string(&path)
                        .map_err(|e| format!("read {}: {e}", path.display()))?;
                    out.push(body);
                }
            }
        }
        Ok(())
    }
}
