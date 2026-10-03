//! Loop lowering (ADR-014): synthesize the `/loops` inbox, lower each loop to a
//! JSON step plan, pin it by content hash, and emit the Go loop kernel.

use sha2::{Digest, Sha256};
use sil_core::{
    loop_bounds, parse_loop_duration, Expr, Loop, LoopNode, Program, TypeExpr, UnaryOp,
};
use std::fs;
use std::path::{Path, PathBuf};

const LOOP_KERNEL_GO: &str = include_str!("../templates/loop_kernel.go");
const LOOP_GOMOD: &str = include_str!("../templates/loop_go.mod");

pub const LOOP_INBOX_ROUTE: &str = "/loops";
pub const LOOP_PLAN_VERSION: u32 = 1;

/// Reserved declarations the compiler adds for programs with loops.
pub const LOOP_RESERVED_NAMES: &[&str] = &[
    "LoopApproval",
    "LoopRun",
    "LoopNotice",
    "LoopRequest",
    "LoopApprovals",
    "LoopRuns",
    "LoopNotices",
    "LoopRequests",
    "LoopApprovalCard",
    "LoopInbox",
    "LoopsApp",
];

pub const LOOP_RUNS_TABLE: &str = "loop_runs";
pub const LOOP_APPROVALS_TABLE: &str = "loop_approvals";
pub const LOOP_NOTICES_TABLE: &str = "loop_notices";
pub const LOOP_REQUESTS_TABLE: &str = "loop_requests";

const SURFACE_DECLS: &str = r#"
contract LoopApproval {
    has Str $.id;
    has Str $.loop;
    has Str $.run_id;
    has Str $.step;
    has Str $.by;
    has Str $.message;
    has Str $.show;
    has Str $.summary;
    has Str $.status;
    has Str $.deadline;
    has Str $.decided_by;
    has Str $.decided_at;
    has Str $.note;
}

contract LoopRun {
    has Str $.id;
    has Str $.loop;
    has Str $.plan;
    has Str $.trigger;
    has Str $.identity;
    has Str $.status;
    has Str $.outcome;
    has Str $.detail;
    has Str $.started_at;
    has Str $.finished_at;
}

contract LoopNotice {
    has Str $.id;
    has Str $.loop;
    has Str $.run_id;
    has Str $.to;
    has Str $.text;
    has Str $.key;
}

contract LoopRequest {
    has Str $.id;
    has Str $.loop;
    has Str $.requested_by;
    has Str $.status;
    has Str $.run_id;
}

resource LoopApprovals for LoopApproval {
    query list;
    mutation update;
}

resource LoopRuns for LoopRun {
    query list;
}

resource LoopNotices for LoopNotice {
    query list;
}

resource LoopRequests for LoopRequest {
    query list;
    mutation create;
}

component LoopApprovalCard {
    has LoopApproval $.approval;
    emit approve(LoopApproval);
    emit decline(LoopApproval);

    method render() {
        ui::card(
            ui::heading(:text($.approval.message), :level(3)),
            ui::badge(:text($.approval.status)),
            ui::text(:text($.approval.summary)),
            ui::text(:text($.approval.show)),
            ui::text(:text($.approval.note)),
            when $.approval.status == "pending" {
                ui::toolbar(
                    ui::button(:label("Approve"), :variant("primary"), :on(click(on_approve))),
                    ui::button(:label("Decline"), :variant("destructive"), :on(click(on_decline)))
                )
            }
        )
    }

    method on_approve() {
        emit approve($.approval);
    }

    method on_decline() {
        emit decline($.approval);
    }
}
"#;

fn decision_handler(name: &str, status: &str) -> String {
    format!(
        r#"
    method {name}(LoopApproval $a) {{
        LoopApprovals.update(LoopApproval.new(
            :id($a.id),
            :loop($a.loop),
            :run_id($a.run_id),
            :step($a.step),
            :by($a.by),
            :message($a.message),
            :show($a.show),
            :summary($a.summary),
            :status("{status}"),
            :deadline($a.deadline),
            :decided_by($.decider),
            :decided_at(""),
            :note("")
        ));
        LoopApprovals.list();
        LoopRuns.list();
    }}
"#
    )
}

