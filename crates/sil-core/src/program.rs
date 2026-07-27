//! A Silc 0.4.0 program: contracts, modules, components, resources, and apps.

use crate::app::App;
use crate::component::{Component, UiTemplate};
use crate::contract::{Contract, Subset, SubsetPredicate};
use crate::expr::Expr;
use crate::module::Module;
use crate::resource::Resource;
use crate::types::TypeExpr;
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
    pub fn validate_source_version(&self, expected: &str) -> Result<(), String> {
        match self.version.as_deref() {
            Some(actual) if actual == expected => Ok(()),
            Some(actual) => Err(format!(
                "source declares Silc {actual}; migrate to `@version(\"{expected}\")`"
            )),
            None => Err(format!(
                "source is missing a version; add `@version(\"{expected}\")`"
            )),
        }
    }

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
                return Err(format!("module `{}` has an unknown subject kind", m.name));
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
            if subset.predicate.is_some() && !resolves_to_str(self, &subset.base) {
                return Err(format!(
                    "subset `{}` has a string where predicate but base type does not resolve to `Str`",
                    subset.name
                ));
            }
        }
        for contract in &self.contracts {
            for field in &contract.fields {
                validate_type(&field.ty, &known_types)?;
                if let Some(default) = &field.default {
                    if let Some(lit) = string_literal_from_default(default) {
                        check_subset_string(
                            self,
                            &field.ty,
                            &lit,
                            &format!(
                                "contract `{}` field `{}` default",
                                contract.name, field.name
                            ),
                        )?;
                    }
                }
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
                if let Some(Expr::String(lit)) = &field.default {
                    check_subset_string(
                        self,
                        &field.ty,
                        lit,
                        &format!(
                            "component `{}` field `{}` default",
                            component.name, field.name
                        ),
                    )?;
                }
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
        }

        for resource in &self.resources {
            let Some(contract) = &resource.contract else {
                return Err(format!(
                    "resource `{}` must declare `for Contract`",
                    resource.name
                ));
            };
            if self.contracts.iter().all(|c| c.name != *contract) {
                return Err(format!(
                    "resource `{}` references unknown contract `{contract}`",
                    resource.name
                ));
            }
            if resource.methods.iter().any(|m| m.shorthand) {
                return Err(format!(
                    "resource `{}` has unresolved capability declarations",
                    resource.name
                ));
            }
            let contract_def = self
                .contracts
                .iter()
                .find(|c| c.name == *contract)
                .expect("contract validated above");
            let mut seen_seed_ids = std::collections::HashSet::new();
            for (index, seed) in resource.seeds.iter().enumerate() {
                if seed.contract != *contract {
                    return Err(format!(
                        "resource `{}` seed #{} constructs `{}`, expected `{contract}`",
                        resource.name,
                        index + 1,
                        seed.contract
                    ));
                }
                let mut has_id = false;
                for (field_name, value) in &seed.fields {
                    let Some(field) = contract_def.fields.iter().find(|f| f.name == *field_name)
                    else {
                        return Err(format!(
                            "resource `{}` seed #{} has unknown field `{field_name}` for contract `{contract}`",
                            resource.name,
                            index + 1
                        ));
                    };
                    if field_name == "id" {
                        has_id = true;
                        let Some(id) = value.as_string_literal() else {
                            return Err(format!(
                                "resource `{}` seed #{} field `id` must be a string literal",
                                resource.name,
                                index + 1
                            ));
                        };
                        if id.is_empty() {
                            return Err(format!(
                                "resource `{}` seed #{} field `id` must be non-empty",
                                resource.name,
                                index + 1
                            ));
                        }
                        if !seen_seed_ids.insert(id.to_string()) {
                            return Err(format!(
                                "resource `{}` has duplicate seed id `{id}`",
                                resource.name
                            ));
                        }
                    }
                    if let Some(lit) = value.as_string_literal() {
                        check_subset_string(
                            self,
                            &field.ty,
                            lit,
                            &format!(
                                "resource `{}` seed #{} field `{field_name}`",
                                resource.name,
                                index + 1
                            ),
                        )?;
                    }
                }
                if !has_id {
                    return Err(format!(
                        "resource `{}` seed #{} must include a stable `:id(\"…\")` for idempotent INSERT OR IGNORE",
                        resource.name,
                        index + 1
                    ));
                }
            }
        }

        reject_author_runtime_mechanics(self)?;

        let _graph = crate::operation::infer_graph(self)?;
        Ok(())
    }
}

