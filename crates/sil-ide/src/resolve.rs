//! Hover resolution: map a byte offset to Markdown documentation.

use crate::docs::{
    builtin_call_doc, builtin_type_doc, executable_op_doc, keyword_doc, namespace_doc, op_prop_doc,
    operator_doc, resource_method_summary, stub_op_doc, unit_literal_doc,
};
use crate::document::{Document, HoverContent, HoverRange};
use sil_core::{
    event_doc, format_component_catalog_line, game_closed_value_doc, game_prop_doc,
    is_executable_op, lookup_component, lookup_game_node, prop_doc, Component, Expr, Program,
    ResourceKind, Span, TypeExpr, UiTemplate,
};
use sil_lexer::Token;

pub fn resolve_hover(doc: &Document, offset: u32) -> Option<HoverContent> {
    let idx = doc.token_index_at(offset)?;
    let token = &doc.tokens[idx];
    let range = HoverRange::from_span(
        &doc.source,
        Span::new(token.start, token.end, token.line, token.col),
    );

    // Prefer AST-backed resolution for idents / members / ui / ops.
    if let Some(content) = resolve_ident_context(doc, idx) {
        return Some(content);
    }

    match &token.token {
        Token::Annotation(name) => Some(HoverContent {
            markdown: md(
                "annotation",
                &format!("@{name}"),
                "Source annotation attached to the following declaration. \
                 `@version(\"0.4.0\")` pins the Silc language version so tooling and the \
                 compiler agree on syntax and runnable ops.",
                None,
            ),
            range,
        }),
        Token::UnitLiteral(lit) => {
            let prose = unit_literal_doc(lit).unwrap_or(
                "Unit literal combining a magnitude with a closed Silc unit suffix.",
            );
            Some(HoverContent {
                markdown: md("unit literal", lit, prose, None),
                range,
            })
        }
        Token::Ident(name) => {
            if let Some(doc_text) = builtin_type_doc(name) {
                return Some(HoverContent {
                    markdown: md("builtin type", name, doc_text, None),
                    range,
                });
            }
            // Fall through to unknown ident — still try declaration lookup.
            if let Some(content) = resolve_declaration_name(doc, name, range.clone()) {
                return Some(content);
            }
            None
        }
        other => {
            // Keyword used as a namespace qualifier (`game::…`, `service::…`).
            if let Some(ns) = namespace_from_token(other) {
                if idx + 1 < doc.tokens.len()
                    && matches!(doc.tokens[idx + 1].token, Token::DoubleColon)
                {
                    if let Some(text) = namespace_doc(ns) {
                        return Some(HoverContent {
                            markdown: md("namespace", ns, &text, None),
                            range,
                        });
                    }
                }
            }
            let slice = token.slice.as_str();
            if let Some(text) = keyword_from_token(other).and_then(keyword_doc) {
                let kw = keyword_from_token(other).unwrap();
                return Some(HoverContent {
                    markdown: md("keyword", kw, text, None),
                    range,
                });
            }
            if let Some(text) = operator_doc(slice) {
                return Some(HoverContent {
                    markdown: md("operator", slice, text, None),
                    range,
                });
            }
            None
        }
    }
}

/// Extract a namespace string from either `Ident("ui")` or keyword tokens like `Game`.
fn namespace_from_token(token: &Token) -> Option<&str> {
    match token {
        Token::Ident(ns) => Some(ns.as_str()),
        Token::Game => Some("game"),
        Token::Service => Some("service"),
        Token::Processor => Some("processor"),
        Token::App => Some("app"),
        Token::Resource => Some("resource"),
        Token::Sink => Some("sink"),
        Token::Task => Some("task"),
        _ => None,
    }
}

fn keyword_from_token(token: &Token) -> Option<&'static str> {
    Some(match token {
        Token::Subset => "subset",
        Token::Class => "class",
        Token::Contract => "contract",
        Token::Component => "component",
        Token::Resource => "resource",
        Token::App => "app",
        Token::Game => "game",
        Token::Service => "service",
        Token::Processor => "processor",
        Token::Sink => "sink",
        Token::Task => "task",
        Token::Has => "has",
        Token::Method => "method",
        Token::Is => "is",
        Token::Of => "of",
        Token::Where => "where",
        Token::Query => "query",
        Token::Mutation => "mutation",
        Token::Seed => "seed",
        Token::Slot => "slot",
        Token::Emit => "emit",
        Token::State => "state",
        Token::When => "when",
        Token::For => "for",
        Token::Else => "else",
        Token::Route => "route",
        Token::Await => "await",
        _ => return None,
    })
}

