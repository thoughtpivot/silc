//! Closed catalogs for `scrape::*` props (ADR-006).

/// Allowed `:js(...)` values on `scrape::page` / `scrape::site`.
pub const JS_MODES: &[&str] = &["false", "auto", "true"];

/// Default `:js` when omitted on shipped ops that accept it.
pub const DEFAULT_JS_MODE: &str = "auto";

/// Default crawl depth when `:depth` is omitted on `scrape::site`.
pub const DEFAULT_SITE_DEPTH: u32 = 2;

/// Maximum author-facing crawl depth.
pub const MAX_SITE_DEPTH: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsMode {
    False,
    Auto,
    True,
}

impl JsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            JsMode::False => "false",
            JsMode::Auto => "auto",
            JsMode::True => "true",
        }
    }

    pub fn needs_browser(self) -> bool {
        matches!(self, JsMode::True | JsMode::Auto)
    }
}

pub fn parse_js_mode(raw: &str) -> Result<JsMode, String> {
    let v = raw.trim().trim_matches('"').trim_matches('\'').to_ascii_lowercase();
    match v.as_str() {
        "false" | "0" | "no" | "off" => Ok(JsMode::False),
        "auto" => Ok(JsMode::Auto),
        "true" | "1" | "yes" | "on" => Ok(JsMode::True),
        _ => Err(format!(
            "invalid scrape :js(`{raw}`); expected one of: {}",
            JS_MODES.join(", ")
        )),
    }
}

pub fn parse_site_depth(raw: &str) -> Result<u32, String> {
    let v = raw.trim().trim_matches('"').trim_matches('\'');
    let depth: u32 = v
        .parse()
        .map_err(|_| format!("invalid scrape :depth(`{raw}`); expected integer 1..{MAX_SITE_DEPTH}"))?;
    if depth == 0 || depth > MAX_SITE_DEPTH {
        return Err(format!(
            "scrape :depth must be between 1 and {MAX_SITE_DEPTH} (got {depth})"
        ));
    }
    Ok(depth)
}

pub fn parse_same_host(raw: &str) -> Result<bool, String> {
    let v = raw.trim().trim_matches('"').trim_matches('\'').to_ascii_lowercase();
    match v.as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(format!(
            "invalid scrape :same_host(`{raw}`); expected true or false"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_js_modes() {
        assert_eq!(parse_js_mode("auto").unwrap(), JsMode::Auto);
        assert_eq!(parse_js_mode("false").unwrap(), JsMode::False);
        assert!(parse_js_mode("maybe").is_err());
    }

    #[test]
    fn parses_depth_bounds() {
        assert_eq!(parse_site_depth("2").unwrap(), 2);
        assert_eq!(parse_site_depth("10").unwrap(), 10);
        assert!(parse_site_depth("0").is_err());
        assert!(parse_site_depth("11").is_err());
    }
}
