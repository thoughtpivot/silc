//! A Silc program: subsets, contracts, modules, and declarative UI views.

use crate::contract::{Contract, Subset};
use crate::module::Module;
use crate::ui::{validate_view, UiView};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Program {
    pub version: Option<String>,
    pub subsets: Vec<Subset>,
    pub contracts: Vec<Contract>,
    pub modules: Vec<Module>,
    pub views: Vec<UiView>,
}

impl Program {
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
        for view in &self.views {
            if !names.insert(view.name.clone()) {
                return Err(format!("duplicate view name `{}`", view.name));
            }
        }

        let known_types: std::collections::HashSet<&str> =
            ["Str", "UUID", "num32", "num64", "int32", "int64", "Bool"]
                .into_iter()
                .chain(self.subsets.iter().map(|subset| subset.name.as_str()))
                .chain(self.contracts.iter().map(|contract| contract.name.as_str()))
                .collect();
        let validate_type = |ty: &crate::TypeExpr| -> Result<(), String> {
            let name = ty.name();
            if known_types.contains(name) {
                Ok(())
            } else {
                Err(format!("unknown type `{name}`"))
            }
        };
        for subset in &self.subsets {
            validate_type(&subset.base)?;
        }
        for contract in &self.contracts {
            for field in &contract.fields {
                validate_type(&field.ty)?;
            }
        }
        for module in &self.modules {
            for field in &module.fields {
                validate_type(&field.ty)?;
            }
            for method in &module.methods {
                for param in &method.params {
                    if let Some(ty) = &param.ty {
                        validate_type(ty)?;
                    }
                }
            }
        }
        // Runnable-program validation (no-op for stub programs).
        let graph = crate::operation::infer_graph(self)?;
        if let Some(graph) = &graph {
            if let Some(view) = &graph.ui_view {
                let contract = self
                    .contracts
                    .iter()
                    .find(|c| Some(c.name.as_str()) == graph.ui_contract.as_deref());
                validate_view(view, contract)?;
                let uses_chat = view.root.contains_component("chat")
                    || view.root.contains_component("chat_history");
                if uses_chat && graph.portal_kind != crate::operation::PortalKind::LlmChat {
                    return Err(format!(
                        "view `{}` uses `ui::chat`/`ui::chat_history`, which require an `llm::complete` portal",
                        view.name
                    ));
                }
            }
        }
        // Orphan views (not referenced by ui::web) are still catalog-checked.
        for view in &self.views {
            let referenced = graph
                .as_ref()
                .and_then(|g| g.ui_view.as_ref())
                .map(|v| v.name == view.name)
                .unwrap_or(false);
            if !referenced {
                validate_view(view, None)?;
            }
        }
        Ok(())
    }
}