fn inbox_source(program: &Program) -> String {
    let manual: Vec<&Loop> = program
        .loops
        .iter()
        .filter(|l| l.trigger().is_some_and(|t| t.name == "manual" || t.name == "schedule"))
        .collect();
    let mut nav = Vec::new();
    if let Some(app) = program.apps.first() {
        for route in &app.routes {
            nav.push(format!(
                "ui::nav_item(:label(\"{}\"), :to(\"{}\"))",
                route.component, route.path
            ));
        }
    }
    nav.push(format!(
        "ui::nav_item(:label(\"Loops\"), :to(\"{LOOP_INBOX_ROUTE}\"), :active)"
    ));
    let run_now = if manual.is_empty() {
        String::new()
    } else {
        let buttons: Vec<String> = manual
            .iter()
            .enumerate()
            .map(|(i, l)| {
                format!(
                    "ui::button(:label(\"Run {} now\"), :variant(\"primary\"), :on(click(run_{i})))",
                    l.name
                )
            })
            .collect();
        format!(
            r#"
                ui::section(
                    :title("Run now"),
                    :description("Start a manual or scheduled loop now. Your name is recorded on the run."),
                    ui::toolbar({})
                ),"#,
            buttons.join(", ")
        )
    };
    let run_handlers: String = manual
        .iter()
        .enumerate()
        .map(|(i, l)| {
            format!(
                r#"
    method run_{i}() {{
        LoopRequests.create(LoopRequest.new(:loop("{}"), :requested_by($.decider), :status("pending"), :run_id("")));
        LoopRuns.list();
    }}
"#,
                l.name
            )
        })
        .collect();
    format!(
        r#"
component LoopInbox {{
    has state Str $.decider = "";
    query $.approvals = LoopApprovals.list();
    query $.runs = LoopRuns.list();
    query $.notices = LoopNotices.list();

    method render() {{
        ui::page(
            :app_bar(ui::app_bar(:title("Loops"))),
            :side_panel(ui::side_panel(
                {nav}
            )),
            ui::stack(
                ui::section(
                    :title("Approvals"),
                    :description("Loops wait here for a person. Enter your name; only the named approver can decide."),
                    ui::text_input(:field(decider), :label("Your name")),
                    ui::toolbar(ui::button(:label("Refresh"), :on(click(on_refresh)))),
                    when !$.approvals {{
                        ui::empty(:text("No approvals yet."))
                    }},
                    for $.approvals -> $approval {{
                        LoopApprovalCard(:approval($approval), :on(approve => on_approve), :on(decline => on_decline))
                    }}
                ),{run_now}
                ui::section(
                    :title("Runs"),
                    ui::table(
                        :rows($.runs),
                        :columns(["loop", "status", "outcome", "trigger", "started_at", "finished_at", "detail"]),
                        :empty_text("No runs yet."),
                        :sortable
                    )
                ),
                ui::section(
                    :title("Notices"),
                    ui::table(
                        :rows($.notices),
                        :columns(["loop", "to", "text"]),
                        :empty_text("No notices yet.")
                    )
                )
            )
        )
    }}
{approve}{decline}{run_handlers}
    method on_refresh() {{
        LoopApprovals.list();
        LoopRuns.list();
        LoopNotices.list();
    }}
}}
"#,
        nav = nav.join(",\n                "),
        approve = decision_handler("on_approve", "approved"),
        decline = decision_handler("on_decline", "declined"),
    )
}