fn resolve_ident_context(doc: &Document, idx: usize) -> Option<HoverContent> {
    let token = &doc.tokens[idx];
    let Token::Ident(name) = &token.token else {
        // Also handle `$name` where caret is on the ident after `$`.
        return None;
    };
    let range = HoverRange::from_span(
        &doc.source,
        Span::new(token.start, token.end, token.line, token.col),
    );

    // ui::component  or  ns::op  (ns may be Ident or a keyword like Game/Service)
    if idx >= 2 && matches!(doc.tokens[idx - 1].token, Token::DoubleColon) {
        if let Some(ns) = namespace_from_token(&doc.tokens[idx - 2].token) {
            let ns_span = Span::new(
                doc.tokens[idx - 2].start,
                token.end,
                doc.tokens[idx - 2].line,
                doc.tokens[idx - 2].col,
            );
            let ns_range = HoverRange::from_span(&doc.source, ns_span);
            if ns == "ui" {
                if let Some(spec) = lookup_component(name) {
                    let line = format_component_catalog_line(spec);
                    let signature = line.trim_start_matches('-').trim();
                    let detail = format!("{}\n\n{}", spec.description, signature);
                    return Some(HoverContent {
                        markdown: md("ui primitive", &format!("ui::{name}"), &detail, None),
                        range: ns_range,
                    });
                }
            }
            if ns == "game" {
                if let Some(spec) = lookup_game_node(name) {
                    let props: Vec<String> = spec
                        .props
                        .iter()
                        .map(|p| {
                            if p.required {
                                format!(":{}(...)", p.name)
                            } else {
                                format!(":{}?", p.name)
                            }
                        })
                        .collect();
                    let detail = format!(
                        "{}\n\n`game::{}({})`",
                        spec.description,
                        spec.name,
                        props.join(", ")
                    );
                    return Some(HoverContent {
                        markdown: md("game node", &format!("game::{name}"), &detail, None),
                        range: ns_range,
                    });
                }
            }
            if let Some(text) = executable_op_doc(ns, name) {
                return Some(HoverContent {
                    markdown: md("executable op", &format!("{ns}::{name}"), &text, None),
                    range: ns_range,
                });
            }
            if !is_executable_op(ns, name) {
                return Some(HoverContent {
                    markdown: md(
                        "namespace op",
                        &format!("{ns}::{name}"),
                        &stub_op_doc(ns, name),
                        None,
                    ),
                    range: ns_range,
                });
            }
        }
    }

    // Member access: Base.field  or  $var.field
    if idx >= 2 && matches!(doc.tokens[idx - 1].token, Token::Dot) {
        if let Some(content) = resolve_member_field(doc, idx, name, range.clone()) {
            return Some(content);
        }
        // Type.new / Str.contains style builtins after `.`
        if let Some(text) = builtin_call_doc(name) {
            return Some(HoverContent {
                markdown: md("builtin", name, text, None),
                range,
            });
        }
    }

    // Variable: $name or $.name
    if idx >= 1 && matches!(doc.tokens[idx - 1].token, Token::Dollar) {
        // $.name
        if idx >= 2 && matches!(doc.tokens[idx - 2].token, Token::Dollar) {
            // shouldn't happen
        }
        let is_dot_var = idx >= 2
            && matches!(doc.tokens[idx - 1].token, Token::Dot)
            && matches!(doc.tokens[idx - 2].token, Token::Dollar);
        if is_dot_var || matches!(doc.tokens[idx - 1].token, Token::Dollar) {
            if let Some(content) = resolve_var(doc, token.start, name, range.clone()) {
                return Some(content);
            }
        }
    }
    // $.name where tokens are Dollar, Dot, Ident
    if idx >= 2
        && matches!(doc.tokens[idx - 1].token, Token::Dot)
        && matches!(doc.tokens[idx - 2].token, Token::Dollar)
    {
        if let Some(content) = resolve_var(doc, token.start, name, range.clone()) {
            return Some(content);
        }
    }

    // Prop key :name(  — UI, game, op, author component, contract.new
    if idx >= 1 && matches!(doc.tokens[idx - 1].token, Token::Colon) {
        if let Some(content) = resolve_ui_prop(doc, idx, name, range.clone()) {
            return Some(content);
        }
        if let Some(content) = resolve_game_prop(doc, idx, name, range.clone()) {
            return Some(content);
        }
        if let Some(content) = resolve_op_prop(doc, idx, name, range.clone()) {
            return Some(content);
        }
        if let Some(content) = resolve_author_prop(doc, idx, name, range.clone()) {
            return Some(content);
        }
    }

    // Closed enum / event value inside :prop(value) or :on(event(...))
    if idx >= 2 && matches!(doc.tokens[idx - 1].token, Token::LParen) {
        if let Some(content) = resolve_paren_ident(doc, idx, name, range.clone()) {
            return Some(content);
        }
    }

    // Namespace qualifier: `ui` in `ui::table`
    if idx + 1 < doc.tokens.len() && matches!(doc.tokens[idx + 1].token, Token::DoubleColon) {
        if let Some(text) = namespace_doc(name) {
            return Some(HoverContent {
                markdown: md("namespace", name, &text, None),
                range,
            });
        }
    }

    // Builtin calls: submit(), navigate(...)
    if idx + 1 < doc.tokens.len() && matches!(doc.tokens[idx + 1].token, Token::LParen) {
        if let Some(text) = builtin_call_doc(name) {
            return Some(HoverContent {
                markdown: md("builtin", name, text, None),
                range,
            });
        }
    }

    // Plain declaration / type / resource / component name
    if let Some(content) = resolve_declaration_name(doc, name, range.clone()) {
        return Some(content);
    }

    if let Some(text) = builtin_type_doc(name) {
        return Some(HoverContent {
            markdown: md("builtin type", name, text, None),
            range,
        });
    }

    // Last-chance closed game/UI enum tokens (when paren walk failed).
    if let Some(text) = game_closed_value_doc(name).or_else(|| ui_closed_value_doc(name)) {
        return Some(HoverContent {
            markdown: md("closed value", name, text, None),
            range,
        });
    }

    None
}

