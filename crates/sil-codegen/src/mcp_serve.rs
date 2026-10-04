//! Synthesized streamable-HTTP MCP server (THO-123).
//!
//! Programs with a web surface expose `POST /mcp`. Tools are derived at compile
//! time from author resources and schedule/manual loops. Auth is a bearer token
//! from `SILC_MCP_TOKEN` (never in source or the plan).

use sil_core::{resource_snake_case, Program, ResourceKind, TypeExpr};

/// Environment variable holding the MCP bearer token.
pub const MCP_AUTH_ENV: &str = "SILC_MCP_TOKEN";

/// Default HTTP path on the web surface.
pub const MCP_PATH: &str = "/mcp";

/// One compiled tool advertised on `tools/list` and dispatched by `tools/call`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpTool {
    pub name: String,
    pub kind: McpToolKind,
    pub description: String,
    pub table: Option<String>,
    pub method: Option<String>,
    pub http_method: Option<String>,
    pub path: Option<String>,
    pub loop_name: Option<String>,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpToolKind {
    Query,
    Mutation,
    RunNow,
    LoopRecent,
}

impl McpToolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            McpToolKind::Query => "query",
            McpToolKind::Mutation => "mutation",
            McpToolKind::RunNow => "run_now",
            McpToolKind::LoopRecent => "loop_recent",
        }
    }
}

/// True when the program has a web surface that can host `/mcp`.
pub fn program_serves_mcp(program: &Program) -> bool {
    !program.apps.is_empty()
}

fn is_loop_inbox_resource(name: &str, table: &str) -> bool {
    name.starts_with("Loop") || table.starts_with("loop_")
}

fn json_type_for_field(ty: &TypeExpr) -> &'static str {
    match ty.name() {
        "Num" | "Int" | "Float" | "num32" | "num64" => "number",
        "Bool" => "boolean",
        _ => "string",
    }
}

fn contract_input_schema(program: &Program, contract_name: &str) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    if let Some(contract) = program.contracts.iter().find(|c| c.name == contract_name) {
        for field in &contract.fields {
            properties.insert(
                field.name.clone(),
                serde_json::json!({ "type": json_type_for_field(&field.ty) }),
            );
            if field.name != "id" {
                required.push(field.name.clone());
            }
        }
    }
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn empty_object_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    })
}

/// Derive the fixed MCP tool list from author resources and runnable loops.
pub fn derive_mcp_tools(program: &Program) -> Vec<McpTool> {
    if !program_serves_mcp(program) {
        return Vec::new();
    }

    let mut tools = Vec::new();

    for resource in &program.resources {
        let table = resource.table_name();
        if is_loop_inbox_resource(&resource.name, &table) {
            continue;
        }
        // Prefer the actual table name for the tool prefix so `Talks` → `talks`.
        let prefix = table.clone();
        for method in &resource.methods {
            let name = format!("{prefix}_{}", method.name);
            let (kind, http_method, path, description, input_schema) =
                match (&method.kind, method.name.as_str()) {
                    (ResourceKind::Query, "list" | "all") => (
                        McpToolKind::Query,
                        Some("GET".into()),
                        Some(format!("/api/{table}")),
                        format!(
                            "Read rows from {} via GET /api/{table} (resource {}.{})",
                            resource.name, resource.name, method.name
                        ),
                        empty_object_schema(),
                    ),
                    (ResourceKind::Query, "get") => (
                        McpToolKind::Query,
                        Some("GET".into()),
                        Some(format!("/api/{table}/:id")),
                        format!(
                            "Read one row from {} by id (resource {}.{})",
                            resource.name, resource.name, method.name
                        ),
                        serde_json::json!({
                            "type": "object",
                            "properties": { "id": { "type": "string" } },
                            "required": ["id"],
                            "additionalProperties": false,
                        }),
                    ),
                    (ResourceKind::Query, other) => (
                        McpToolKind::Query,
                        Some("GET".into()),
                        Some(format!("/api/{table}/{other}")),
                        format!(
                            "Query {} via GET /api/{table}/{other} (resource {}.{})",
                            resource.name, resource.name, method.name
                        ),
                        empty_object_schema(),
                    ),
                    (ResourceKind::Mutation, "create" | "add") => (
                        McpToolKind::Mutation,
                        Some("POST".into()),
                        Some(format!("/api/{table}")),
                        format!(
                            "Create a row in {} via POST /api/{table} (resource {}.{})",
                            resource.name, resource.name, method.name
                        ),
                        resource
                            .contract
                            .as_deref()
                            .map(|c| contract_input_schema(program, c))
                            .unwrap_or_else(empty_object_schema),
                    ),
                    (ResourceKind::Mutation, "update") => (
                        McpToolKind::Mutation,
                        Some("PUT".into()),
                        Some(format!("/api/{table}/:id")),
                        format!(
                            "Update a row in {} via PUT /api/{table}/:id (resource {}.{})",
                            resource.name, resource.name, method.name
                        ),
                        resource
                            .contract
                            .as_deref()
                            .map(|c| contract_input_schema(program, c))
                            .unwrap_or_else(empty_object_schema),
                    ),
                    (ResourceKind::Mutation, "delete" | "remove") => (
                        McpToolKind::Mutation,
                        Some("DELETE".into()),
                        Some(format!("/api/{table}/:id")),
                        format!(
                            "Delete a row in {} via DELETE /api/{table}/:id (resource {}.{})",
                            resource.name, resource.name, method.name
                        ),
                        serde_json::json!({
                            "type": "object",
                            "properties": { "id": { "type": "string" } },
                            "required": ["id"],
                            "additionalProperties": false,
                        }),
                    ),
                    (ResourceKind::Mutation, other) => (
                        McpToolKind::Mutation,
                        Some("POST".into()),
                        Some(format!("/api/{table}/{other}")),
                        format!(
                            "Mutate {} via POST /api/{table}/{other} (resource {}.{})",
                            resource.name, resource.name, method.name
                        ),
                        resource
                            .contract
                            .as_deref()
                            .map(|c| contract_input_schema(program, c))
                            .unwrap_or_else(empty_object_schema),
                    ),
                };
            tools.push(McpTool {
                name,
                kind,
                description,
                table: Some(table.clone()),
                method: Some(method.name.clone()),
                http_method,
                path,
                loop_name: None,
                input_schema,
            });
        }
    }

    for lp in &program.loops {
        let trigger = lp.trigger().map(|t| t.name.as_str());
        if !matches!(trigger, Some("schedule") | Some("manual")) {
            continue;
        }
        let snake = resource_snake_case(&lp.name);
        tools.push(McpTool {
            name: format!("{snake}_run"),
            kind: McpToolKind::RunNow,
            description: format!(
                "Start loop {} now (same as Run now in /loops). No-overlap: returns the in-flight run when one is active.",
                lp.name
            ),
            table: None,
            method: None,
            http_method: None,
            path: None,
            loop_name: Some(lp.name.clone()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "requested_by": { "type": "string", "description": "Who requested the run" }
                },
                "additionalProperties": false,
            }),
        });
        tools.push(McpTool {
            name: format!("{snake}_recent"),
            kind: McpToolKind::LoopRecent,
            description: format!(
                "Recent runs and notices for loop {} (outcome of run_now without scraping /loops).",
                lp.name
            ),
            table: None,
            method: None,
            http_method: None,
            path: None,
            loop_name: Some(lp.name.clone()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "number", "description": "Max rows per list (default 20)" }
                },
                "additionalProperties": false,
            }),
        });
    }

    tools
}