/// Add the reserved inbox contracts, resources, components, and `/loops`
/// route. Programs without loops are returned unchanged.
pub fn synthesize_loop_surface(program: &Program) -> Result<Program, String> {
    if program.loops.is_empty() {
        return Ok(program.clone());
    }
    let taken = program
        .contracts
        .iter()
        .map(|c| c.name.as_str())
        .chain(program.resources.iter().map(|r| r.name.as_str()))
        .chain(program.components.iter().map(|c| c.name.as_str()))
        .chain(program.apps.iter().map(|a| a.name.as_str()));
    for name in taken {
        if LOOP_RESERVED_NAMES.contains(&name) {
            return Err(format!(
                "`{name}` is reserved for the synthesized loop inbox; rename it"
            ));
        }
    }
    for resource in &program.resources {
        let table = resource.table_name();
        if [LOOP_RUNS_TABLE, LOOP_APPROVALS_TABLE, LOOP_NOTICES_TABLE, LOOP_REQUESTS_TABLE]
            .contains(&table.as_str())
            || table.starts_with("loop_")
        {
            return Err(format!(
                "resource `{}` uses table `{table}`; tables starting with `loop_` belong to the loop kernel",
                resource.name
            ));
        }
    }
    if program
        .apps
        .iter()
        .flat_map(|a| a.routes.iter())
        .any(|r| r.path == LOOP_INBOX_ROUTE)
    {
        return Err(format!(
            "route `{LOOP_INBOX_ROUTE}` is reserved for the synthesized loop inbox"
        ));
    }

    let mut src = String::from(SURFACE_DECLS);
    src.push_str(&inbox_source(program));
    if program.apps.is_empty() {
        src.push_str(&format!(
            "\napp LoopsApp {{\n    route \"/\" => LoopInbox;\n    route \"{LOOP_INBOX_ROUTE}\" => LoopInbox;\n}}\n"
        ));
    }
    let synth = sil_parser::parse(&src)
        .map_err(|e| format!("internal: synthesized loop inbox failed to parse: {e}"))?;

    let mut out = program.clone();
    out.contracts.extend(synth.contracts);
    out.resources.extend(synth.resources);
    out.components.extend(synth.components);
    if let Some(app) = out.apps.first_mut() {
        app.routes.push(sil_core::Route {
            path: LOOP_INBOX_ROUTE.into(),
            component: "LoopInbox".into(),
            span: Default::default(),
        });
    } else {
        out.apps.extend(synth.apps);
    }
    out.validate()
        .map_err(|e| format!("internal: synthesized loop inbox is invalid: {e}"))?;
    Ok(out)
}

fn lower_expr(expr: &Expr) -> serde_json::Value {
    use serde_json::json;
    match expr {
        Expr::String(s) => json!({"k": "str", "s": s}),
        Expr::Number(n) => json!({"k": "num", "n": n.parse::<f64>().unwrap_or(0.0)}),
        Expr::Bool(b) => json!({"k": "bool", "b": b}),
        Expr::Var(name) => json!({"k": "var", "name": name}),
        Expr::Ident(name) => json!({"k": "str", "s": name}),
        Expr::Member { base, field } => {
            json!({"k": "member", "base": lower_expr(base), "field": field})
        }
        Expr::BinOp { op, left, right } => json!({
            "k": "bin",
            "op": sil_core::loops::loop_binop_symbol(op),
            "l": lower_expr(left),
            "r": lower_expr(right),
        }),
        Expr::Unary { op, expr } => json!({
            "k": match op { UnaryOp::Not => "not", UnaryOp::Neg => "neg" },
            "e": lower_expr(expr),
        }),
        Expr::List(items) => json!({"k": "list", "items": items.iter().map(lower_expr).collect::<Vec<_>>()}),
        Expr::New { ty, fields } => json!({
            "k": "new",
            "type": ty,
            "fields": fields
                .iter()
                .map(|(name, value)| json!({"name": name, "value": lower_expr(value)}))
                .collect::<Vec<_>>(),
        }),
        _ => json!({"k": "unknown"}),
    }
}

fn contract_fields(program: &Program, contract: &str) -> Vec<String> {
    program
        .contracts
        .iter()
        .find(|c| c.name == contract)
        .map(|c| c.fields.iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default()
}

fn resource_table_and_contract(program: &Program, resource: &str) -> (String, String) {
    program
        .resources
        .iter()
        .find(|r| r.name == resource)
        .map(|r| (r.table_name(), r.contract.clone().unwrap_or_default()))
        .unwrap_or_default()
}

fn lower_block(program: &Program, steps: &[LoopNode], prefix: &str) -> Vec<serde_json::Value> {
    steps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let path = if prefix.is_empty() {
                i.to_string()
            } else {
                format!("{prefix}.{i}")
            };
            lower_step(program, s, &path)
        })
        .collect()
}