fn resolve_member_field(
    doc: &Document,
    idx: usize,
    field: &str,
    range: HoverRange,
) -> Option<HoverContent> {
    let base_name = preceding_base_name(doc, idx)?;
    let program = &doc.program;

    // Resource.method
    if let Some(resource) = program.resources.iter().find(|r| r.name == base_name) {
        if let Some(method) = resource.find_method(field) {
            let kind = match method.kind {
                ResourceKind::Query => "query",
                ResourceKind::Mutation => "mutation",
            };
            let ret = method
                .return_ty
                .as_ref()
                .map(|t| t.display())
                .unwrap_or_else(|| "()".into());
            let params = method
                .params
                .iter()
                .map(|p| {
                    format!(
                        "{}: {}",
                        p.name,
                        p.ty.as_ref()
                            .map(|t| t.display())
                            .unwrap_or_else(|| "_".into())
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let sig = format!("({params}) -> {ret}");
            let http = synthesized_http(resource, method.name.as_str(), method.kind);
            let mut detail = format!(
                "{}\n\n| | |\n|---|---|\n| **Resource** | `{}` |\n| **Signature** | `{sig}` |",
                resource_method_summary(kind, field),
                resource.name
            );
            if let Some(http) = http {
                detail.push_str(&format!("\n| **HTTP** | `{http}` |"));
            }
            if let Some(contract) = &resource.contract {
                detail.push_str(&format!("\n| **Contract** | `{contract}` |"));
            }
            return Some(HoverContent {
                markdown: md(kind, field, &detail, None),
                range,
            });
        }
        // Unknown method on known resource
        return Some(HoverContent {
            markdown: md(
                "member",
                field,
                &format!(
                    "Member `{field}` on resource `{}`. It is not a declared query or \
                     mutation on this resource; check the resource's method list for legal calls.",
                    resource.name
                ),
                None,
            ),
            range,
        });
    }

    // $var.field — resolve var type then contract field
    let scope = scope_at(doc, doc.tokens[idx].start);
    if let Some(ty) = scope.get(&base_name) {
        if let Some(content) = contract_field_hover(program, ty, field, range.clone()) {
            return Some(content);
        }
        return Some(HoverContent {
            markdown: md(
                "member",
                field,
                &format!(
                    "Member `{field}` accessed on `{}` (type `{}`). No matching contract \
                     field was found for that type in the current program.",
                    base_name,
                    ty.display()
                ),
                None,
            ),
            range,
        });
    }

    // Bare Ident used as typed value in scope? Already handled.
    // Contract name.field is uncommon; try contracts by name.
    if let Some(contract) = program.contracts.iter().find(|c| c.name == base_name) {
        if let Some(f) = contract.fields.iter().find(|f| f.name == field) {
            return Some(HoverContent {
                markdown: md(
                    "field",
                    field,
                    &format!(
                        "Field on contract `{base_name}`. Its type shapes storage columns, \
                         form bindings, and member access such as `$row.{field}`.\n\n\
                         | | |\n|---|---|\n| **Type** | `{}` |",
                        f.ty.display()
                    ),
                    None,
                ),
                range,
            });
        }
    }

    None
}

fn preceding_base_name(doc: &Document, idx: usize) -> Option<String> {
    // patterns:
    // Ident . field
    // Dollar Ident . field
    // Dollar Dot Ident . field  ($.articles.something — rare)
    if idx < 2 {
        return None;
    }
    let before_dot = &doc.tokens[idx - 2];
    match &before_dot.token {
        Token::Ident(s) => {
            // Maybe Dollar before ident: $article.id
            if idx >= 3 && matches!(doc.tokens[idx - 3].token, Token::Dollar) {
                return Some(s.clone());
            }
            Some(s.clone())
        }
        Token::Dot if idx >= 3 => {
            // $.name.field
            if let Token::Ident(s) = &doc.tokens[idx - 1].token {
                // wait, idx-1 is Dot for field... this branch is before_dot = tokens[idx-2]
                let _ = s;
            }
            if matches!(doc.tokens[idx - 2].token, Token::Dot)
                && idx >= 3
                && matches!(&doc.tokens[idx - 3].token, Token::Ident(s) if true)
            {
                if let Token::Ident(s) = &doc.tokens[idx - 3].token {
                    return Some(s.clone());
                }
            }
            None
        }
        _ => None,
    }
}

fn resolve_var(
    doc: &Document,
    offset: u32,
    name: &str,
    range: HoverRange,
) -> Option<HoverContent> {
    let scope = scope_at(doc, offset);
    if let Some(ty) = scope.get(name) {
        // Query binding?
        if let Some(comp) = enclosing_component(doc, offset) {
            if let Some(q) = comp.queries.iter().find(|q| q.name == name) {
                let ret = doc
                    .program
                    .resources
                    .iter()
                    .find(|r| r.name == q.resource)
                    .and_then(|r| r.find_method(&q.method))
                    .and_then(|m| m.return_ty.clone())
                    .unwrap_or_else(|| ty.clone());
                return Some(HoverContent {
                    markdown: md(
                        "query binding",
                        &format!("$.{name}"),
                        &format!(
                            "Read-only component query binding. The runtime re-runs \
                             `{}.{}()` when dependencies invalidate; assign through a \
                             mutation handler instead of writing this binding directly.\n\n\
                             | | |\n|---|---|\n| **Source** | `{}.{}()` |\n| **Type** | `{}` |",
                            q.resource,
                            q.method,
                            q.resource,
                            q.method,
                            ret.display()
                        ),
                        None,
                    ),
                    range,
                });
            }
            if let Some(field) = comp.all_fields().find(|f| f.name == name) {
                let (kind, prose) = if field.is_state {
                    (
                        "state",
                        "Mutable component-owned state. Handlers may assign it; the value \
                         persists across renders until the component unmounts.",
                    )
                } else {
                    (
                        "prop",
                        "Incoming component prop supplied by the parent. Treat it as \
                         read-only inside this component; lift writes to the parent or to \
                         local `state`.",
                    )
                };
                return Some(HoverContent {
                    markdown: md(
                        kind,
                        &format!("$.{name}"),
                        &format!(
                            "{prose}\n\n| | |\n|---|---|\n| **Component** | `{}` |\n| **Type** | `{}` |",
                            comp.name,
                            field.ty.display()
                        ),
                        None,
                    ),
                    range,
                });
            }
            if let Some(handler) = enclosing_handler(comp, offset) {
                if let Some(param) = handler.params.iter().find(|p| p.name == name) {
                    let ty_s = param
                        .ty
                        .as_ref()
                        .map(|t| t.display())
                        .unwrap_or_else(|| "_".into());
                    return Some(HoverContent {
                        markdown: md(
                            "parameter",
                            &format!("${name}"),
                            &format!(
                                "Handler parameter in scope only for this invocation. Use it \
                                 for event payloads and explicit arguments passed into `{}`.\n\n\
                                 | | |\n|---|---|\n| **Handler** | `{}` |\n| **Type** | `{ty_s}` |",
                                handler.name, handler.name
                            ),
                            None,
                        ),
                        range,
                    });
                }
            }
        }
        return Some(HoverContent {
            markdown: md(
                "variable",
                &format!("${name}"),
                &format!(
                    "Local variable in the current scope. Its lifetime is the enclosing \
                     handler, loop, or block — not component state.\n\n\
                     | | |\n|---|---|\n| **Type** | `{}` |",
                    ty.display()
                ),
                None,
            ),
            range,
        });
    }
    None
}

fn resolve_ui_prop(
    doc: &Document,
    idx: usize,
    prop: &str,
    range: HoverRange,
) -> Option<HoverContent> {
    // Walk left to find nearest ui::component or author component call.
    let mut i = idx;
    while i > 0 {
        i -= 1;
        if matches!(doc.tokens[i].token, Token::Ident(_))
            && i >= 2
            && matches!(doc.tokens[i - 1].token, Token::DoubleColon)
            && matches!(&doc.tokens[i - 2].token, Token::Ident(ns) if ns == "ui")
        {
            let Token::Ident(comp) = &doc.tokens[i].token else {
                break;
            };
            if let Some(spec) = lookup_component(comp) {
                if let Some(p) = spec.props.iter().find(|p| p.name == prop) {
                    let req = if p.required { "required" } else { "optional" };
                    let prose = prop_doc(comp, prop).unwrap_or(
                        "Prop on this UI primitive. Consult the component catalog for details.",
                    );
                    return Some(HoverContent {
                        markdown: md(
                            "ui prop",
                            prop,
                            &format!(
                                "{prose}\n\n| | |\n|---|---|\n| **Component** | `ui::{comp}` |\n| **Required** | `{req}` |"
                            ),
                            None,
                        ),
                        range,
                    });
                }
                if let Some(slot) = spec.slots.iter().find(|s| s.name == prop) {
                    let req = if slot.required { "required" } else { "optional" };
                    let prose = slot_doc(prop).unwrap_or(
                        "Named content slot on this UI primitive. Parents fill the slot with a child tree.",
                    );
                    return Some(HoverContent {
                        markdown: md(
                            "ui slot",
                            prop,
                            &format!(
                                "{prose}\n\n| | |\n|---|---|\n| **Component** | `ui::{comp}` |\n| **Fills** | `ui::{}` |\n| **Required** | `{req}` |",
                                slot.component
                            ),
                            None,
                        ),
                        range,
                    });
                }
                if let Some(ev) = spec.events.iter().find(|e| e.name == prop) {
                    let prose = event_doc(comp, ev.name).unwrap_or(
                        "Event emitted by this UI primitive. Bind it with `:on(event(handler))`.",
                    );
                    return Some(HoverContent {
                        markdown: md(
                            "ui event",
                            prop,
                            &format!(
                                "{prose}\n\n| | |\n|---|---|\n| **Component** | `ui::{comp}` |\n| **Bind** | `:on({}(handler))` |",
                                ev.name
                            ),
                            None,
                        ),
                        range,
                    });
                }
            }
            break;
        }
    }
    // `:on` is special
    if prop == "on" {
        return Some(HoverContent {
            markdown: md(
                "ui binding",
                "on",
                "Event binding form that connects a UI event to a handler: \
                 `:on(event(handler))` or `:on(event => handler)`. The event name must be \
                 declared on the primitive (for example `click`, `change`, or `submit`).",
                None,
            ),
            range,
        });
    }
    None
}

fn slot_doc(name: &str) -> Option<&'static str> {
    Some(match name {
        "app_bar" => {
            "Top chrome slot on `ui::page`. Fill it with `ui::app_bar` for the screen title and primary actions."
        }
        "side_panel" => {
            "Side navigation slot on `ui::page`. Fill it with `ui::side_panel` hosting `nav_item` children."
        }
        "footer" => {
            "Bottom chrome slot on `ui::page`. Fill it with `ui::footer` for secondary links or legal copy."
        }
        "actions" => {
            "Actions slot on `ui::card`. Fill it with a `ui::row` of buttons for card-level controls."
        }
        _ => return None,
    })
}

fn resolve_game_prop(
    doc: &Document,
    idx: usize,
    prop: &str,
    range: HoverRange,
) -> Option<HoverContent> {
    let mut i = idx;
    while i > 0 {
        i -= 1;
        if matches!(doc.tokens[i].token, Token::Ident(_))
            && i >= 2
            && matches!(doc.tokens[i - 1].token, Token::DoubleColon)
            && matches!(doc.tokens[i - 2].token, Token::Game)
        {
            let Token::Ident(node) = &doc.tokens[i].token else {
                break;
            };
            if let Some(spec) = lookup_game_node(node) {
                if let Some(p) = spec.props.iter().find(|p| p.name == prop) {
                    let req = if p.required { "required" } else { "optional" };
                    let prose = game_prop_doc(node, prop).unwrap_or(p.description);
                    let closed = if p.closed_values.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "\n| **Closed values** | `{}` |",
                            p.closed_values.join("` \\| `")
                        )
                    };
                    return Some(HoverContent {
                        markdown: md(
                            "game prop",
                            prop,
                            &format!(
                                "{prose}\n\n| | |\n|---|---|\n| **Node** | `game::{node}` |\n| **Kind** | `{:?}` |\n| **Required** | `{req}` |{closed}",
                                p.kind
                            ),
                            None,
                        ),
                        range,
                    });
                }
            }
            break;
        }
    }
    None
}

