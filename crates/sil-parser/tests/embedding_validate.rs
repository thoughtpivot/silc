//! Embeddings in apps and loops, and `query nearest` (THO-132, ADR-010).

use sil_core::{infer_graph, ProcessorOp};
use sil_parser::parse;

const FIXTURE: &str = include_str!("../../silc/tests/fixtures/embedding_app.silc");

fn graph(src: &str) -> Result<sil_core::ExecutableGraph, String> {
    let program = parse(src).map_err(|e| format!("parse: {}", e.message))?;
    program.validate()?;
    Ok(infer_graph(&program)?.expect("runnable graph"))
}

fn expect_err(src: &str, needle: &str) {
    match graph(src) {
        Ok(_) => panic!("expected error containing `{needle}`, but it validated"),
        Err(err) => assert!(err.contains(needle), "expected `{needle}` in: {err}"),
    }
}

#[test]
fn app_loop_llm_and_tensor_feed_build_together() {
    let graph = graph(FIXTURE).unwrap();
    assert_eq!(graph.processor_op, ProcessorOp::LlmComplete);
    assert_eq!(graph.processor, "Assistant");
    assert!(!graph.is_pipeline_only());
    assert!(graph.has_ui());
    assert!(graph.doc.active());
    let plan = graph.embedding.as_ref().expect("embedding plan");
    assert!(graph.needs_embed_worker());
    assert_eq!(plan.model.id, "minilm-l6-v2");
    assert_eq!(plan.model.dimension, 384);
    assert_eq!(plan.feeds.len(), 1);
    let feed = &plan.feeds[0];
    assert_eq!(feed.processor, "Embedder");
    assert_eq!(feed.contract, "FileEntry");
    assert_eq!(feed.input_field, "text_excerpt");
    assert_eq!(feed.output_field, "vector_embedding");
    assert_eq!(feed.tables, vec!["files".to_string()]);
    assert_eq!(plan.nearest.len(), 1);
    assert_eq!(plan.nearest[0].resource, "Files");
    assert_eq!(plan.nearest[0].table, "files");
    assert!(graph
        .actions
        .iter()
        .any(|a| a.id == "Files.nearest" && a.path == "/api/files/nearest"));
}

#[test]
fn embeds_never_count_as_model_calls() {
    let program = parse(FIXTURE).unwrap();
    for lp in &program.loops {
        let bounds = sil_core::loops::loop_bounds(lp);
        assert_eq!(bounds.model_calls, 0, "loop `{}`", lp.name);
    }
}

#[test]
fn cuda_still_fails_with_release_label() {
    let src = FIXTURE.replace(":prefer(CPU)", ":prefer(CUDA)");
    let err = graph(&src).unwrap_err();
    assert!(err.contains("CPU-only") && err.contains("CUDA"), "{err}");
    assert!(err.contains(sil_core::silc_release()), "{err}");
}

#[test]
fn unknown_model_fails() {
    expect_err(
        &FIXTURE.replace("minilm-l6-v2", "not-a-model"),
        "not-a-model",
    );
}

#[test]
fn vector_field_must_match_model_dimension() {
    let src = FIXTURE.replace(
        "subset Emb384 of Vec[num32; 384];",
        "subset Emb384 of Vec[num32; 768];",
    );
    let err = graph(&src).unwrap_err();
    assert!(err.contains("768") && err.contains("384"), "{err}");
}

#[test]
fn served_model_declares_its_own_dimension() {
    let src = FIXTURE
        .replace("minilm-l6-v2", "nomic-embed-text")
        .replace("Vec[num32; 384]", "Vec[num32; 768]")
        .replace(":prefer(CPU)", "");
    let graph = graph(&src).unwrap();
    let plan = graph.embedding.unwrap();
    assert_eq!(plan.model.id, "nomic-embed-text");
    assert_eq!(plan.model.dimension, 768);
}

#[test]
fn nearest_limit_is_a_bounded_integer_literal() {
    expect_err(
        &FIXTURE.replace(
            "Files.nearest($.needle, 50)",
            "Files.nearest($.needle, $.needle)",
        ),
        "integer literal",
    );
    expect_err(
        &FIXTURE.replace(
            "Files.nearest($.needle, 50)",
            "Files.nearest($.needle, 10001)",
        ),
        "between 1 and 10000",
    );
    expect_err(
        &FIXTURE.replace("Files.nearest($.needle, 50)", "Files.nearest($.needle, 0)"),
        "between 1 and 10000",
    );
    expect_err(
        &FIXTURE.replace("Files.nearest($.needle, 50)", "Files.nearest($.needle)"),
        "expected 2",
    );
}

#[test]
fn nearest_needs_a_vector_field() {
    let src = FIXTURE.replace("    has Emb384 $.vector_embedding;\n", "");
    let err = graph(&src).unwrap_err();
    assert!(err.contains("vector_embedding"), "{err}");
}

#[test]
fn feed_input_must_be_a_contract_string_field() {
    expect_err(
        &FIXTURE.replace("$e.text_excerpt ==>", "$e.missing ==>"),
        "is not a field",
    );
}

#[test]
fn nearest_alone_uses_the_default_model() {
    let start = FIXTURE.find("processor Embedder").unwrap();
    let end = FIXTURE.find("processor Assistant").unwrap();
    let src = format!("{}{}", &FIXTURE[..start], &FIXTURE[end..]);
    let graph = graph(&src).unwrap();
    let plan = graph.embedding.unwrap();
    assert_eq!(plan.model.id, "minilm-l6-v2");
    assert!(plan.feeds.is_empty());
    assert_eq!(plan.nearest.len(), 1);
}

#[test]
fn text_score_with_embedding_stays_banned() {
    let start = FIXTURE.find("service Extractor").unwrap();
    let end = FIXTURE.find("loop IndexFiles").unwrap();
    let src = format!("{}{}", &FIXTURE[..start], &FIXTURE[end..]).replace(
        "$note.prompt ==> llm::complete()",
        "$note.prompt ==> text::score()",
    );
    expect_err(&src, "cannot mix text::score with tensor::infer");
}