fn lower_step(program: &Program, step: &LoopNode, path: &str) -> serde_json::Value {
    use serde_json::json;
    let mut v = json!({"op": step.name, "path": path});
    let obj = v.as_object_mut().unwrap();
    let mut put = |k: &str, val: serde_json::Value| {
        obj.insert(k.to_string(), val);
    };
    if let Some(name) = step.ident_prop("as") {
        put("as", json!(name));
    }
    for key in ["reason", "prompt", "key", "unchecked", "by", "message", "text"] {
        if let Some(s) = step.string_prop(key) {
            put(key, json!(s));
        }
    }
    for key in ["where", "url", "from", "that", "in", "value", "show"] {
        if step.name == "find" && key == "from" {
            continue;
        }
        if let Some(e) = step.prop(key) {
            put(key, lower_expr(e));
        }
    }
    match step.name.as_str() {
        "find" => {
            let (res, _) = step.ref_prop("from").unwrap_or_default();
            let (table, contract) = resource_table_and_contract(program, res);
            put("resource", json!(res));
            put("table", json!(table));
            put("row_fields", json!(contract_fields(program, &contract)));
            put("contract", json!(contract));
            if let Some(Expr::Var(f)) = step.prop("order") {
                put("order", json!(f));
            }
            put("desc", json!(step.has_flag("desc")));
            put("one", json!(step.has_flag("one")));
            put("max", json!(if step.has_flag("one") { 1 } else { step.number_prop("max").unwrap_or(0) }));
        }
        "read" => {
            put("read_op", json!(step.string_prop("op").unwrap_or("scrape::page")));
            for key in ["server", "tool", "auth_env", "select"] {
                if let Some(s) = step.string_prop(key) {
                    put(key, json!(s));
                }
            }
            if let Some(e) = step.prop("args") {
                put("args", lower_expr(e));
            }
            put("retry", json!(step.number_prop("retry").unwrap_or(sil_core::loops::DEFAULT_READ_RETRY)));
        }
        "ask" => {
            let into = step.ident_prop("into").unwrap_or_default();
            put("into", json!(into));
            put("retry", json!(step.number_prop("retry").unwrap_or(sil_core::loops::DEFAULT_ASK_RETRY)));
            if let Some(o) = step.child("otherwise") {
                put("otherwise", json!(lower_block(program, &o.children, &format!("{path}.otherwise"))));
            }
        }
        "gate" => {
            if let Some(o) = step.child("otherwise") {
                put("otherwise", json!(lower_block(program, &o.children, &format!("{path}.otherwise"))));
            }
        }
        "branch" => {
            let mut arms = Vec::new();
            for (i, arm) in step.children.iter().enumerate() {
                if arm.name == "when" {
                    arms.push(json!({
                        "that": arm.prop("that").map(lower_expr),
                        "steps": lower_block(program, &arm.children, &format!("{path}.when{i}")),
                    }));
                } else {
                    put("otherwise", json!(lower_block(program, &arm.children, &format!("{path}.otherwise"))));
                }
            }
            put("arms", json!(arms));
        }
        "each" => {
            put("max", json!(step.number_prop("max").unwrap_or(0)));
            put("steps", json!(lower_block(program, &step.children, &format!("{path}.each"))));
        }
        "write" => {
            let (res, cap) = step.ref_prop("to").unwrap_or_default();
            let (table, contract) = resource_table_and_contract(program, res);
            put("resource", json!(res));
            put("table", json!(table));
            put("contract", json!(contract));
            put("mode", json!(cap));
        }
        "notify" => {
            if let Some(s) = step.string_prop("to") {
                put("to", json!(s));
            }
        }
        "approve" => {
            let within = step
                .string_prop("within")
                .and_then(|w| parse_loop_duration(w).ok())
                .unwrap_or(0);
            put("within_minutes", json!(within));
            if let Some(d) = step.child("declined") {
                put("declined", json!(lower_block(program, &d.children, &format!("{path}.declined"))));
            }
            if let Some(t) = step.child("timed_out") {
                put("timed_out", json!(lower_block(program, &t.children, &format!("{path}.timed_out"))));
            }
        }
        _ => {}
    }
    v
}

fn lower_trigger(program: &Program, lp: &Loop) -> serde_json::Value {
    use serde_json::json;
    let Some(t) = lp.trigger() else {
        return json!({"kind": "none"});
    };
    match t.name.as_str() {
        "schedule" => json!({
            "kind": "schedule",
            "cron": t.string_prop("cron"),
            "tz": t.string_prop("tz"),
            "catch_up_minutes": t.string_prop("catch_up").and_then(|c| parse_loop_duration(c).ok()).unwrap_or(0),
        }),
        "on_mutation" => {
            let res = t.ident_prop("resource").unwrap_or_default();
            let (table, contract) = resource_table_and_contract(program, res);
            json!({
                "kind": "on_mutation",
                "resource": res,
                "table": table,
                "contract": contract,
                "mutation": t.ident_prop("mutation"),
            })
        }
        other => json!({"kind": other}),
    }
}