fn reject_author_runtime_mechanics(program: &Program) -> Result<(), String> {
    let mut forbidden = Vec::new();
    let mut note = |ns: &str, name: &str| {
        let key = format!("{ns}::{name}");
        if !forbidden.contains(&key) {
            forbidden.push(key);
        }
    };
    crate::operation::scan_author_calls(program, |ns, name| match (ns, name) {
        ("ui", "web") | ("ui", "terminal") => note(ns, name),
        ("ipc", _) => note(ns, name),
        ("store", _) => note(ns, name),
        ("resource", _) => note(ns, name),
        _ => {}
    });
    if forbidden.is_empty() {
        return Ok(());
    }
    Err(format!(
        "runtime mechanics are compiler-owned in Silc 0.4.0 and must not appear in source ({}); remove `method serve()`, `sink`, `ipc::*`, `store::*`, and `resource::*` pipelines — declare `app` routes, `resource Name for Contract {{ query/mutation; }}`, and processor workflows only",
        forbidden.join(", ")
    ))
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

fn resolves_to_str(program: &Program, ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Named(name) if name == "Str" => true,
        TypeExpr::Named(name) => program
            .subsets
            .iter()
            .find(|s| s.name == *name)
            .is_some_and(|s| resolves_to_str(program, &s.base)),
        TypeExpr::Optional(inner) => resolves_to_str(program, inner),
        _ => false,
    }
}

fn subset_predicate_for_type<'a>(
    program: &'a Program,
    ty: &TypeExpr,
) -> Option<(&'a str, &'a SubsetPredicate)> {
    match ty {
        TypeExpr::Named(name) => {
            let subset = program.subsets.iter().find(|s| s.name == *name)?;
            subset.predicate.as_ref().map(|p| (subset.name.as_str(), p))
        }
        TypeExpr::Optional(inner) => subset_predicate_for_type(program, inner),
        _ => None,
    }
}

fn string_literal_from_default(default: &str) -> Option<String> {
    let trimmed = default.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        Some(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        None
    }
}

fn check_subset_string(
    program: &Program,
    ty: &TypeExpr,
    value: &str,
    context: &str,
) -> Result<(), String> {
    if let Some((subset_name, pred)) = subset_predicate_for_type(program, ty) {
        if !pred.check_str(value) {
            return Err(format!(
                "{context}: value `{value}` does not satisfy subset `{subset_name}`"
            ));
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
                if let Expr::String(lit) = value.as_ref() {
                    if let Some(field) = component.state.iter().find(|f| f.name == *name) {
                        check_subset_string(
                            program,
                            &field.ty,
                            lit,
                            &format!("component `{}` assignment to `{name}`", component.name),
                        )?;
                    }
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
        Expr::New { ty, fields } => {
            if let Some(contract) = program.contracts.iter().find(|c| c.name == *ty) {
                for (fname, value) in fields {
                    if let Some(field) = contract.fields.iter().find(|f| f.name == *fname) {
                        if let Expr::String(lit) = value {
                            check_subset_string(
                                program,
                                &field.ty,
                                lit,
                                &format!("{ty}.new field `{fname}`"),
                            )?;
                        }
                    }
                    validate_handler_expr(program, component, value)?;
                }
            } else {
                for (_, value) in fields {
                    validate_handler_expr(program, component, value)?;
                }
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
