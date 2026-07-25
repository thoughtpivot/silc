//! Application subject: routes and dual-surface serve entry.

use crate::module::Method;
use crate::types::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub path: String,
    pub component: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub name: String,
    pub routes: Vec<Route>,
    pub serve: Option<Method>,
    pub span: Span,
}

impl App {
    pub fn default_route(&self) -> Option<&Route> {
        self.routes
            .iter()
            .find(|r| r.path == "/")
            .or_else(|| self.routes.first())
    }
}
