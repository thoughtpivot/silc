//! Compile-time acceptance for `ui::embed` (THO-119 / ADR-017).

use sil_parser::parse;

fn validate(src: &str) -> Result<(), String> {
    let program = parse(src).map_err(|e| format!("parse: {}", e.message))?;
    program.validate()
}

const EMBED_APP: &str = r#"
@version("0.6.0")

component GamePane {
    has state Bool $.game_open = true;
    has state Str $.game_url = "http://127.0.0.1:18140/";

    method render() {
        ui::page(
            ui::dialog(
                :open($.game_open),
                :title("Firefly Run"),
                :on(confirm(on_close_game)),
                :on(cancel(on_close_game)),
                ui::embed(
                    :src($.game_url),
                    :title("Firefly Run")
                )
            ),
            ui::embed(
                :src("http://127.0.0.1:18140/"),
                :title("Firefly Run")
            )
        )
    }

    method on_close_game() {
        $.game_open = false;
    }
}

app Journal {
    route "/" => GamePane;
}
"#;

#[test]
fn embed_inside_dialog_and_page_with_state_src_validates() {
    validate(EMBED_APP).expect("ui::embed should validate inside dialog and page");
}

#[test]
fn embed_missing_src_is_error() {
    let src = r#"
@version("0.6.0")
component Pane {
    method render() {
        ui::page(
            ui::embed(:title("Missing src"))
        )
    }
}
app A { route "/" => Pane; }
"#;
    let err = validate(src).expect_err("missing :src must fail");
    assert!(
        err.contains("missing required option `:src`"),
        "unexpected error: {err}"
    );
}

#[test]
fn embed_unknown_option_is_error() {
    let src = r#"
@version("0.6.0")
component Pane {
    method render() {
        ui::page(
            ui::embed(
                :src("http://127.0.0.1:18140/"),
                :html("<b>nope</b>")
            )
        )
    }
}
app A { route "/" => Pane; }
"#;
    let err = validate(src).expect_err("unknown option must fail");
    assert!(
        err.contains("unknown option `:html`"),
        "unexpected error: {err}"
    );
}

#[test]
fn game_and_app_mixing_still_rejected() {
    let src = r#"
@version("0.6.0")
component Pane {
    method render() {
        ui::page(
            ui::embed(:src("http://127.0.0.1:18140/"))
        )
    }
}
app Journal { route "/" => Pane; }
scene FireflyRun {
    scene::scene(:title("Firefly Run"))
}
"#;
    let err = validate(src).expect_err("game+app mix must fail");
    assert!(
        err.contains("cannot mix") && err.contains("game") && err.contains("app"),
        "unexpected error: {err}"
    );
}
