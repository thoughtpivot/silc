//! Integration hovers against the blogApp example.

use sil_core::offset_to_lsp;
use sil_ide::{hover_at_lsp, resolve_hover, Document};

fn blog_source() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/blogApp/main.silc");
    std::fs::read_to_string(path).expect("read blogApp/main.silc")
}

fn hover_at_needle(src: &str, needle: &str) -> String {
    let offset = src
        .find(needle)
        .unwrap_or_else(|| panic!("needle not found: {needle}")) as u32;
    let doc = Document::open("file://blog.silc", 1, src);
    let hover = resolve_hover(&doc, offset)
        .unwrap_or_else(|| panic!("no hover for `{needle}` (parse_error={:?})", doc.parse_error));
    hover.markdown
}

#[test]
fn blog_articles_list_method_hover() {
    let src = blog_source();
    // First occurrence of Articles.list() in a query binding.
    let md = hover_at_needle(&src, "Articles.list()");
    // Cursor on list — find the list token inside that occurrence.
    let list_offset = (src.find("Articles.list()").unwrap() + "Articles.".len()) as u32;
    let doc = Document::open("file://blog.silc", 1, &src);
    let hover = resolve_hover(&doc, list_offset).expect("list hover");
    assert!(
        hover.markdown.contains("list")
            && (hover.markdown.contains("[Article]") || hover.markdown.contains("Article")),
        "{}",
        hover.markdown
    );
    assert!(
        hover.markdown.contains("GET /api/articles") || hover.markdown.contains("Articles"),
        "{}",
        hover.markdown
    );
    let _ = md;
}

#[test]
fn blog_ui_table_hover() {
    let src = blog_source();
    let offset = src.find("ui::table").and_then(|i| Some(i + "ui::".len())).expect("ui::table") as u32;
    let doc = Document::open("file://blog.silc", 1, &src);
    let hover = resolve_hover(&doc, offset).expect("table hover");
    assert!(hover.markdown.contains("ui primitive"), "{}", hover.markdown);
    assert!(
        hover.markdown.contains("Renders a collection of records")
            || hover.markdown.contains("tabular data"),
        "expected prose description ahead of catalog line:\n{}",
        hover.markdown
    );
    let body = hover.markdown.split("---").next().unwrap_or(&hover.markdown);
    assert!(
        body.len() > 120,
        "ui::table hover should include more than the bare catalog line:\n{body}"
    );
}

#[test]
fn blog_contract_article_hover() {
    let src = blog_source();
    // Hover the contract name in `contract Article`.
    let needle = "contract Article";
    let offset = (src.find(needle).unwrap() + "contract ".len()) as u32;
    let doc = Document::open("file://blog.silc", 1, &src);
    let hover = resolve_hover(&doc, offset).expect("Article hover");
    assert!(
        hover.markdown.contains("contract") || hover.markdown.contains("Article"),
        "{}",
        hover.markdown
    );
}

#[test]
fn blog_resource_articles_hover() {
    let src = blog_source();
    let needle = "resource Articles";
    let offset = (src.find(needle).unwrap() + "resource ".len()) as u32;
    let doc = Document::open("file://blog.silc", 1, &src);
    let hover = resolve_hover(&doc, offset).expect("Articles resource hover");
    assert!(hover.markdown.contains("resource"), "{}", hover.markdown);
    assert!(hover.markdown.contains("Article"), "{}", hover.markdown);
}

#[test]
fn blog_keyword_and_type_hover_via_lsp_position() {
    let src = blog_source();
    let doc = Document::open("file://blog.silc", 1, &src);
    let query_offset = src.find("query list").expect("query list") as u32;
    let (line, character) = offset_to_lsp(&src, query_offset as usize);
    let hover = hover_at_lsp(&doc, line, character).expect("keyword hover");
    assert!(hover.markdown.contains("keyword"), "{}", hover.markdown);

    let str_offset = src.find("has Str").map(|i| i + 4).expect("has Str") as u32;
    let (line, character) = offset_to_lsp(&src, str_offset as usize);
    let hover = hover_at_lsp(&doc, line, character).expect("Str hover");
    assert!(
        hover.markdown.contains("builtin type") || hover.markdown.contains("Str"),
        "{}",
        hover.markdown
    );
}

#[test]
fn blog_query_binding_articles_var() {
    let src = blog_source();
    // `query $.articles = Articles.list();`
    let idx = src.find("$.articles = Articles").expect("query binding");
    let offset = (idx + 2) as u32; // on 'a' of articles
    let doc = Document::open("file://blog.silc", 1, &src);
    let hover = resolve_hover(&doc, offset).expect("binding hover");
    assert!(
        hover.markdown.contains("query binding")
            || hover.markdown.contains("Articles.list")
            || hover.markdown.contains("articles"),
        "{}",
        hover.markdown
    );
    assert!(
        hover.markdown.contains("Read-only") || hover.markdown.contains("re-run"),
        "expected query-binding lifetime prose:\n{}",
        hover.markdown
    );
}

#[test]
fn blog_handler_param_and_field_hover() {
    let src = blog_source();
    // `method on_select(Article $article) { … $article.id … }`
    let param_idx = src.find("Article $article)").expect("handler param");
    let param_offset = (param_idx + "Article $".len()) as u32;
    let doc = Document::open("file://blog.silc", 1, &src);
    let hover = resolve_hover(&doc, param_offset).expect("param hover");
    assert!(
        hover.markdown.contains("parameter")
            || hover.markdown.contains("article")
            || hover.markdown.contains("Article"),
        "{}",
        hover.markdown
    );

    // Prefer the assignment in on_select: `$.selected_id = $article.id;`
    let field_idx = src
        .find("$.selected_id = $article.id")
        .expect("$article.id assignment");
    let id_offset = (field_idx + "$.selected_id = $article.".len()) as u32;
    let hover = resolve_hover(&doc, id_offset).expect("field hover");
    assert!(
        hover.markdown.contains("field")
            || hover.markdown.contains("id")
            || hover.markdown.contains("UUID")
            || hover.markdown.contains("Article"),
        "{}",
        hover.markdown
    );
}

#[test]
fn blog_feed_operator_hover() {
    let src = blog_source();
    let Some(idx) = src.find("==>") else {
        // blogApp may not use feed ops; still ensure operator docs resolve on a snippet.
        let snippet = "processor X { method m() { $x ==> llm::complete() } }\n";
        let offset = snippet.find("==>").unwrap() as u32;
        let doc = Document::open("file://feed.silc", 1, snippet);
        // Parse may fail for incomplete program; token hover should still work.
        if let Some(hover) = resolve_hover(&doc, offset) {
            assert!(hover.markdown.contains("operator") || hover.markdown.contains("==>"), "{}", hover.markdown);
        }
        return;
    };
    let doc = Document::open("file://blog.silc", 1, &src);
    let hover = resolve_hover(&doc, idx as u32).expect("==> hover");
    assert!(
        hover.markdown.contains("operator") || hover.markdown.contains("feed"),
        "{}",
        hover.markdown
    );
}
