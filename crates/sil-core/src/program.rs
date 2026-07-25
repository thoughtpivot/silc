//! A Silc 0.2.0 program: contracts, modules, components, resources, and apps.

use crate::app::App;
use crate::component::{Component, UiTemplate};
use crate::contract::{Contract, Subset};
use crate::expr::Expr;
use crate::module::Module;
use crate::resource::Resource;
use crate::ui::validate_template;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program {
    pub version: Option<String>,
    pub subsets: Vec<Subset>,
    pub contracts: Vec<Contract>,
    pub modules: Vec<Module>,
    pub components: Vec<Component>,
    pub resources: Vec<Resource>,
    pub apps: Vec<App>,
}

impl Program {
    pub fn all_components(&self) -> impl Iterator<Item = &Component> {
        self.components.iter()
    }

    pub fn find_component(&self, name: &str) -> Option<&Component> {
        self.all_components().find(|c| c.name == name)
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut names = std::collections::HashSet::new();
        for subset in &self.subsets {
            if !names.insert(subset.name.clone()) {
                return Err(format!("duplicate subset name `{}`", subset.name));
            }
        }
        for c in &self.contracts {
            if !names.insert(c.name.clone()) {
                return Err(format!("duplicate contract name `{}`", c.name));
            }
        }
        for m in &self.modules {
            if !names.insert(m.name.clone()) {
                return Err(format!("duplicate module name `{}`", m.name));
            }
            if m.kind == crate::module::ModuleKind::Unknown {
                return Err(format!(
                    "module `{}` missing kind trait (is service|processor|sink)",
                    m.name
                ));
            }
        }
        for component in self.all_components() {
            if !names.insert(component.name.clone()) {
                return Err(format!("duplicate component name `{}`", component.name));
            }
        }
        for resource in &self.resources {
            if !names.insert(resource.name.clone()) {
                return Err(format!("duplicate resource name `{}`", resource.name));
            }
        }
        for app in &self.apps {
            if !names.insert(app.name.clone()) {
                return Err(format!("duplicate app name `{}`", app.name));
            }
        }

        let known_types: std::collections::HashSet<&str> = [
            "Str", "UUID", "num32", "num64", "int32", "int64", "Bool", "Int",
        ]
        .into_iter()
        .chain(self.subsets.iter().map(|subset| subset.name.as_str()))
        .chain(self.contracts.iter().map(|contract| contract.name.as_str()))
        .collect();

        fn validate_type(
            ty: &crate::TypeExpr,
            known_types: &std::collections::HashSet<&str>,
        ) -> Result<(), String> {
            match ty {
                crate::TypeExpr::Named(name) => {
                    if known_types.contains(name.as_str()) {
                        Ok(())
                    } else {
                        Err(format!("unknown type `{name}`"))
                    }
                }
                crate::TypeExpr::Vec { elem, .. } => {
                    if known_types.contains(elem.as_str()) {
                        Ok(())
                    } else {
                        Err(format!("unknown type `{elem}`"))
                    }
                }
                crate::TypeExpr::Array(inner) | crate::TypeExpr::Optional(inner) => {
                    validate_type(inner, known_types)
                }
            }
        }

        for subset in &self.subsets {
            validate_type(&subset.base, &known_types)?;
        }
        for contract in &self.contracts {
            for field in &contract.fields {
                validate_type(&field.ty, &known_types)?;
            }
        }
        for module in &self.modules {
            for field in &module.fields {
                validate_type(&field.ty, &known_types)?;
            }
            for method in &module.methods {
                for param in &method.params {
                    if let Some(ty) = &param.ty {
                        validate_type(ty, &known_types)?;
                    }
                }
            }
        }
        for component in self.all_components() {
            for field in component.all_fields() {
                validate_type(&field.ty, &known_types)?;
            }
            for query in &component.queries {
                let Some(resource) = self.resources.iter().find(|r| r.name == query.resource)
                else {
                    return Err(format!(
                        "component `{}` query `{}` references unknown resource `{}`",
                        component.name, query.name, query.resource
                    ));
                };
                let Some(method) = resource.find_method(&query.method) else {
                    return Err(format!(
                        "component `{}` query `{}` references unknown resource method `{}.{}`",
                        component.name, query.name, query.resource, query.method
                    ));
                };
                if method.kind != crate::ResourceKind::Query {
                    return Err(format!(
                        "component `{}` query `{}` must call a resource query",
                        component.name, query.name
                    ));
                }
                if query.args.len() != method.params.len() {
                    return Err(format!(
                        "component `{}` query `{}` passes {} arguments to `{}.{}`, expected {}",
                        component.name,
                        query.name,
                        query.args.len(),
                        query.resource,
                        query.method,
                        method.params.len()
                    ));
                }
            }
            for emit in &component.emits {
                if let Some(payload) = &emit.payload {
                    if !known_types.contains(payload.as_str()) {
                        return Err(format!(
                            "component `{}` emit `{}` references unknown type `{payload}`",
                            component.name, emit.name
                        ));
                    }
                }
            }
            validate_template(&component.render)?;
            validate_component_template(self, component, &component.render)?;
            for handler in &component.handlers {
                for expr in &handler.body {
                    validate_handler_expr(self, component, expr)?;
                }
            }
        }
        for resource in &self.resources {
            for method in &resource.methods {
                for param in &method.params {
                    if let Some(ty) = &param.ty {
                        validate_type(ty, &known_types)?;
                    }
                }
                if let Some(ty) = &method.return_ty {
                    validate_type(ty, &known_types)?;
                }
            }
        }
        for app in &self.apps {
            if app.routes.is_empty() {
                return Err(format!(
                    "app `{}` must declare at least one route",
                    app.name
                ));
            }
            for route in &app.routes {
                if self.find_component(&route.component).is_none() {
                    return Err(format!(
                        "app `{}` route `{}` references unknown component `{}`",
                        app.name, route.path, route.component
                    ));
                }
            }
            if app.serve.is_none() {
                return Err(format!("app `{}` must declare `method serve()`", app.name));
            }
        }

        let _graph = crate::operation::infer_graph(self)?;
        Ok(())
    }
}