fn resolve_op_prop(
    doc: &Document,
    idx: usize,
    prop: &str,
    range: HoverRange,
) -> Option<HoverContent> {
    let mut i = idx;
    while i > 0 {
        i -= 1;
        if matches!(doc.tokens[i].token, Token::Ident(_))
            && i >= 2
            && matches!(doc.tokens[i - 1].token, Token::DoubleColon)
        {
            if let Some(ns) = namespace_from_token(&doc.tokens[i - 2].token) {
                let Token::Ident(op) = &doc.tokens[i].token else {
                    break;
                };
                if ns == "ui" || ns == "game" {
                    break;
                }
                if let Some(prose) = op_prop_doc(ns, op, prop) {
                    return Some(HoverContent {
                        markdown: md(
                            "op prop",
                            prop,
                            &format!(
                                "{prose}\n\n| | |\n|---|---|\n| **Operation** | `{ns}::{op}` |"
                            ),
                            None,
                        ),
                        range,
                    });
                }
            }
            break;
        }
    }
    None
}

fn resolve_author_prop(
    doc: &Document,
    idx: usize,
    prop: &str,
    range: HoverRange,
) -> Option<HoverContent> {
    // Walk left for ComponentName(...) or Contract.new(...)
    let mut i = idx;
    while i > 0 {
        i -= 1;
        match &doc.tokens[i].token {
            Token::Ident(base) => {
                // Contract.new(:field) — look for `.new` just after base
                if i + 2 < doc.tokens.len()
                    && matches!(doc.tokens[i + 1].token, Token::Dot)
                    && matches!(&doc.tokens[i + 2].token, Token::Ident(m) if m == "new")
                {
                    if let Some(contract) = doc.program.contracts.iter().find(|c| c.name == *base) {
                        if let Some(field) = contract.fields.iter().find(|f| f.name == prop) {
                            return Some(HoverContent {
                                markdown: md(
                                    "contract field",
                                    prop,
                                    &format!(
                                        "Constructor field on `{base}.new(...)`.\n\n\
                                         | | |\n|---|---|\n| **Contract** | `{base}` |\n| **Type** | `{}` |",
                                        field.ty.display()
                                    ),
                                    None,
                                ),
                                range,
                            });
                        }
                    }
                }
                // Author component call: ComponentName(:prop(...))
                if let Some(comp) = doc.program.components.iter().find(|c| c.name == *base) {
                    if let Some(field) = comp.all_fields().find(|f| f.name == prop) {
                        let kind = if field.is_state { "state" } else { "prop" };
                        return Some(HoverContent {
                            markdown: md(
                                "component prop",
                                prop,
                                &format!(
                                    "Author-declared {kind} on component `{base}`.\n\n\
                                     | | |\n|---|---|\n| **Component** | `{base}` |\n| **Type** | `{}` |",
                                    field.ty.display()
                                ),
                                None,
                            ),
                            range,
                        });
                    }
                    if comp.emits.iter().any(|e| e.name == prop) {
                        return Some(HoverContent {
                            markdown: md(
                                "component emit",
                                prop,
                                &format!(
                                    "Author-declared emit on component `{base}`. Bind it from a parent with `:on({prop}(handler))`."
                                ),
                                None,
                            ),
                            range,
                        });
                    }
                }
                // Stop at first plausible call head that isn't a lowercase keyword-ish token
                if base
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                {
                    break;
                }
            }
            Token::DoubleColon | Token::Game => break,
            _ => {}
        }
    }
    None
}

