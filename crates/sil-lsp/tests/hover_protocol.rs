//! Framed JSON-RPC smoke test against the sil-lsp binary.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

fn read_message(stdout: &mut impl Read) -> serde_json::Value {
    let mut header = Vec::new();
    let mut buf = [0u8; 1];
    loop {
        stdout.read_exact(&mut buf).expect("read header byte");
        header.push(buf[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
        if header.len() > 4096 {
            panic!("header too large");
        }
    }
    let header = String::from_utf8(header).expect("utf8 header");
    let len = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .expect("Content-Length")
        .trim()
        .parse::<usize>()
        .expect("parse len");
    let mut body = vec![0u8; len];
    stdout.read_exact(&mut body).expect("read body");
    serde_json::from_slice(&body).expect("json body")
}

fn write_message(stdin: &mut impl Write, value: &serde_json::Value) {
    let body = serde_json::to_vec(value).unwrap();
    write!(stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    stdin.write_all(&body).unwrap();
    stdin.flush().unwrap();
}

#[test]
fn hover_list_over_stdio() {
    let bin = env!("CARGO_BIN_EXE_sil-lsp");
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sil-lsp");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    write_message(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "capabilities": {},
                "clientInfo": { "name": "hover_protocol" },
                "rootUri": null
            }
        }),
    );
    let init = read_message(&mut stdout);
    assert_eq!(init["id"], 1);
    assert!(init["result"]["capabilities"]["hoverProvider"].as_bool().unwrap_or(false));

    write_message(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    );

    let src = r#"contract Article { has UUID $.id; has Str $.title; }
resource Articles for Article { query list; }
component Page {
    query $.articles = Articles.list();
    method render() { ui::text(:content("x")) }
}
"#;
    write_message(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///smoke.silc",
                    "languageId": "silc",
                    "version": 1,
                    "text": src
                }
            }
        }),
    );

    let list_offset = src.find("Articles.list()").unwrap() + "Articles.".len();
    let (line, character) = sil_core::offset_to_lsp(src, list_offset);

    write_message(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///smoke.silc" },
                "position": { "line": line, "character": character }
            }
        }),
    );

    // Give the server a moment if needed (usually immediate).
    std::thread::sleep(Duration::from_millis(20));
    let hover = read_message(&mut stdout);
    assert_eq!(hover["id"], 2);
    let value = hover["result"]["contents"]["value"]
        .as_str()
        .expect("markdown value");
    assert!(value.contains("list"), "{value}");
    assert!(
        value.contains("Article") || value.contains("[Article]"),
        "{value}"
    );

    write_message(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "shutdown",
            "params": null
        }),
    );
    let _ = read_message(&mut stdout);
    write_message(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "exit",
            "params": null
        }),
    );
    drop(stdin);
    drop(stdout);
    let status = child.wait().expect("wait sil-lsp");
    assert!(status.success(), "sil-lsp exited with {status}");
}
