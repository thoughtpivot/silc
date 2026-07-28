//! Stdio LSP loop for Silc hover support.

use std::collections::HashMap;
use std::error::Error;

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
};
use lsp_types::request::{HoverRequest, Initialize, Request as _, Shutdown};
use lsp_types::{
    Hover, HoverContents, HoverProviderCapability, InitializeResult, MarkupContent, MarkupKind,
    Position, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use sil_ide::{hover_at_lsp, Document};
use serde_json::Value;

pub fn run() -> Result<(), Box<dyn Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start()?;
    let _params: Value = serde_json::from_value(params)?;

    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        ..ServerCapabilities::default()
    };
    let result = InitializeResult {
        capabilities,
        server_info: Some(lsp_types::ServerInfo {
            name: "sil-lsp".into(),
            version: Some(env!("CARGO_PKG_VERSION").into()),
        }),
    };
    connection.initialize_finish(id, serde_json::to_value(result)?)?;

    let mut state = ServerState::default();
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                // Acknowledge shutdown but keep the loop alive until `exit`.
                if connection.handle_shutdown(&req)? {
                    continue;
                }
                if let Err(err) = handle_request(&connection, &mut state, req) {
                    eprintln!("sil-lsp request error: {err}");
                }
            }
            Message::Notification(not) => {
                if not.method == "exit" {
                    // Drop the connection and exit promptly; joining IO threads can block
                    // on a still-open stdio pipe after the protocol is finished.
                    drop(connection);
                    let _ = io_threads.join();
                    return Ok(());
                }
                if let Err(err) = handle_notification(&mut state, not) {
                    eprintln!("sil-lsp notification error: {err}");
                }
            }
            Message::Response(_) => {}
        }
    }

    drop(connection);
    let _ = io_threads.join();
    Ok(())
}

#[derive(Default)]
struct ServerState {
    documents: HashMap<String, Document>,
}

fn uri_key(uri: &Uri) -> String {
    // lsp-types 0.97 Uri Display/as_str can differ; normalize to as_str.
    uri.as_str().to_string()
}

fn handle_notification(
    state: &mut ServerState,
    not: Notification,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    match not.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: lsp_types::DidOpenTextDocumentParams = serde_json::from_value(not.params)?;
            let uri = uri_key(&params.text_document.uri);
            let doc = Document::open(
                uri.clone(),
                params.text_document.version,
                params.text_document.text,
            );
            state.documents.insert(uri, doc);
        }
        DidChangeTextDocument::METHOD => {
            let params: lsp_types::DidChangeTextDocumentParams =
                serde_json::from_value(not.params)?;
            let uri = uri_key(&params.text_document.uri);
            if let Some(change) = params.content_changes.into_iter().last() {
                if let Some(doc) = state.documents.get_mut(&uri) {
                    doc.update(params.text_document.version, change.text);
                } else {
                    state.documents.insert(
                        uri.clone(),
                        Document::open(uri, params.text_document.version, change.text),
                    );
                }
            }
        }
        DidCloseTextDocument::METHOD => {
            let params: lsp_types::DidCloseTextDocumentParams = serde_json::from_value(not.params)?;
            state.documents.remove(&uri_key(&params.text_document.uri));
        }
        _ => {}
    }
    Ok(())
}

fn handle_request(
    connection: &Connection,
    state: &mut ServerState,
    req: Request,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    match req.method.as_str() {
        Initialize::METHOD => {
            // Already handled during initialize handshake.
            let resp = Response::new_ok(req.id, Value::Null);
            connection.sender.send(Message::Response(resp))?;
        }
        Shutdown::METHOD => {
            let resp = Response::new_ok(req.id, Value::Null);
            connection.sender.send(Message::Response(resp))?;
        }
        HoverRequest::METHOD => {
            let id = req.id.clone();
            match cast_request::<HoverRequest>(req) {
                Ok((_id, params)) => {
                    let pos = params.text_document_position_params;
                    let hover = compute_hover(state, &pos.text_document.uri, pos.position);
                    let resp = Response::new_ok(id, hover);
                    connection.sender.send(Message::Response(resp))?;
                }
                Err(err) => {
                    let resp = Response::new_err(
                        id,
                        lsp_server::ErrorCode::InvalidParams as i32,
                        err.to_string(),
                    );
                    connection.sender.send(Message::Response(resp))?;
                }
            }
        }
        _ => {
            let resp = Response::new_err(
                req.id,
                lsp_server::ErrorCode::MethodNotFound as i32,
                format!("unsupported method {}", req.method),
            );
            connection.sender.send(Message::Response(resp))?;
        }
    }
    Ok(())
}

fn compute_hover(
    state: &ServerState,
    uri: &Uri,
    position: Position,
) -> Option<Hover> {
    let doc = state.documents.get(&uri_key(uri))?;
    let content = hover_at_lsp(doc, position.line, position.character)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content.markdown,
        }),
        range: Some(lsp_types::Range {
            start: Position {
                line: content.range.start_line,
                character: content.range.start_character,
            },
            end: Position {
                line: content.range.end_line,
                character: content.range.end_character,
            },
        }),
    })
}

fn cast_request<R>(req: Request) -> Result<(RequestId, R::Params), Box<dyn Error + Sync + Send>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    let params = serde_json::from_value(req.params)?;
    Ok((req.id, params))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sil_ide::Document;

    #[test]
    fn hover_from_document_state() {
        let src = r#"
contract Article { has UUID $.id; has Str $.title; }
resource Articles for Article { query list; }
component Page {
    query $.articles = Articles.list();
    method render() { ui::text(:content("hi")) }
}
"#;
        let mut state = ServerState::default();
        let uri = "file:///test.silc".to_string();
        state
            .documents
            .insert(uri.clone(), Document::open(uri.clone(), 1, src));
        let offset = src.find("list()").unwrap();
        // Convert offset to line/col
        let (line, character) = sil_core::offset_to_lsp(src, offset);
        let url: Uri = uri.parse().unwrap();
        let hover = compute_hover(
            &state,
            &url,
            Position {
                line,
                character,
            },
        )
        .expect("hover");
        let HoverContents::Markup(m) = hover.contents else {
            panic!("expected markup");
        };
        assert!(m.value.contains("list"), "{}", m.value);
        assert!(m.value.contains("[Article]") || m.value.contains("Article"), "{}", m.value);
    }
}