fn resolve_paren_ident(
    doc: &Document,
    idx: usize,
    name: &str,
    range: HoverRange,
) -> Option<HoverContent> {
    // Pattern: :prop(value)  → tokens Colon, Ident(prop), LParen, Ident(value)
    if idx >= 3
        && matches!(doc.tokens[idx - 1].token, Token::LParen)
        && matches!(&doc.tokens[idx - 2].token, Token::Ident(_))
        && matches!(doc.tokens[idx - 3].token, Token::Colon)
    {
        let Token::Ident(prop) = &doc.tokens[idx - 2].token else {
            return None;
        };

        // :on(event(...)) — event name
        if prop == "on" {
            if let Some(prose) = event_doc("_", name).or_else(|| shared_event_fallback(name)) {
                return Some(HoverContent {
                    markdown: md(
                        "ui event",
                        name,
                        &format!(
                            "{prose}\n\nBind with `:on({name}(handler))` on a UI primitive that declares this event."
                        ),
                        None,
                    ),
                    range,
                });
            }
        }

        // Prefer game closed values when inside a game:: node prop
        if let Some(node) = enclosing_game_node(doc, idx) {
            if let Some(spec) = lookup_game_node(node) {
                if let Some(p) = spec.props.iter().find(|p| p.name == *prop) {
                    if p.closed_values.contains(&name)
                        || game_closed_value_doc(name).is_some()
                    {
                        let prose = game_closed_value_doc(name).unwrap_or(
                            "Closed ident token for this game prop.",
                        );
                        return Some(HoverContent {
                            markdown: md(
                                "game value",
                                name,
                                &format!(
                                    "{prose}\n\n| | |\n|---|---|\n| **Node** | `game::{node}` |\n| **Prop** | `:{prop}` |"
                                ),
                                None,
                            ),
                            range,
                        });
                    }
                }
            }
        }

        if let Some(prose) = ui_closed_value_doc(name) {
            return Some(HoverContent {
                markdown: md(
                    "ui value",
                    name,
                    &format!("{prose}\n\nUsed with `:{prop}({name})` on UI primitives."),
                    None,
                ),
                range,
            });
        }

        if let Some(prose) = game_closed_value_doc(name) {
            return Some(HoverContent {
                markdown: md("game value", name, prose, None),
                range,
            });
        }
    }
    None
}

fn enclosing_game_node<'a>(doc: &'a Document, idx: usize) -> Option<&'a str> {
    let mut i = idx;
    while i > 0 {
        i -= 1;
        if matches!(doc.tokens[i].token, Token::Ident(_))
            && i >= 2
            && matches!(doc.tokens[i - 1].token, Token::DoubleColon)
            && matches!(doc.tokens[i - 2].token, Token::Game)
        {
            if let Token::Ident(node) = &doc.tokens[i].token {
                return Some(node.as_str());
            }
        }
    }
    None
}

