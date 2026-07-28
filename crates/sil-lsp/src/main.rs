//! Silc Language Server — stdio LSP (hover).

mod server;

fn main() {
    if let Err(err) = server::run() {
        eprintln!("sil-lsp error: {err}");
        std::process::exit(1);
    }
}
