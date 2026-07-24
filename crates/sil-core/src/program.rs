//! A Silc program: subsets, contracts, and modules.

use crate::contract::{Contract, Subset};
use crate::module::Module;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Program {
    pub version: Option<String>,
    pub subsets: Vec<Subset>,
    pub contracts: Vec<Contract>,
    pub modules: Vec<Module>,
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
        crate::operation::infer_graph(self)?;
        Ok(())
    }
}