fn ui_closed_value_doc(value: &str) -> Option<&'static str> {
    Some(match value {
        "primary" => "Primary action style — strongest call-to-action chrome for buttons and similar controls.",
        "secondary" => "Secondary action style — quieter than primary, for supporting actions.",
        "destructive" => "Destructive action style — reserved for delete/remove confirmations.",
        "ghost" => "Ghost style — minimal chrome for low-emphasis actions.",
        "default" => "Default tone — neutral status chrome.",
        "muted" => "Muted tone — de-emphasized secondary status text.",
        "info" => "Info tone — informational notice chrome.",
        "success" => "Success tone — positive completion / confirmation chrome.",
        "warning" => "Warning tone — cautionary status chrome.",
        "danger" => "Danger tone — error or high-severity status chrome.",
        "sm" => "Small size token for compact controls.",
        "md" => "Medium (default) size token for controls.",
        "lg" => "Large size token for prominent controls.",
        _ => return None,
    })
}

fn shared_event_fallback(event: &str) -> Option<&'static str> {
    event_doc("button", event)
        .or_else(|| event_doc("form", event))
        .or_else(|| event_doc("chat", event))
        .or_else(|| event_doc("table", event))
        .or_else(|| event_doc("alert", event))
        .or_else(|| event_doc("dialog", event))
}

fn resolve_declaration_name(
    doc: &Document,
    name: &str,
    range: HoverRange,
) -> Option<HoverContent> {
    let program = &doc.program;
    if let Some(resource) = program.resources.iter().find(|r| r.name == name) {
        let methods = resource
            .methods
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let contract = resource
            .contract
            .clone()
            .unwrap_or_else(|| "_".into());
        return Some(HoverContent {
            markdown: md(
                "resource",
                name,
                &format!(
                    "Persistent resource bound to a contract. Query and mutation methods \
                     read and write the backing table; when the app is served they also \
                     synthesize matching HTTP routes.\n\n\
                     | | |\n|---|---|\n| **Contract** | `{contract}` |\n| **Table** | `{}` |\n| **Methods** | `{methods}` |",
                    resource.table_name()
                ),
                None,
            ),
            range,
        });
    }
    if let Some(contract) = program.contracts.iter().find(|c| c.name == name) {
        let fields = contract
            .fields
            .iter()
            .map(|f| format!("{}: {}", f.name, f.ty.display()))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(HoverContent {
            markdown: md(
                "contract",
                name,
                &format!(
                    "Data contract / schema. Fields define storage shape, form bindings, and \
                     the values returned from resource queries.\n\n**Fields:** `{fields}`"
                ),
                None,
            ),
            range,
        });
    }
    if let Some(component) = program.components.iter().find(|c| c.name == name) {
        return Some(HoverContent {
            markdown: md(
                "component",
                name,
                &format!(
                    "UI component that renders on web and terminal. It currently declares \
                     {} prop(s), {} state field(s), {} quer(ies), and {} handler(s).\n\n\
                     Props arrive from parents; state is local and mutable; queries are \
                     read-only bindings to resources.",
                    component.props.len(),
                    component.state.len(),
                    component.queries.len(),
                    component.handlers.len()
                ),
                None,
            ),
            range,
        });
    }
    if let Some(subset) = program.subsets.iter().find(|s| s.name == name) {
        return Some(HoverContent {
            markdown: md(
                "subset",
                name,
                &format!(
                    "Semantic subtype of `{}`. Use it wherever the base type is accepted when \
                     you need a named refinement checked at type boundaries.",
                    subset.base.display()
                ),
                None,
            ),
            range,
        });
    }
    if let Some(module) = program.modules.iter().find(|m| m.name == name) {
        return Some(HoverContent {
            markdown: md(
                "module",
                name,
                &format!(
                    "`{}` module. Modules group executable operations and side-effecting \
                     methods outside the UI component tree.",
                    module.kind.as_str()
                ),
                None,
            ),
            range,
        });
    }
    if let Some(app) = program.apps.iter().find(|a| a.name == name) {
        return Some(HoverContent {
            markdown: md(
                "app",
                name,
                &format!(
                    "Application entry that maps URL paths to components. It currently \
                     declares {} route(s); the runtime serves the matching component tree for \
                     both web and terminal surfaces.",
                    app.routes.len()
                ),
                None,
            ),
            range,
        });
    }
    if let Some(game) = program.games.iter().find(|g| g.name == name) {
        return Some(HoverContent {
            markdown: md(
                "game",
                name,
                &format!(
                    "WebGPU game entry rooted at `game::{}`. The compiler lowers the scene \
                     tree to a Babylon runtime; there is no dual-surface terminal path.",
                    game.root.name
                ),
                None,
            ),
            range,
        });
    }
    // Contract field name unique lookup
    for contract in &program.contracts {
        if let Some(f) = contract.fields.iter().find(|f| f.name == name) {
            if f.span.contains_offset(
                // if range matches field span — caller may be on any occurrence
                // Only use when token span equals field span.
                doc.tokens
                    .iter()
                    .find(|t| t.start as usize == range.start_line as usize) // placeholder never
                    .map(|t| t.start)
                    .unwrap_or(u32::MAX),
            ) {
                let _ = f;
            }
            // Prefer field hover only when span matches.
            if f.span.start != 0
                && doc
                    .tokens
                    .iter()
                    .any(|t| t.start == f.span.start && matches!(&t.token, Token::Ident(n) if n == name))
            {
                // Check if current token is this field declaration
                // We don't have idx here; compare via range reconstructed from f.span
                let field_range = HoverRange::from_span(&doc.source, f.span);
                if field_range.start_line == range.start_line
                    && field_range.start_character == range.start_character
                {
                    return Some(HoverContent {
                        markdown: md(
                            "field",
                            name,
                            &format!(
                                "Field on contract `{}`. Its type shapes storage columns, \
                                 form bindings, and member access on values of this contract.\n\n\
                                 | | |\n|---|---|\n| **Type** | `{}` |",
                                contract.name,
                                f.ty.display()
                            ),
                            None,
                        ),
                        range,
                    });
                }
            }
        }
    }
    None
}