fn validate_component_template(
    program: &Program,
    owner: &Component,
    template: &UiTemplate,
) -> Result<(), String> {
    match template {
        UiTemplate::Node(node) => {
            if crate::ui::lookup_component(&node.component).is_none() {
                let target = program.find_component(&node.component).ok_or_else(|| {
                    format!(
                        "component `{}` renders unknown component `{}`",
                        owner.name, node.component
                    )
                })?;
                for prop in &target.props {
                    if prop.default.is_none() && node.prop(&prop.name).is_none() {
                        return Err(format!(
                            "component `{}` invocation in `{}` is missing required prop `:{}`",
                            target.name, owner.name, prop.name
                        ));
                    }
                }
                for (name, _) in &node.props {
                    if !target.props.iter().any(|prop| prop.name == *name) {
                        return Err(format!(
                            "unknown prop `:{name}` on component `{}`",
                            target.name
                        ));
                    }
                }
                for (name, _) in &node.slots {
                    if !target.slots.iter().any(|slot| slot.name == *name) {
                        return Err(format!(
                            "unknown slot `:{name}` on component `{}`",
                            target.name
                        ));
                    }
                }
                for binding in &node.events {
                    if !target.emits.iter().any(|emit| emit.name == binding.event) {
                        return Err(format!(
                            "component `{}` does not emit `{}`",
                            target.name, binding.event
                        ));
                    }
                }
            }
            for binding in &node.events {
                let handler_exists = owner
                    .handlers
                    .iter()
                    .any(|handler| handler.name == binding.handler)
                    || owner
                        .methods
                        .iter()
                        .any(|method| method.name == binding.handler);
                if !handler_exists {
                    return Err(format!(
                        "component `{}` binds event `{}` to unknown handler `{}`",
                        owner.name, binding.event, binding.handler
                    ));
                }
            }
            for child in &node.children {
                validate_component_template(program, owner, child)?;
            }
            for (_, slot) in &node.slots {
                validate_component_template(program, owner, slot)?;
            }
        }
        UiTemplate::When {
            body, else_body, ..
        } => {
            validate_component_template(program, owner, body)?;
            if let Some(else_body) = else_body {
                validate_component_template(program, owner, else_body)?;
            }
        }
        UiTemplate::For { body, .. } => validate_component_template(program, owner, body)?,
        UiTemplate::Block(items) => {
            for item in items {
                validate_component_template(program, owner, item)?;
            }
        }
    }
    Ok(())
}

fn validate_handler_expr(
    program: &Program,
    component: &Component,
    expr: &Expr,
) -> Result<(), String> {
    match expr {
        Expr::Assign { target, value } => {
            if let Expr::Var(name) = target.as_ref() {
                if !component.state.iter().any(|field| field.name == *name) {
                    return Err(format!(
                        "component `{}` may only assign reactive state; `{name}` is not state",
                        component.name
                    ));
                }
            }
            validate_handler_expr(program, component, value)?;
        }
        Expr::Emit { event, payload } => {
            if !component.emits.iter().any(|emit| emit.name == *event) {
                return Err(format!(
                    "component `{}` emits undeclared event `{event}`",
                    component.name
                ));
            }
            if let Some(payload) = payload {
                validate_handler_expr(program, component, payload)?;
            }
        }
        Expr::Call { callee, args } => {
            if let Expr::Member { base, field } = callee.as_ref() {
                if let Expr::Ident(resource_name) = base.as_ref() {
                    if let Some(resource) =
                        program.resources.iter().find(|r| r.name == *resource_name)
                    {
                        let method = resource.find_method(field).ok_or_else(|| {
                            format!(
                                "component `{}` calls unknown resource method `{}.{field}`",
                                component.name, resource_name
                            )
                        })?;
                        if args.len() != method.params.len() {
                            return Err(format!(
                                "resource call `{}.{field}` passes {} arguments, expected {}",
                                resource_name,
                                args.len(),
                                method.params.len()
                            ));
                        }
                    }
                }
            }
            for arg in args {
                validate_handler_expr(program, component, arg)?;
            }
        }
        Expr::BinOp { left, right, .. } => {
            validate_handler_expr(program, component, left)?;
            validate_handler_expr(program, component, right)?;
        }
        Expr::Unary { expr, .. } | Expr::Await(expr) => {
            validate_handler_expr(program, component, expr)?;
        }
        Expr::List(items) => {
            for item in items {
                validate_handler_expr(program, component, item)?;
            }
        }
        Expr::New { fields, .. } => {
            for (_, value) in fields {
                validate_handler_expr(program, component, value)?;
            }
        }
        Expr::Member { base, .. } => validate_handler_expr(program, component, base)?,
        Expr::Interpolated(parts) => {
            for part in parts {
                if let crate::InterpPart::Expr(expr) = part {
                    validate_handler_expr(program, component, expr)?;
                }
            }
        }
        Expr::String(_)
        | Expr::Number(_)
        | Expr::Bool(_)
        | Expr::Ident(_)
        | Expr::Var(_)
        | Expr::Navigate { .. } => {}
    }
    Ok(())
}
