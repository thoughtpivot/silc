//! Compile-time acceptance for `files "<dir>";` + `ui::file_browser` (ADR-018).

use sil_parser::parse;

fn validate(src: &str) -> Result<(), String> {
    let program = parse(src).map_err(|e| format!("parse: {}", e.message))?;
    program.validate()
}

const FILES_APP: &str = r#"
@version("0.6.0")

component FilesPage {
    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Files"))),
            ui::section(
                :title("Library"),
                ui::file_browser(:title("File library"), :empty_text("Nothing here yet."))
            )
        )
    }
}

app Board {
    route "/" => FilesPage;
    files "./files";
}
"#;

#[test]
fn files_directive_and_file_browser_validate() {
    let program = parse(FILES_APP).expect("parse");
    let app = &program.apps[0];
    assert_eq!(app.files.as_ref().map(|f| f.path.as_str()), Some("./files"));
    assert_eq!(app.routes.len(), 1);
    program.validate().expect("files app should validate");
}

#[test]
fn files_directive_may_precede_routes() {
    let src = r#"
@version("0.6.0")
component Home { method render() { ui::page(ui::file_browser()) } }
app Board {
    files "shared";
    route "/" => Home;
}
"#;
    validate(src).expect("files before route should validate");
}

#[test]
fn duplicate_files_directive_is_error() {
    let src = r#"
@version("0.6.0")
component Home { method render() { ui::page(ui::text(:text("hi"))) } }
app Board {
    route "/" => Home;
    files "./a";
    files "./b";
}
"#;
    let err = validate(src).unwrap_err();
    assert!(err.contains("only once"), "got: {err}");
}

#[test]
fn files_without_string_is_error() {
    let src = r#"
@version("0.6.0")
component Home { method render() { ui::page(ui::text(:text("hi"))) } }
app Board {
    route "/" => Home;
    files;
}
"#;
    let err = validate(src).unwrap_err();
    assert!(err.contains("directory string"), "got: {err}");
}

#[test]
fn files_with_parent_segments_is_error() {
    let src = r#"
@version("0.6.0")
component Home { method render() { ui::page(ui::text(:text("hi"))) } }
app Board {
    route "/" => Home;
    files "../outside";
}
"#;
    let err = validate(src).unwrap_err();
    assert!(err.contains("must not contain `..`"), "got: {err}");
}

#[test]
fn file_browser_without_files_directive_is_error() {
    let src = r#"
@version("0.6.0")
component Home { method render() { ui::page(ui::file_browser()) } }
app Board {
    route "/" => Home;
}
"#;
    let err = validate(src).unwrap_err();
    assert!(err.contains("`ui::file_browser` requires"), "got: {err}");
    assert!(err.contains("files \"./files\";"), "got: {err}");
}

#[test]
fn file_browser_rejects_unknown_option() {
    let src = r#"
@version("0.6.0")
component Home { method render() { ui::page(ui::file_browser(:directory("./x"))) } }
app Board {
    route "/" => Home;
    files "./x";
}
"#;
    let err = validate(src).unwrap_err();
    assert!(err.contains("unknown option `:directory`"), "got: {err}");
}

#[test]
fn files_remains_an_ordinary_identifier_elsewhere() {
    // `files` is matched by name only inside the app body, so state fields,
    // handles, and options named `files` keep working.
    let src = r#"
@version("0.6.0")
component Home {
    has state Str $.files = "";
    method render() {
        ui::page(
            ui::form(:on(submit(on_submit)),
                ui::text_input(:field(files), :label("Files")),
                ui::button(:label("Go"), :submit)
            )
        )
    }
    method on_submit() { $.files = ""; }
}
app Board {
    route "/" => Home;
}
"#;
    validate(src).expect("`files` identifier should still be allowed");
}