fn contract_field_hover(
    program: &Program,
    ty: &TypeExpr,
    field: &str,
    range: HoverRange,
) -> Option<HoverContent> {
    let named = match ty {
        TypeExpr::Named(n) => n.as_str(),
        TypeExpr::Array(inner) => return contract_field_hover(program, inner, field, range),
        TypeExpr::Optional(inner) => return contract_field_hover(program, inner, field, range),
        TypeExpr::Vec { .. } => return None,
    };
    let contract = program.contracts.iter().find(|c| c.name == named)?;
    let f = contract.fields.iter().find(|f| f.name == field)?;
    Some(HoverContent {
        markdown: md(
            "field",
            field,
            &format!(
                "Field on contract `{named}`. Its type shapes storage columns, form \
                 bindings, and member access such as `$row.{field}`.\n\n\
                 | | |\n|---|---|\n| **Type** | `{}` |",
                f.ty.display()
            ),
            None,
        ),
        range,
    })
}

fn synthesized_http(
    resource: &sil_core::Resource,
    method: &str,
    kind: ResourceKind,
) -> Option<String> {
    let table = resource.table_name();
    match (kind, method) {
        (ResourceKind::Query, "list" | "all") => Some(format!("GET /api/{table}")),
        (ResourceKind::Query, "get") => Some(format!("GET /api/{table}/:id")),
        (ResourceKind::Mutation, "create" | "add") => Some(format!("POST /api/{table}")),
        (ResourceKind::Mutation, "update") => Some(format!("PUT /api/{table}/:id")),
        (ResourceKind::Mutation, "delete" | "remove") => Some(format!("DELETE /api/{table}/:id")),
        _ => None,
    }
}

fn enclosing_component<'a>(doc: &'a Document, offset: u32) -> Option<&'a Component> {
    doc.program
        .components
        .iter()
        .filter(|c| c.span.contains_offset(offset) || (c.span.start <= offset && offset <= c.span.end))
        .max_by_key(|c| c.span.start)
}

fn enclosing_handler<'a>(
    component: &'a Component,
    offset: u32,
) -> Option<&'a sil_core::Handler> {
    component
        .handlers
        .iter()
        .filter(|h| h.span.contains_offset(offset) || (h.span.start <= offset && offset <= h.span.end))
        .max_by_key(|h| h.span.start)
}

/// Build a name→type map for the scope containing `offset`.
fn scope_at(doc: &Document, offset: u32) -> std::collections::HashMap<String, TypeExpr> {
    let mut scope = std::collections::HashMap::new();
    let Some(comp) = enclosing_component(doc, offset) else {
        return scope;
    };
    for field in comp.all_fields() {
        scope.insert(field.name.clone(), field.ty.clone());
    }
    for q in &comp.queries {
        let ty = doc
            .program
            .resources
            .iter()
            .find(|r| r.name == q.resource)
            .and_then(|r| r.find_method(&q.method))
            .and_then(|m| m.return_ty.clone())
            .unwrap_or_else(|| TypeExpr::Array(Box::new(TypeExpr::Named("Any".into()))));
        scope.insert(q.name.clone(), ty);
    }
    if let Some(handler) = enclosing_handler(comp, offset) {
        for p in &handler.params {
            if let Some(ty) = &p.ty {
                scope.insert(p.name.clone(), ty.clone());
            }
        }
    }
    // Best-effort for-loop items from render template near offset (type from collection).
    collect_for_bindings(&comp.render, &scope.clone(), &mut scope);
    scope
}

fn collect_for_bindings(
    tmpl: &UiTemplate,
    outer: &std::collections::HashMap<String, TypeExpr>,
    into: &mut std::collections::HashMap<String, TypeExpr>,
) {
    match tmpl {
        UiTemplate::For {
            items,
            item_name,
            body,
        } => {
            let item_ty = expr_elem_type(items, outer)
                .unwrap_or_else(|| TypeExpr::Named("Any".into()));
            into.insert(item_name.clone(), item_ty);
            collect_for_bindings(body, outer, into);
        }
        UiTemplate::When {
            body, else_body, ..
        } => {
            collect_for_bindings(body, outer, into);
            if let Some(e) = else_body {
                collect_for_bindings(e, outer, into);
            }
        }
        UiTemplate::Block(items) => {
            for i in items {
                collect_for_bindings(i, outer, into);
            }
        }
        UiTemplate::Node(node) => {
            for child in &node.children {
                collect_for_bindings(child, outer, into);
            }
            for (_, slot) in &node.slots {
                collect_for_bindings(slot, outer, into);
            }
        }
    }
}

fn expr_elem_type(
    expr: &Expr,
    scope: &std::collections::HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    match expr {
        Expr::Var(name) | Expr::Ident(name) => scope.get(name).and_then(|t| match t {
            TypeExpr::Array(inner) => Some((**inner).clone()),
            other => Some(other.clone()),
        }),
        Expr::Member { base, .. } => expr_elem_type(base, scope),
        _ => None,
    }
}

