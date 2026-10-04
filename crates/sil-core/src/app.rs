//! Application subject: routes. Serving is synthesized; authors do not declare it.

use crate::types::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub path: String,
    pub component: String,
    pub span: Span,
}

/// `files "<dir>";` — one sysop-shared directory the synthesized runtime
/// lists and serves (ADR-018). The path is relative to the entry file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesDecl {
    pub path: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub name: String,
    pub routes: Vec<Route>,
    pub files: Option<FilesDecl>,
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