fn contract_schema(program: &Program) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for c in &program.contracts {
        let fields: Vec<serde_json::Value> = c
            .fields
            .iter()
            .map(|f| {
                let ty = match &f.ty {
                    TypeExpr::Named(n) => n.clone(),
                    TypeExpr::Optional(inner) => match inner.as_ref() {
                        TypeExpr::Named(n) => format!("{n}?"),
                        _ => "Any?".into(),
                    },
                    TypeExpr::Array(_) => "List".into(),
                    TypeExpr::Vec { .. } => "List".into(),
                };
                serde_json::json!({"name": f.name, "type": ty})
            })
            .collect();
        map.insert(c.name.clone(), serde_json::Value::Array(fields));
    }
    serde_json::Value::Object(map)
}

/// Lower every loop into one plan document and stamp its content hash.
pub fn lower_loop_plan(program: &Program, compiler_version: &str) -> serde_json::Value {
    use serde_json::json;
    let loops: Vec<serde_json::Value> = program
        .loops
        .iter()
        .map(|lp| {
            let b = loop_bounds(lp);
            json!({
                "name": lp.name,
                "tz": lp.time_zone(),
                "trigger": lower_trigger(program, lp),
                "bounds": {
                    "model_calls": b.model_calls,
                    "writes": b.writes,
                    "notices": b.notices,
                    "approvals": b.approvals,
                    "reads": b.reads,
                    "rows_scanned": b.rows_scanned,
                },
                "steps": lower_block(program, lp.steps(), ""),
            })
        })
        .collect();
    let mut plan = json!({
        "plan_version": LOOP_PLAN_VERSION,
        "compiler_version": compiler_version,
        "loops": loops,
        "contracts": contract_schema(program),
        "tables": {
            "runs": LOOP_RUNS_TABLE,
            "approvals": LOOP_APPROVALS_TABLE,
            "notices": LOOP_NOTICES_TABLE,
            "requests": LOOP_REQUESTS_TABLE,
        },
        "resource_tables": program.resources.iter().map(|r| r.table_name()).collect::<Vec<_>>(),
    });
    let canonical = serde_json::to_string(&plan).unwrap_or_default();
    let hash = format!("{:x}", Sha256::digest(canonical.as_bytes()));
    plan["hash"] = json!(hash);
    plan
}

/// Write `loop/plan.json`, the pinned `loop/plans/<hash>.json`, and the Go
/// kernel under `go/loop/`. Returns the plan hash.
pub fn emit_loop_kernel(
    root: &Path,
    program: &Program,
    compiler_version: &str,
    generated: &mut Vec<PathBuf>,
) -> Result<String, String> {
    let plan = lower_loop_plan(program, compiler_version);
    let hash = plan["hash"].as_str().unwrap_or_default().to_string();
    let text = serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?;
    let plans_dir = root.join("loop").join("plans");
    fs::create_dir_all(&plans_dir).map_err(|e| format!("create {}: {e}", plans_dir.display()))?;
    let current = root.join("loop").join("plan.json");
    fs::write(&current, &text).map_err(|e| format!("write {}: {e}", current.display()))?;
    let pinned = plans_dir.join(format!("{hash}.json"));
    if !pinned.exists() {
        fs::write(&pinned, &text).map_err(|e| format!("write {}: {e}", pinned.display()))?;
    }
    generated.push(current);
    generated.push(pinned);

    let go_dir = root.join("go").join("loop");
    fs::create_dir_all(&go_dir).map_err(|e| format!("create {}: {e}", go_dir.display()))?;
    let kernel = go_dir.join("kernel.go");
    fs::write(&kernel, LOOP_KERNEL_GO).map_err(|e| format!("write {}: {e}", kernel.display()))?;
    let gomod = go_dir.join("go.mod");
    fs::write(&gomod, LOOP_GOMOD).map_err(|e| format!("write {}: {e}", gomod.display()))?;
    generated.push(kernel);
    generated.push(gomod);
    Ok(hash)
}

/// Kernel source and go.mod, for tests that build the kernel directly.
pub fn loop_kernel_sources() -> (&'static str, &'static str) {
    (LOOP_KERNEL_GO, LOOP_GOMOD)
}