fn md(kind: &str, name: &str, body: &str, footer: Option<&str>) -> String {
    let mut out = format!("### {kind}: `{name}`\n\n{body}");
    if let Some(f) = footer {
        out.push_str("\n\n");
        out.push_str(f);
    }
    out.push_str("\n\n---\n*Silc 0.4.0*");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    fn doc(src: &str) -> Document {
        Document::open("file://test.silc", 1, src)
    }

    fn hover_on(src: &str, needle: &str) -> HoverContent {
        let offset = src.find(needle).expect("needle") as u32;
        let d = doc(src);
        resolve_hover(&d, offset).unwrap_or_else(|| {
            panic!(
                "no hover for `{needle}` at {offset}; parse_error={:?}",
                d.parse_error
            )
        })
    }

    #[test]
    fn hovers_resource_list_method() {
        let src = r#"
contract Article {
    has UUID $.id;
    has Str $.title;
}
resource Articles for Article {
    query list;
}
component Page {
    query $.articles = Articles.list();
    method render() {
        ui::text(:content("x"))
    }
}
"#;
        let h = hover_on(src, "list()");
        assert!(h.markdown.contains("list"), "{}", h.markdown);
        assert!(h.markdown.contains("[Article]") || h.markdown.contains("Article"), "{}", h.markdown);
        assert!(h.markdown.contains("GET /api/articles"), "{}", h.markdown);
    }

    #[test]
    fn hovers_query_binding() {
        let src = r#"
contract Article { has UUID $.id; has Str $.title; }
resource Articles for Article { query list; }
component Page {
    query $.articles = Articles.list();
    method render() { ui::text(:content("x")) }
}
"#;
        // hover on the binding name in `query $.articles`
        let offset = src.find("$.articles").unwrap() as u32 + 2; // on 'a' of articles
        let d = doc(src);
        let h = resolve_hover(&d, offset).expect("hover");
        assert!(h.markdown.contains("query binding") || h.markdown.contains("articles"), "{}", h.markdown);
        assert!(h.markdown.contains("Articles.list"), "{}", h.markdown);
    }

    #[test]
    fn hovers_ui_primitive() {
        let src = r#"
component Page {
    method render() {
        ui::table(:rows([]), :columns([]))
    }
}
"#;
        let h = hover_on(src, "table");
        assert!(h.markdown.contains("ui primitive"), "{}", h.markdown);
        assert!(
            h.markdown.contains("Renders a collection of records")
                || h.markdown.contains("tabular data"),
            "expected prose description, got:\n{}",
            h.markdown
        );
        // Catalog signature still present, but must not be the only body content.
        assert!(h.markdown.contains("props:"), "{}", h.markdown);
        let body = h.markdown.split("---").next().unwrap_or(&h.markdown);
        assert!(
            body.len() > 120,
            "ui::table hover body too short:\n{body}"
        );
    }

    #[test]
    fn hovers_ui_namespace_qualifier() {
        let src = r#"
component Page {
    method render() {
        ui::table(:rows([]), :columns([]))
    }
}
"#;
        let offset = src.find("ui::table").expect("ui::") as u32;
        let h = resolve_hover(&doc(src), offset).expect("namespace hover");
        assert!(h.markdown.contains("namespace"), "{}", h.markdown);
        assert!(
            h.markdown.contains("UI primitive") || h.markdown.contains("dual-surface"),
            "expected ui namespace prose:\n{}",
            h.markdown
        );
        assert!(
            !h.markdown.contains("ui primitive: `ui::table`"),
            "namespace hover must not be the member hover:\n{}",
            h.markdown
        );
    }

    #[test]
    fn hovers_ui_prop_with_prose() {
        let src = r#"
component Page {
    method render() {
        ui::table(:rows([]), :columns([]), :sortable)
    }
}
"#;
        let h = hover_on(src, "sortable");
        assert!(h.markdown.contains("ui prop"), "{}", h.markdown);
        assert!(
            h.markdown.contains("sort") || h.markdown.contains("column"),
            "{}",
            h.markdown
        );
        assert!(
            !h.markdown.contains("props: `rows`"),
            "prop hover should not dump the full catalog line:\n{}",
            h.markdown
        );
    }

    #[test]
    fn hovers_keyword_query() {
        let src = "resource Articles for Article { query list; }\ncontract Article { has Str $.title; }\n";
        let offset = src.find("query").unwrap() as u32;
        let h = resolve_hover(&doc(src), offset).expect("hover");
        assert!(h.markdown.contains("keyword"), "{}", h.markdown);
        assert!(
            h.markdown.contains("read-only") || h.markdown.contains("resource query"),
            "expected richer keyword prose:\n{}",
            h.markdown
        );
    }

    #[test]
    fn keyword_catalog_is_complete() {
        for kw in crate::docs::KEYWORD_NAMES {
            let doc = keyword_doc(kw).unwrap_or_else(|| panic!("missing doc for {kw}"));
            assert!(
                doc.len() > 60,
                "keyword `{kw}` doc should be at least a sentence or two, got: {doc}"
            );
        }
        for ty in crate::docs::BUILTIN_TYPE_NAMES {
            let doc = builtin_type_doc(ty).unwrap_or_else(|| panic!("missing doc for {ty}"));
            assert!(
                doc.len() > 40,
                "type `{ty}` doc should be at least a sentence or two, got: {doc}"
            );
        }
    }

    #[test]
    fn namespace_catalog_is_complete() {
        for ns in sil_core::KNOWN_NAMESPACES {
            let doc = namespace_doc(ns).unwrap_or_else(|| panic!("missing namespace_doc for {ns}"));
            assert!(
                !doc.trim().is_empty() && doc.len() > 40,
                "namespace `{ns}` doc too short: {doc}"
            );
        }
    }

    #[test]
    fn hovers_game_entity_node() {
        let src = r#"
game Arena {
    game::scene(:title("Test"), :renderer(webgpu),
        game::entity(:name("Player"), :x(0), :y(0), :z(0))
    )
}
"#;
        // Hover on "entity" after "game::"
        let offset = src.find("game::entity").unwrap() + "game::".len();
        let d = doc(src);
        let h = resolve_hover(&d, offset as u32).expect("hover");
        assert!(
            h.markdown.contains("game node") && h.markdown.contains("game::entity"),
            "expected game node hover, got:\n{}",
            h.markdown
        );
        assert!(
            h.markdown.contains("Named node in the scene tree"),
            "expected entity description, got:\n{}",
            h.markdown
        );
    }
}