impl McpTool {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "kind": self.kind.as_str(),
            "description": self.description,
            "table": self.table,
            "method": self.method,
            "http_method": self.http_method,
            "path": self.path,
            "loop": self.loop_name,
            "inputSchema": self.input_schema,
        })
    }
}

/// Plan / manifest fragment covering the synthesized MCP surface.
pub fn mcp_manifest_section(program: &Program) -> Option<serde_json::Value> {
    if !program_serves_mcp(program) {
        return None;
    }
    let tools: Vec<serde_json::Value> = derive_mcp_tools(program)
        .iter()
        .map(McpTool::to_json)
        .collect();
    Some(serde_json::json!({
        "path": MCP_PATH,
        "auth_env": MCP_AUTH_ENV,
        "tools": tools,
        "provenance": "compiler-owned MCP server from resources + loops (THO-123)",
    }))
}

/// Text printed by `silc build` listing tools and the auth env var.
pub fn format_mcp_report(program: &Program) -> Option<String> {
    if !program_serves_mcp(program) {
        return None;
    }
    let tools = derive_mcp_tools(program);
    let mut out = String::new();
    out.push_str(&format!(
        "MCP server: POST {MCP_PATH} (auth env: {MCP_AUTH_ENV})\n"
    ));
    if tools.is_empty() {
        out.push_str("  (no tools — declare a resource query/mutation or a schedule/manual loop)\n");
        return Some(out);
    }
    for tool in &tools {
        out.push_str(&format!("  {} — {}\n", tool.name, tool.description));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn talk_today() -> Program {
        let src = include_str!("../tests/fixtures/talk_today.silc");
        let program = sil_parser::parse(src).expect("parse");
        program.validate().expect("validate");
        program
    }

    #[test]
    fn talk_today_tools_include_list_and_run() {
        let program = crate::loop_lower::synthesize_loop_surface(&talk_today()).unwrap();
        let names: Vec<_> = derive_mcp_tools(&program)
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert!(names.contains(&"talks_list".into()), "{names:?}");
        assert!(names.contains(&"talk_today_run".into()), "{names:?}");
        assert!(names.contains(&"talk_today_recent".into()), "{names:?}");
        assert!(
            !names.iter().any(|n| n.starts_with("loop_")),
            "inbox resources must not become tools: {names:?}"
        );
    }

    #[test]
    fn report_names_auth_env() {
        let program = crate::loop_lower::synthesize_loop_surface(&talk_today()).unwrap();
        let report = format_mcp_report(&program).expect("report");
        assert!(report.contains(MCP_AUTH_ENV), "{report}");
        assert!(report.contains("talks_list"), "{report}");
        assert!(report.contains("talk_today_run"), "{report}");
    }
}
