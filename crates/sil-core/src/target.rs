//! Target subject: resolved Go, Python, or Bun execution assignment.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Go,
    Python,
    Bun,
}

impl Target {
    pub fn as_str(self) -> &'static str {
        match self {
            Target::Go => "go",
            Target::Python => "python",
            Target::Bun => "bun",
        }
    }

    pub fn runtime_dir(self) -> &'static str {
        match self {
            Target::Go => "go",
            Target::Python => "python",
            Target::Bun => "typescript",
        }
    }
}
