//! Static loop rules (ADR-014): each negative fixture must fail with a
//! diagnostic that names the rule.

use sil_parser::parse;

const PRELUDE: &str = r#"
@version("0.8")
contract Rfi {
    has Str $.id;
    has Str $.status;
    has Str $.due;
    has Str $.assignee;
}
contract Note {
    has Str $.id;
    has Str $.rfi;
    has Str $.text;
}
contract Draft {
    has Str $.text;
}
resource Rfis for Rfi {
    query list;
    mutation update;
}
resource Notes for Note {
    query list;
    mutation create;
}
"#;

fn check(body: &str) -> Result<(), String> {
    let src = format!("{PRELUDE}\nloop Chase {{\n{body}\n}}\n");
    let program = parse(&src).map_err(|e| format!("parse: {}", e.message))?;
    program.validate()
}

fn expect_err(body: &str, needle: &str) {
    match check(body) {
        Ok(()) => panic!("expected error containing `{needle}`, but it validated:\n{body}"),
        Err(e) => assert!(e.contains(needle), "expected `{needle}` in error, got: {e}"),
    }
}

const GOOD: &str = r#"
loop::flow(
    loop::schedule(:cron("0 8 * * 1-5"), :tz("America/New_York")),
    loop::find(:as(rfis), :from(Rfis.list), :where($.status == "open"), :max(50)),
    loop::each(:in($rfis), :as(rfi), :max(50),
        loop::ask(:as(draft), :into(Draft), :from($rfi), :prompt("Remind {$rfi.assignee}")),
        loop::gate(:that($draft.text != ""), :reason("never write an empty note")),
        loop::write(:to(Notes.create), :key("{$rfi.id}:{$today}"), :value(Note.new(:rfi($rfi.id), :text($draft.text))))
    )
)
"#;

#[test]
fn good_loop_validates() {
    check(GOOD).unwrap();
}

#[test]
fn cost_report_multiplies_through_each() {
    let src = format!("{PRELUDE}\nloop Chase {{\n{GOOD}\n}}\n");
    let program = parse(&src).unwrap();
    let b = sil_core::loop_bounds(&program.loops[0]);
    assert_eq!(b.model_calls, 100, "50 items × (1 call + 1 retry)");
    assert_eq!(b.writes, 50);
    assert_eq!(b.rows_scanned, 50);
    let report = sil_core::format_loop_cost_report(&program).unwrap();
    assert!(report.contains("Chase"), "{report}");
    assert!(report.contains("model calls 100"), "{report}");
    assert!(
        report.contains("schedule \"0 8 * * 1-5\" America/New_York"),
        "{report}"
    );
}

#[test]
fn rfi_chase_cost_report() {
    let program = parse(include_str!(
        "../../../examples/domains/aec/rfiChaseApp/main.silc"
    ))
    .unwrap();
    let report = sil_core::format_loop_cost_report(&program).unwrap();
    assert!(
        report.contains("model calls 600 · effects 600 (writes 400, notices 200) · approvals 200"),
        "{report}"
    );
    assert!(report.contains("OpenRfiDigest  manual\n"), "{report}");
}

#[test]
fn model_output_must_be_checked_before_an_effect() {
    let body = GOOD.replace(
        r#"loop::gate(:that($draft.text != ""), :reason("never write an empty note")),"#,
        "",
    );
    expect_err(&body, "has not passed a loop::gate or loop::approve");
}

#[test]
fn unchecked_lets_model_output_through_with_a_reason() {
    let body = GOOD
        .replace(
            r#"loop::gate(:that($draft.text != ""), :reason("never write an empty note")),"#,
            "",
        )
        .replace(
            r#":key("{$rfi.id}:{$today}"),"#,
            r#":key("{$rfi.id}:{$today}"), :unchecked("drafts are internal notes"),"#,
        );
    check(&body).unwrap();
}

#[test]
fn derived_values_stay_tainted() {
    let body = GOOD
        .replace(
            r#"loop::gate(:that($draft.text != ""), :reason("never write an empty note")),"#,
            r#"loop::let(:as(copy), :value($draft.text)),"#,
        )
        .replace(":text($draft.text)", ":text($copy)");
    expect_err(&body, "has not passed");
}

#[test]
fn approval_showing_the_value_clears_it() {
    let body = GOOD.replace(
        r#"loop::gate(:that($draft.text != ""), :reason("never write an empty note")),"#,
        r#"loop::approve(:by("pm"), :message("Send?"), :show($draft), :within("1d"),
            loop::declined(loop::skip(:reason("declined"))),
            loop::timed_out(loop::skip(:reason("late")))),"#,
    );
    check(&body).unwrap();
}

#[test]
fn command_program_cannot_wait_for_approval() {
    let approve = GOOD.replace(
        r#"loop::gate(:that($draft.text != ""), :reason("never write an empty note")),"#,
        r#"loop::approve(:by("pm"), :message("Send?"), :show($draft), :within("1d"),
            loop::declined(loop::skip(:reason("declined"))),
            loop::timed_out(loop::skip(:reason("late")))),"#,
    );
    let manual = |body: &str| {
        body.replace(
            r#"loop::schedule(:cron("0 8 * * 1-5"), :tz("America/New_York")),"#,
            "loop::manual(),",
        )
    };
    check(&manual(GOOD)).unwrap();
    expect_err(&manual(&approve), "this program is a command");
}

#[test]
fn find_requires_max() {
    expect_err(&GOOD.replace(", :max(50)),", "),"), "bounded");
}

#[test]
fn each_max_is_capped() {
    expect_err(
        &GOOD.replace(":as(rfi), :max(50)", ":as(rfi), :max(20000)"),
        "between 1 and 10000",
    );
}

#[test]
fn effect_key_needs_a_placeholder() {
    expect_err(
        &GOOD.replace(r#":key("{$rfi.id}:{$today}")"#, r#":key("same")"#),
        "placeholder",
    );
}

#[test]
fn effect_requires_key() {
    expect_err(
        &GOOD.replace(r#":key("{$rfi.id}:{$today}"), "#, ""),
        "requires option `:key`",
    );
}

#[test]
fn gate_otherwise_must_terminate() {
    let body = GOOD.replace(
        r#":reason("never write an empty note")),"#,
        r#":reason("never write an empty note"), loop::otherwise(loop::let(:as(x), :value(1)))),"#,
    );
    expect_err(&body, "must end in loop::stop, loop::fail, or loop::skip");
}

#[test]
fn skip_only_inside_each() {
    let body = r#"
loop::flow(
    loop::manual(),
    loop::skip(:reason("nothing"))
)"#;
    expect_err(body, "only valid inside loop::each");
}

#[test]
fn steps_after_stop_are_unreachable() {
    let body = r#"
loop::flow(
    loop::manual(),
    loop::stop(),
    loop::let(:as(x), :value(1))
)"#;
    expect_err(body, "unreachable");
}

#[test]
fn unknown_resource_is_rejected() {
    expect_err(
        &GOOD.replace("Rfis.list", "Tickets.list"),
        "unknown resource `Tickets`",
    );
}

#[test]
fn find_needs_a_list_query() {
    expect_err(
        &GOOD.replace("Rfis.list", "Rfis.update"),
        "is not a list query",
    );
}

#[test]
fn write_needs_a_declared_mutation() {
    expect_err(
        &GOOD.replace("Notes.create", "Notes.update"),
        "not a create/update mutation",
    );
}

#[test]
fn approve_needs_both_outcome_blocks() {
    let body = GOOD.replace(
        r#"loop::gate(:that($draft.text != ""), :reason("never write an empty note")),"#,
        r#"loop::approve(:by("pm"), :message("Send?"), :show($draft), :within("1d"),
            loop::declined(loop::skip(:reason("declined")))),"#,
    );
    expect_err(&body, "loop::timed_out");
}

#[test]
fn approve_within_is_bounded() {
    let body = GOOD.replace(
        r#"loop::gate(:that($draft.text != ""), :reason("never write an empty note")),"#,
        r#"loop::approve(:by("pm"), :message("Send?"), :show($draft), :within("45d"),
            loop::declined(loop::skip(:reason("declined"))),
            loop::timed_out(loop::skip(:reason("late")))),"#,
    );
    expect_err(&body, "30d");
}

#[test]
fn only_one_trigger() {
    expect_err(
        &GOOD.replace("loop::schedule(", "loop::manual(), loop::schedule("),
        "only one is allowed",
    );
}

#[test]
fn flow_needs_a_trigger_first() {
    let body = r#"
loop::flow(
    loop::let(:as(x), :value(1))
)"#;
    expect_err(body, "must be a trigger");
}

#[test]
fn cron_and_tz_are_checked() {
    expect_err(&GOOD.replace("0 8 * * 1-5", "0 8 * *"), "5 fields");
    expect_err(&GOOD.replace("America/New_York", "Eastern"), "IANA");
}

#[test]
fn unknown_binding_is_rejected() {
    expect_err(
        &GOOD.replace("$rfi.assignee", "$ticket.assignee"),
        "unknown binding `$ticket`",
    );
}

#[test]
fn unknown_field_on_a_row_is_rejected() {
    expect_err(
        &GOOD.replace("$rfi.assignee", "$rfi.owner"),
        "has no field `owner`",
    );
}

#[test]
fn bindings_inside_each_do_not_leak() {
    let body = GOOD.replacen(
        "\n)\n",
        ",\n    loop::notify(:to(\"pm\"), :text(\"{$draft.text}\"), :key(\"{$today}\"))\n)\n",
        1,
    );
    expect_err(&body, "unknown binding `$draft`");
}

#[test]
fn duplicate_bindings_are_rejected() {
    expect_err(&GOOD.replace(":as(draft)", ":as(rfi)"), "already declared");
}

#[test]
fn calls_are_not_expressions() {
    expect_err(
        &GOOD.replace(r#"$.status == "open""#, "Rfis.list()"),
        "calls are not allowed",
    );
}

#[test]
fn unknown_props_and_nodes_are_rejected() {
    expect_err(
        &GOOD.replace(":max(50)),", ":max(50), :limit(3)),"),
        "unknown option `:limit`",
    );
    expect_err(
        &GOOD.replace("loop::gate(", "loop::check("),
        "unknown node `loop::check`",
    );
}

#[test]
fn mutation_trigger_cannot_write_its_own_resource() {
    let body = r#"
loop::flow(
    loop::on_mutation(:resource(Rfis), :mutation(update)),
    loop::write(:to(Rfis.update), :key("{$event.id}"), :value(Rfi.new(:id($event.id), :status("seen"))))
)"#;
    expect_err(body, "retrigger");
}

#[test]
fn write_value_must_match_the_resource_contract() {
    expect_err(
        &GOOD.replace(
            "Note.new(:rfi($rfi.id), :text($draft.text))",
            "Rfi.new(:id($rfi.id))",
        ),
        ":value must be `Note.new(...)`",
    );
}

#[test]
fn update_needs_an_id() {
    let body = r#"
loop::flow(
    loop::manual(),
    loop::write(:to(Rfis.update), :key("{$today}"), :value(Rfi.new(:status("x"))))
)"#;
    expect_err(body, "must set `:id(...)`");
}

#[test]
fn loops_cannot_mix_with_games() {
    let src = format!(
        "{PRELUDE}\nloop Chase {{ loop::flow(loop::manual(), loop::stop()) }}\ngame G {{ scene::scene(:title(\"T\")) }}\n"
    );
    let err = parse(&src).unwrap().validate().unwrap_err();
    assert!(err.contains("cannot mix") && err.contains("game"), "{err}");
}

#[test]
fn reserved_loop_names() {
    let src = format!("{PRELUDE}\nloop LoopInbox {{ loop::flow(loop::manual(), loop::stop()) }}\n");
    let err = parse(&src).unwrap().validate().unwrap_err();
    assert!(err.contains("reserved"), "{err}");
}

fn score_processor_program(extra_processor: &str) -> String {
    format!(
        r#"{PRELUDE}
contract Msg {{
    has Str $.text;
}}
component Page {{
    method render() {{ ui::text(:text("x")) }}
}}
app A {{
    route "/" => Page;
}}
processor Scorer {{
    method score(Msg $m) {{
        $m.text ==> text::score()
    }}{extra_processor}
}}
loop Chase {{
{GOOD}
}}
"#
    )
}

#[test]
fn ask_shares_a_program_with_a_text_score_processor() {
    let program = parse(&score_processor_program("")).unwrap();
    program
        .validate()
        .expect("text::score and loop::ask coexist");
    let graph = sil_core::infer_graph(&program).unwrap().unwrap();
    assert_eq!(graph.processor_op, sil_core::ProcessorOp::Score);
    assert!(graph.needs_llm(), "loop::ask still provisions silclm");
}

#[test]
fn text_score_still_cannot_mix_with_llm_complete() {
    let src = score_processor_program(
        "\n    method reply(Msg $m) {\n        $m.text ==> llm::complete()\n    }",
    );
    let err = parse(&src).unwrap().validate().unwrap_err();
    assert!(
        err.contains("cannot mix")
            && err.contains("llm::complete")
            && err.contains("processor slot"),
        "{err}"
    );
}

const MCP_PRELUDE: &str = r#"
contract KbArgs {
    has Str $.path;
    has Int $.tailRecords;
}
"#;

fn check_mcp(read: &str) -> Result<(), String> {
    let body = format!(
        r#"loop::flow(
    loop::manual(),
    {read},
    loop::ask(:as(draft), :into(Draft), :from($kb.data), :prompt("Summarize for {{$calendar.today}}")),
    loop::gate(:that($draft.text != ""), :reason("never write an empty note")),
    loop::write(:to(Notes.create), :key("kb:{{$calendar.today}}"), :value(Note.new(:rfi("kb"), :text($draft.text))))
)"#
    );
    let src = format!("{PRELUDE}{MCP_PRELUDE}\nloop Brief {{\n{body}\n}}\n");
    let program = parse(&src).map_err(|e| format!("parse: {}", e.message))?;
    program.validate()
}

const GOOD_MCP: &str = r#"loop::read(:as(kb), :op("mcp::call"), :server("https://kb.example.com/mcp"), :auth_env("KB_TOKEN"), :tool("kb_jsonl_read_window"), :args(KbArgs.new(:path("memory/decisions.jsonl"), :tailRecords(6))), :select("records.parsed"))"#;

fn expect_mcp_err(read: &str, needle: &str) {
    match check_mcp(read) {
        Ok(()) => panic!("expected error containing `{needle}`, but it validated:\n{read}"),
        Err(e) => assert!(e.contains(needle), "expected `{needle}` in error, got: {e}"),
    }
}

#[test]
fn mcp_call_read_validates() {
    check_mcp(GOOD_MCP).unwrap();
}

#[test]
fn mcp_call_requires_server_and_tool() {
    expect_mcp_err(
        &GOOD_MCP.replace(r#":server("https://kb.example.com/mcp"), "#, ""),
        "requires :server",
    );
    expect_mcp_err(
        &GOOD_MCP.replace(r#":tool("kb_jsonl_read_window"), "#, ""),
        "requires :tool",
    );
    expect_mcp_err(
        &GOOD_MCP.replace("https://kb.example.com/mcp", "kb.example.com/mcp"),
        "absolute http(s)",
    );
}

#[test]
fn mcp_call_rejects_url_and_scrape_rejects_mcp_props() {
    expect_mcp_err(
        &GOOD_MCP.replace(r#":tool("#, r#":url("https://x.example"), :tool("#),
        "not :url",
    );
    expect_mcp_err(
        r#"loop::read(:as(kb), :op("scrape::page"), :url("https://x.example"), :tool("kb_search"))"#,
        ":tool only applies to `mcp::call`",
    );
}

#[test]
fn mcp_call_args_select_and_auth_env_are_checked() {
    expect_mcp_err(
        &GOOD_MCP.replace(
            r#"KbArgs.new(:path("memory/decisions.jsonl"), :tailRecords(6))"#,
            r#""memory/decisions.jsonl""#,
        ),
        ":args must be `Contract.new(...)`",
    );
    expect_mcp_err(
        &GOOD_MCP.replace("KB_TOKEN", "kb-token"),
        ":auth_env must be an environment variable name",
    );
    expect_mcp_err(
        &GOOD_MCP.replace("records.parsed", "records..parsed"),
        ":select must be a dotted field path",
    );
}

#[test]
fn mcp_result_and_calendar_fields_are_closed() {
    let src = GOOD_MCP.to_string();
    let bad_field = format!("{src},\n    loop::let(:as(x), :value($kb.title))");
    expect_mcp_err(&bad_field, "`$kb` has no field `title`");
    let bad_calendar = format!("{src},\n    loop::let(:as(x), :value($calendar.tomorrow))");
    expect_mcp_err(&bad_calendar, "`$calendar` has no field `tomorrow`");
    expect_mcp_err(
        &GOOD_MCP.replace("loop::read(:as(kb)", "loop::read(:as(calendar)"),
        "`calendar` is reserved",
    );
}

#[test]
fn mcp_fixture_validates_and_reports_costs() {
    let program = parse(include_str!(
        "../../sil-codegen/tests/fixtures/loop_digest.silc"
    ))
    .unwrap();
    program.validate().unwrap();
    let report = sil_core::format_loop_cost_report(&program).unwrap();
    assert!(
        report.contains("DailyDigest  schedule \"0 5 * * *\" UTC, catch up 12h"),
        "{report}"
    );
    assert!(
        report.contains("model calls 6 · effects 2 (writes 1, notices 1) · approvals 0 · reads 6"),
        "{report}"
    );
}

// ---------------------------------------------------------------- loop::fallback (THO-559)

const FALLBACK: &str = r#"loop::fallback(:as(page), :accept($page.status == 200 && $page.text != ""),
        loop::read(scrape::page(:url("https://example.com/changelog"))),
        loop::read(scrape::page(:url("https://example.com/releases"), :retry(0))),
        loop::otherwise(loop::fail(:reason("no readable changelog"))))"#;

fn check_fallback(fallback: &str) -> Result<(), String> {
    check(&fallback_flow(fallback))
}

fn fallback_flow(fallback: &str) -> String {
    format!(
        r#"loop::flow(
    loop::manual(),
    {fallback},
    loop::write(:to(Notes.create), :key("{{$page.url}}"), :value(Note.new(:rfi("page"), :text($page.title))))
)"#
    )
}

fn expect_fallback_err(fallback: &str, needle: &str) {
    match check_fallback(fallback) {
        Ok(()) => panic!("expected error containing `{needle}`, but it validated:\n{fallback}"),
        Err(e) => assert!(e.contains(needle), "expected `{needle}` in error, got: {e}"),
    }
}

#[test]
fn fallback_validates_and_binds_the_accepted_page() {
    check_fallback(FALLBACK).unwrap();
}

#[test]
fn fallback_composes_with_each_skip_and_ask() {
    let body = r#"loop::flow(
    loop::manual(),
    loop::find(:as(rfis), :from(Rfis.list), :max(5)),
    loop::each(:in($rfis), :as(rfi), :max(5),
        loop::fallback(:as(page), :accept($page.status == 200 && $page.text != ""),
            loop::read(scrape::page(:url("https://example.com/changelog"))),
            loop::read(scrape::page(:url("https://example.com/releases"))),
            loop::otherwise(loop::skip(:reason("no answer that fits Impact")))),
        loop::ask(:as(draft), :into(Draft), :from($page), :retry(2), :prompt("Summarize {$page.title}"),
            loop::otherwise(loop::skip(:reason("no answer that fits Draft")))),
        loop::gate(:that($draft.text != ""), :reason("never write an empty note")),
        loop::write(:to(Notes.create), :key("{$rfi.id}:{$today}"), :value(Note.new(:rfi($rfi.id), :text($draft.text))))
    )
)"#;
    check(body).unwrap();
}

#[test]
fn fallback_accepts_mcp_call_attempts() {
    let fallback = r#"loop::fallback(:as(kb), :accept($kb.text != ""),
        loop::read(mcp::call(:server("https://a.example.com/mcp"), :tool("search"))),
        loop::read(mcp::call(:server("https://b.example.com/mcp"), :tool("search"))),
        loop::otherwise(loop::fail(:reason("no readable result"))))"#;
    let body = format!(
        r#"loop::flow(
    loop::manual(),
    {fallback},
    loop::write(:to(Notes.create), :key("{{$kb.text}}"), :value(Note.new(:rfi("kb"), :text($kb.text))))
)"#
    );
    check(&body).unwrap();
}

#[test]
fn fallback_needs_a_terminating_otherwise() {
    let without = FALLBACK.replace(
        r#",
        loop::otherwise(loop::fail(:reason("no readable changelog")))"#,
        "",
    );
    expect_fallback_err(&without, "exactly one loop::otherwise");
    let falls_through = FALLBACK.replace(
        r#"loop::fail(:reason("no readable changelog"))"#,
        r#"loop::let(:as(none), :value("x"))"#,
    );
    expect_fallback_err(
        &falls_through,
        "must end in loop::stop, loop::fail, or loop::skip",
    );
    let twice = FALLBACK.replace(
        r#"loop::otherwise(loop::fail(:reason("no readable changelog")))"#,
        r#"loop::otherwise(loop::fail(:reason("a"))), loop::otherwise(loop::fail(:reason("b")))"#,
    );
    expect_fallback_err(&twice, "exactly one loop::otherwise");
}

#[test]
fn fallback_needs_two_or_more_attempts() {
    let one = FALLBACK.replace(
        r#"loop::read(scrape::page(:url("https://example.com/releases"), :retry(0))),
        "#,
        "",
    );
    expect_fallback_err(&one, "two or more loop::read attempts");
}

#[test]
fn fallback_otherwise_comes_last_and_only_reads_are_attempts() {
    let otherwise_first = r#"loop::fallback(:as(page), :accept($page.status == 200),
        loop::otherwise(loop::fail(:reason("x"))),
        loop::read(scrape::page(:url("https://example.com/a"))),
        loop::read(scrape::page(:url("https://example.com/b"))))"#;
    expect_fallback_err(otherwise_first, "followed by exactly one loop::otherwise");
    let not_a_read = FALLBACK.replace(
        r#"loop::read(scrape::page(:url("https://example.com/releases"), :retry(0))),"#,
        r#"loop::gate(:that(true), :reason("x")),"#,
    );
    expect_fallback_err(&not_a_read, "cannot contain loop::gate");
}

#[test]
fn fallback_requires_as_and_accept() {
    expect_fallback_err(
        &FALLBACK.replace(r#":accept($page.status == 200 && $page.text != ""),"#, ""),
        "requires option `:accept`",
    );
    expect_fallback_err(
        &FALLBACK.replace(":as(page), ", ""),
        "requires option `:as`",
    );
}

#[test]
fn fallback_accept_must_test_the_attempt_result() {
    expect_fallback_err(
        &FALLBACK.replace(r#"$page.status == 200 && $page.text != """#, "true"),
        ":accept must test the attempt's result through `$page`",
    );
    expect_fallback_err(
        &FALLBACK.replace(
            "$page.status == 200 && $page.text != \"\"",
            "$page.status == 200 && $other.x != \"\"",
        ),
        "unknown binding `$other`",
    );
    expect_fallback_err(
        &FALLBACK.replace("$page.text != \"\"", "$page.body != \"\""),
        "`$page` has no field `body`",
    );
}

#[test]
fn fallback_attempts_share_the_fallbacks_binding() {
    expect_fallback_err(
        &FALLBACK.replace(
            r#"loop::read(scrape::page(:url("https://example.com/changelog"))),"#,
            r#"loop::read(:as(first), scrape::page(:url("https://example.com/changelog"))),"#,
        ),
        "a read has no :as",
    );
    expect_fallback_err(
        &FALLBACK.replace(
            r#"loop::read(scrape::page(:url("https://example.com/releases"), :retry(0))),"#,
            r#"loop::read(mcp::call(:server("https://a.example.com/mcp"), :tool("search"))),"#,
        ),
        "attempts must use one read operation",
    );
}

#[test]
fn fallback_attempts_keep_the_read_rules() {
    expect_fallback_err(
        &FALLBACK.replace(":retry(0)", ":retry(11)"),
        ":retry must be at most 10",
    );
    expect_fallback_err(
        &FALLBACK.replace("https://example.com/changelog", "example.com/changelog"),
        ":url must be an absolute http(s) URL",
    );
    expect_fallback_err(
        &FALLBACK.replace(
            r#"scrape::page(:url("https://example.com/changelog"))"#,
            r#"scrape::page(:url($missing.url))"#,
        ),
        "unknown binding `$missing`",
    );
}

#[test]
fn fallback_binding_is_scoped_like_other_reads() {
    expect_fallback_err(
        &FALLBACK.replace(
            r#"loop::fail(:reason("no readable changelog"))"#,
            r#"loop::fail(:reason("tried {$page.url}"))"#,
        ),
        "unknown binding `$page`",
    );
    let duplicate = fallback_flow(&format!(
        r#"loop::read(:as(page), scrape::page(:url("https://example.com/a"))),
    {FALLBACK}"#
    ));
    match check(&duplicate) {
        Ok(()) => panic!("a second `page` binding must be rejected"),
        Err(e) => assert!(e.contains("binding `page` is already declared"), "{e}"),
    }
}

#[test]
fn fallback_skip_is_only_valid_inside_each() {
    expect_fallback_err(
        &FALLBACK.replace(
            r#"loop::fail(:reason("no readable changelog"))"#,
            r#"loop::skip(:reason("no readable changelog"))"#,
        ),
        "is only valid inside loop::each",
    );
}

#[test]
fn fallback_cost_report_counts_every_attempt() {
    let src = format!(
        "{PRELUDE}\nloop Chase {{\n{}\n}}\n",
        fallback_flow(FALLBACK)
    );
    let program = parse(&src).unwrap();
    let b = sil_core::loop_bounds(&program.loops[0]);
    assert_eq!(
        b.reads,
        (1 + sil_core::loops::DEFAULT_READ_RETRY) + 1,
        "first read keeps its default retries, second says :retry(0)"
    );
    let report = sil_core::format_loop_cost_report(&program).unwrap();
    assert!(report.contains("reads 4"), "{report}");
}

#[test]
fn fallback_inside_each_multiplies_attempts() {
    let body = r#"loop::flow(
    loop::manual(),
    loop::find(:as(rfis), :from(Rfis.list), :max(5)),
    loop::each(:in($rfis), :as(rfi), :max(5),
        loop::fallback(:as(page), :accept($page.status == 200),
            loop::read(scrape::page(:url("https://example.com/a")), :retry(1)),
            loop::read(scrape::page(:url("https://example.com/b")), :retry(1)),
            loop::otherwise(loop::skip(:reason("none")))),
        loop::notify(:to("pm"), :text("{$page.title}"), :key("{$rfi.id}"))
    )
)"#;
    let src = format!("{PRELUDE}\nloop Chase {{\n{body}\n}}\n");
    let program = parse(&src).unwrap();
    program.validate().unwrap();
    assert_eq!(sil_core::loop_bounds(&program.loops[0]).reads, 5 * 4);
}

const TYPED_PRELUDE: &str = r#"
@version("0.8")
subset Emb384 of Vec[num32; 384];
subset Emb768 of Vec[num32; 768];
contract Src {
    has Str $.id;
    has Str $.url;
    has Int $.hits;
    has Emb384 $.vector_embedding;
    has Emb768 $.wide_embedding;
}
contract Row {
    has Str $.id;
    has Int $.n;
    has Emb384 $.vector_embedding;
}
resource Srcs for Src {
    query list;
}
resource Rows for Row {
    mutation create;
}
"#;

fn check_typed(write_value: &str) -> Result<(), String> {
    let src = format!(
        "{TYPED_PRELUDE}\nloop Typed {{\nloop::flow(\n    loop::manual(),\n    loop::find(:as(s), :from(Srcs.list), :max(5)),\n    loop::each(:in($s), :as(x), :max(2),\n        loop::write(:to(Rows.create), :key(\"r:{{$x.id}}\"), :value({write_value}))\n    )\n)\n}}\n"
    );
    let program = parse(&src).map_err(|e| format!("parse: {}", e.message))?;
    program.validate()
}

fn expect_typed_err(write_value: &str, needles: &[&str]) {
    match check_typed(write_value) {
        Ok(()) => panic!("expected a type error, but it validated: {write_value}"),
        Err(e) => {
            for needle in needles {
                assert!(e.contains(needle), "expected `{needle}` in error, got: {e}");
            }
        }
    }
}

#[test]
fn write_rejects_str_into_int_field() {
    expect_typed_err(
        "Row.new(:n($x.url))",
        &["field `n`", "expects Int", "`$x.url` is Str"],
    );
}

#[test]
fn write_rejects_str_into_emb384_field() {
    expect_typed_err(
        "Row.new(:vector_embedding($x.url))",
        &[
            "field `vector_embedding`",
            "expects Vec[num32; 384] (Emb384)",
            "`$x.url` is Str",
        ],
    );
}

#[test]
fn write_rejects_the_reference_repro() {
    expect_typed_err(
        "Row.new(:n($x.url), :vector_embedding($x.url))",
        &[
            "field `n`",
            "expects Int",
            "field `vector_embedding`",
            "expects Vec[num32; 384] (Emb384)",
        ],
    );
}

#[test]
fn write_rejects_wrong_dimension_vector() {
    expect_typed_err(
        "Row.new(:vector_embedding($x.wide_embedding))",
        &["expects Vec[num32; 384] (Emb384)", "is Vec[num32; 768]"],
    );
}

#[test]
fn write_rejects_number_and_text_literals_in_vector_field() {
    expect_typed_err("Row.new(:vector_embedding(1))", &["is Int"]);
    expect_typed_err("Row.new(:vector_embedding(\"abc\"))", &["is Str"]);
}

#[test]
fn write_rejects_int_into_str_field() {
    expect_typed_err(
        "Row.new(:id($x.hits))",
        &["field `id`", "expects Str", "is Int"],
    );
}

#[test]
fn write_rejects_unknown_type_into_typed_field() {
    let src = format!(
        "{TYPED_PRELUDE}\nloop Typed {{\nloop::flow(\n    loop::manual(),\n    loop::read(:as(r), :op(\"mcp::call\"), :server(\"https://example.com/mcp\"), :tool(\"t\")),\n    loop::write(:to(Rows.create), :key(\"r\"), :value(Row.new(:n($r.data))))\n)\n}}\n"
    );
    let err = parse(&src).unwrap().validate().unwrap_err();
    assert!(
        err.contains("field `n`") && err.contains("is not known"),
        "{err}"
    );
}

#[test]
fn write_accepts_correctly_typed_values() {
    check_typed("Row.new(:id($x.id), :n($x.hits), :vector_embedding($x.vector_embedding))")
        .unwrap();
    check_typed("Row.new(:id(\"fixed\"), :n(3))").unwrap();
    check_typed("Row.new(:id(\"{$x.id}:{$x.url}\"), :n($x.hits))").unwrap();
}

fn ask_from(from: &str) -> String {
    format!(
        r#"loop::flow(
    loop::manual(),
    loop::find(:as(release), :from(Rfis.list), :max(10)),
    loop::find(:as(seen), :from(Notes.list), :max(10)),
    loop::ask(:as(draft), :into(Draft), :retry(2), :from({from}), :prompt("Compare the release with what was seen")),
    loop::gate(:that($draft.text != ""), :reason("never write an empty note")),
    loop::write(:to(Notes.create), :key("watch:{{$today}}"), :value(Note.new(:rfi("watch"), :text($draft.text))))
)"#
    )
}

#[test]
fn ask_from_accepts_a_comma_list_of_bindings() {
    check(&ask_from("$release, $seen")).unwrap();
    check(&ask_from("$release, $seen, $release.count")).unwrap();
    check(&ask_from("[$release, $seen]")).unwrap();
}

#[test]
fn ask_from_comma_list_desugars_to_the_bracket_list() {
    let comma = parse(&format!(
        "{PRELUDE}\nloop Chase {{\n{}\n}}\n",
        ask_from("$release, $seen")
    ))
    .unwrap();
    let bracket = parse(&format!(
        "{PRELUDE}\nloop Chase {{\n{}\n}}\n",
        ask_from("[$release, $seen]")
    ))
    .unwrap();
    let from = |p: &sil_core::Program| {
        p.loops[0]
            .steps()
            .iter()
            .find(|s| s.name == "ask")
            .and_then(|s| s.prop("from"))
            .cloned()
            .unwrap()
    };
    assert_eq!(from(&comma), from(&bracket));
    assert!(matches!(from(&comma), sil_core::Expr::List(items) if items.len() == 2));
}

#[test]
fn ask_from_rejects_an_unknown_binding_in_either_position() {
    expect_err(&ask_from("$release, $missing"), "$missing");
    expect_err(&ask_from("$missing, $seen"), "$missing");
    expect_err(&ask_from("[$release, $missing]"), "$missing");
    expect_err(&ask_from("[$missing, $seen]"), "$missing");
}

#[test]
fn comma_list_is_only_valid_on_ask_from() {
    let other_option = GOOD.replace(":max(50)", ":max(50, 60)");
    let err = check(&other_option).unwrap_err();
    assert!(
        err.contains("only valid on `loop::ask :from`") && err.contains("bracket form"),
        "{err}"
    );
    let other_node = GOOD.replace(
        ":where($.status == \"open\")",
        ":where($.status == \"open\", 1)",
    );
    let err = check(&other_node).unwrap_err();
    assert!(err.contains("only valid on `loop::ask :from`"), "{err}");
    let other_ask_option = GOOD.replace(":into(Draft)", ":into(Draft, Note)");
    let err = check(&other_ask_option).unwrap_err();
    assert!(err.contains("only valid on `loop::ask :from`"), "{err}");
}

// ---------------------------------------------------------------- page reads (THO-554)

const PAGE_PRELUDE: &str = r#"
contract Dependency {
    has Str $.id;
    has Str $.changelog;
}
contract Source {
    has Str $.id;
    has Str $.url;
}
contract Release { has Str $.version; has Str $.text; }
contract Article { has Str $.title; has Str $.text; }
resource Dependencies for Dependency {
    query list;
}
resource Sources for Source {
    query list;
}
"#;

/// The site's UpstreamWatch read, verbatim, inside an `each` over `$dep`.
const RELEASE_READ: &str = r#"loop::read(:as(release), :into(Release),
  scrape::page(:url($dep.changelog), :select("article")))"#;

/// The site's ArticleIngest read, verbatim, inside an `each` over `$src`.
const ARTICLE_READ: &str = r#"loop::read(:as(article), :into(Article),
  scrape::page(:url($src.url), :select("article")))"#;

fn page_loop(read: &str, after: &str) -> String {
    format!(
        r#"loop::flow(
    loop::schedule(:cron("0 6 * * *"), :tz("UTC")),
    loop::find(:as(deps), :from(Dependencies.list), :max(20)),
    loop::each(:in($deps), :as(dep), :max(20),
        {read},
        {after}
    )
)"#
    )
}

fn release_loop(read: &str) -> String {
    page_loop(
        read,
        r#"loop::ask(:as(draft), :into(Draft), :from($release.text), :prompt("Summarize release {$release.version}")),
        loop::gate(:that($draft.text != ""), :reason("never write an empty note")),
        loop::write(:to(Notes.create), :key("{$dep.id}:{$release.version}"), :value(Note.new(:rfi($dep.id), :text($draft.text))))"#,
    )
}

fn check_page(body: &str) -> Result<sil_core::Program, String> {
    let src = format!("{PRELUDE}{PAGE_PRELUDE}\nloop Watch {{\n{body}\n}}\n");
    let program = parse(&src).map_err(|e| format!("parse: {}", e.message))?;
    program.validate()?;
    Ok(program)
}

fn expect_page_err(body: &str, needle: &str) {
    match check_page(body) {
        Ok(_) => panic!("expected error containing `{needle}`, but it validated:\n{body}"),
        Err(e) => assert!(e.contains(needle), "expected `{needle}` in error, got: {e}"),
    }
}

#[test]
fn site_release_read_types_version_and_text() {
    check_page(&release_loop(RELEASE_READ)).unwrap();
}

#[test]
fn site_article_read_types_text() {
    let body = format!(
        r#"loop::flow(
    loop::schedule(:cron("0 6 * * *"), :tz("UTC")),
    loop::find(:as(sources), :from(Sources.list), :max(20)),
    loop::each(:in($sources), :as(src), :max(20),
        {ARTICLE_READ},
        loop::ask(:as(draft), :into(Draft), :from($article.text), :prompt("Summarize {{$article.title}}")),
        loop::gate(:that($draft.text != ""), :reason("never write an empty note")),
        loop::write(:to(Notes.create), :key("{{$src.id}}:article"), :value(Note.new(:rfi($src.id), :text($draft.text))))
    )
)"#
    );
    check_page(&body).unwrap();
}

#[test]
fn page_read_into_unknown_contract_is_rejected() {
    expect_page_err(
        &release_loop(&RELEASE_READ.replace(":into(Release)", ":into(Changelog)")),
        "unknown contract `Changelog` in :into",
    );
}

#[test]
fn page_read_into_closes_the_row_fields() {
    expect_page_err(
        &release_loop(RELEASE_READ).replace("{$release.version}\")", "{$release.tag}\")"),
        "`$release` has no field `tag`",
    );
    expect_page_err(
        &release_loop(&RELEASE_READ.replace(
            r#":select("article")"#,
            r#":select("article"), :fields(Release.new(:tag("h2")))"#,
        )),
        "contract `Release` has no field `tag`",
    );
    // Without :into a page binds the fixed page shape.
    expect_page_err(
        &release_loop(&RELEASE_READ.replace(":into(Release),", "")),
        "`$release` has no field `version`",
    );
}

#[test]
fn page_read_fields_are_checked() {
    let with_fields = |fields: &str| {
        release_loop(&RELEASE_READ.replace(
            r#":select("article")"#,
            &format!(r#":select("article"), :fields({fields})"#),
        ))
    };
    check_page(&with_fields(r#"Release.new(:version("h2"))"#)).unwrap();
    expect_page_err(
        &with_fields(r#"Article.new(:title("h1"))"#),
        ":fields must be `Release.new(...)`",
    );
    expect_page_err(
        &with_fields(r#"Release.new(:version("h2:first-child"))"#),
        "pseudo-classes",
    );
    expect_page_err(
        &release_loop(&RELEASE_READ.replace(":into(Release),", "").replace(
            r#":select("article")"#,
            r#":select("article"), :fields(Release.new(:version("h2")))"#,
        )),
        ":fields needs :into(Contract)",
    );
}

#[test]
fn page_fields_must_match_page_types() {
    let src = format!(
        "{PRELUDE}{PAGE_PRELUDE}contract Hit {{ has Str $.text; has Str $.status; }}\nloop Watch {{\n{}\n}}\n",
        page_loop(
            r#"loop::read(:as(hit), :into(Hit), scrape::page(:url($dep.changelog)))"#,
            r#"loop::stop()"#,
        )
    );
    let err = parse(&src).unwrap().validate().unwrap_err();
    assert!(
        err.contains("`Hit.status` takes the page's `status`, so it must be Int"),
        "{err}"
    );
}

#[test]
fn page_select_rejects_bad_css() {
    for (sel, needle) in [
        ("", "the selector is empty"),
        ("article[", "attribute name"),
        ("article > ", "missing after a combinator"),
        ("a:hover", "pseudo-classes"),
        ("main, ", "missing after `,`"),
        ("1st", "unexpected `1`"),
    ] {
        expect_page_err(
            &release_loop(&RELEASE_READ.replace("\"article\"", &format!("\"{sel}\""))),
            needle,
        );
    }
    for good in [
        "article",
        "main > article.post",
        "#notes h2, section[data-kind=\"release\"]",
        "* + p ~ div",
    ] {
        sil_core::validate_css_selector(good).unwrap_or_else(|e| panic!("{good}: {e}"));
    }
}

#[test]
fn select_on_mcp_and_into_on_mcp_are_rejected_clearly() {
    expect_mcp_err(
        &GOOD_MCP.replace("records.parsed", "main > article"),
        "CSS selectors apply to `scrape::page`",
    );
    expect_mcp_err(
        &GOOD_MCP.replace("loop::read(:as(kb),", "loop::read(:as(kb), :into(Draft),"),
        ":into only applies to `scrape::page`",
    );
    expect_page_err(
        &release_loop(&RELEASE_READ.replace("\"article\"", "\"records..parsed\"")),
        "dotted JSON paths apply to `mcp::call`",
    );
}

#[test]
fn page_read_into_and_select_leave_the_reads_count_alone() {
    let reads = |read: &str| {
        let program = check_page(&page_loop(read, "loop::skip(:reason(\"done\"))")).unwrap();
        sil_core::loop_bounds(&program.loops[0]).reads
    };
    let plain = reads(
        &RELEASE_READ
            .replace(":into(Release),", "")
            .replace(r#", :select("article")"#, ""),
    );
    assert_eq!(plain, 20 * 3, "20 items × (1 fetch + 2 retries)");
    assert_eq!(reads(RELEASE_READ), plain);
    let program = check_page(&release_loop(RELEASE_READ)).unwrap();
    let report = sil_core::format_loop_cost_report(&program).unwrap();
    assert!(report.contains("reads 60"), "{report}");
}

#[test]
fn fallback_attempts_take_select_and_one_shared_into() {
    let fallback = |second_into: &str| {
        page_loop(
            &format!(
                r#"loop::fallback(:as(release), :accept($release.version != ""),
            loop::read(:into(Release), scrape::page(:url($dep.changelog), :select("article"), :fields(Release.new(:version("h2"))))),
            loop::read({second_into}scrape::page(:url($dep.changelog), :select("main"))),
            loop::otherwise(loop::skip(:reason("no release"))))"#
            ),
            r#"loop::notify(:to("Maintainers"), :text("{$release.version}: {$release.text}"), :key("{$dep.id}:{$release.version}"))"#,
        )
    };
    check_page(&fallback(":into(Release), ")).unwrap();
    expect_page_err(&fallback(""), "attempts must bind one shape");
    expect_page_err(
        &fallback(":into(Article), "),
        "attempts must bind one shape",
    );
    expect_page_err(
        &fallback(":into(Release), ").replace(r#":select("main")"#, r#":select("main:hover")"#),
        "pseudo-classes",
    );
}

const INTO_TYPED_PRELUDE: &str = r#"
@version("0.8")
contract Dep { has Str $.id; has Str $.changelog; }
contract Release { has Str $.version; has Str $.text; }
contract Hit { has Str $.title; has Int $.status; }
contract Stat { has Str $.id; has Int $.code; has Str $.label; }
resource Deps for Dep {
    query list;
}
resource Stats for Stat {
    mutation create;
}
"#;

fn check_into_write(read: &str, value: &str) -> Result<(), String> {
    let src = format!(
        "{INTO_TYPED_PRELUDE}\nloop Typed {{\nloop::flow(\n    loop::manual(),\n    loop::find(:as(deps), :from(Deps.list), :max(5)),\n    loop::each(:in($deps), :as(dep), :max(5),\n        {read},\n        loop::write(:to(Stats.create), :key(\"s:{{$dep.id}}\"), :value({value}))\n    )\n)\n}}\n"
    );
    let program = parse(&src).map_err(|e| format!("parse: {}", e.message))?;
    program.validate()
}

fn expect_into_write_err(read: &str, value: &str, needles: &[&str]) {
    match check_into_write(read, value) {
        Ok(()) => panic!("expected a type error, but it validated: {value}"),
        Err(e) => {
            for needle in needles {
                assert!(e.contains(needle), "expected `{needle}` in error, got: {e}");
            }
        }
    }
}

#[test]
fn into_row_fields_take_the_contract_types_without_coercion() {
    let hit = r#"loop::read(:as(hit), :into(Hit), scrape::page(:url($dep.changelog), :select("article")))"#;
    let release = r#"loop::read(:as(release), :into(Release), scrape::page(:url($dep.changelog), :select("article")))"#;
    check_into_write(hit, "Stat.new(:code($hit.status), :label($hit.title))").unwrap();
    check_into_write(release, "Stat.new(:label($release.version))").unwrap();
    expect_into_write_err(
        release,
        "Stat.new(:code($release.version))",
        &["field `code`", "expects Int", "`$release.version` is Str"],
    );
    expect_into_write_err(
        hit,
        "Stat.new(:label($hit.status))",
        &["field `label`", "expects Str", "`$hit.status` is Int"],
    );
    let fallback = r#"loop::fallback(:as(hit), :accept($hit.status == 200),
            loop::read(:into(Hit), scrape::page(:url($dep.changelog), :select("article"))),
            loop::read(:into(Hit), scrape::page(:url($dep.changelog), :select("main"))),
            loop::otherwise(loop::skip(:reason("no page"))))"#;
    check_into_write(fallback, "Stat.new(:code($hit.status))").unwrap();
    expect_into_write_err(
        fallback,
        "Stat.new(:label($hit.status))",
        &["field `label`", "expects Str", "is Int"],
    );
}

// ---------------------------------------------------------------- embedding steps (THO-552)

const EMBED_PRELUDE: &str = r#"
@version("0.8")
subset Emb384 of Vec[num32; 384];
subset Emb768 of Vec[num32; 768];
contract Dependency { has Str $.id; has Str $.changelog; }
contract Source { has Str $.id; has Str $.url; }
contract Release { has Str $.version; has Str $.text; }
contract Article { has Str $.title; has Str $.text; }
contract Draft { has Str $.text; }
contract Incident { has Str $.id; has Str $.title; has Emb384 $.vector_embedding; }
contract WideIncident { has Str $.title; has Emb768 $.vector_embedding; }
resource Dependencies for Dependency {
    query list;
}
resource Sources for Source {
    query list;
}
resource Incidents for Incident {
    query list;
    query nearest;
    mutation create;
}
resource WideIncidents for WideIncident {
    mutation create;
}
"#;

/// The site's UpstreamWatch embedding line, verbatim.
const SITE_RELEASE_EMBED: &str = r#"loop::let(:as(vec),
  tensor::infer(:model("minilm-l6-v2"), :from($release.text)))"#;

/// The site's ArticleIngest embedding line, verbatim.
const SITE_ARTICLE_EMBED: &str = r#"loop::let(:as(vec),
  tensor::infer(:model("minilm-l6-v2"), :from($article.text)))"#;

const WRITE_INCIDENT: &str = r#"loop::write(:to(Incidents.create), :key("{$dep.id}:{$release.version}"),
  :value(Incident.new(:title($release.version), :vector_embedding($vec))))"#;

fn embed_src(steps: &str) -> String {
    format!(
        r#"{EMBED_PRELUDE}
loop UpstreamWatch {{
loop::flow(
    loop::schedule(:cron("0 6 * * *"), :tz("UTC")),
    loop::find(:as(deps), :from(Dependencies.list), :max(20)),
    loop::each(:in($deps), :as(dep), :max(20),
        {RELEASE_READ},
        {steps}
    )
)
}}
"#
    )
}

fn check_embed(steps: &str) -> Result<sil_core::Program, String> {
    check_embed_src(&embed_src(steps))
}

fn check_embed_src(src: &str) -> Result<sil_core::Program, String> {
    let program = parse(src).map_err(|e| format!("parse: {}", e.message))?;
    program.validate()?;
    Ok(program)
}

fn expect_embed_err(steps: &str, needles: &[&str]) {
    match check_embed(steps) {
        Ok(_) => panic!("expected an error, but it validated:\n{steps}"),
        Err(e) => {
            for needle in needles {
                assert!(e.contains(needle), "expected `{needle}` in error, got: {e}");
            }
        }
    }
}

#[test]
fn site_upstreamwatch_embed_parses_validates_and_plans_the_worker() {
    let program = check_embed(&format!("{SITE_RELEASE_EMBED},\n{WRITE_INCIDENT}")).unwrap();
    let graph = sil_core::infer_graph(&program).unwrap().unwrap();
    assert!(graph.needs_embed_worker());
    let plan = graph.embedding.unwrap();
    assert_eq!(plan.model.id, "minilm-l6-v2");
    assert_eq!(plan.model.dimension, 384);
    assert!(plan.feeds.is_empty());
    assert_eq!(plan.nearest.len(), 1, "Incidents keeps `query nearest`");
    assert_eq!(plan.loop_embeds.len(), 1);
    assert_eq!(plan.loop_embeds[0].loop_name, "UpstreamWatch");
    assert_eq!(plan.loop_embeds[0].binding, "vec");
    let steps = sil_core::loop_embed_steps(&program.loops[0]);
    assert_eq!(steps.len(), 1);
    assert_eq!(
        sil_core::loop_embed_type(sil_core::loop_embed_model(steps[0]).unwrap()),
        sil_core::TypeExpr::Vec {
            elem: "num32".into(),
            len: Some(384)
        }
    );
}

#[test]
fn site_articleingest_embed_validates_and_writes_an_emb384_field() {
    let src = format!(
        r#"{EMBED_PRELUDE}
loop ArticleIngest {{
loop::flow(
    loop::schedule(:cron("0 * * * *"), :tz("UTC")),
    loop::find(:as(sources), :from(Sources.list), :max(50)),
    loop::each(:in($sources), :as(src), :max(50),
        {ARTICLE_READ},
        {SITE_ARTICLE_EMBED},
        loop::write(:to(Incidents.create), :key("{{$src.id}}:{{$article.title}}"),
          :value(Incident.new(:title($article.title), :vector_embedding($vec))))
    )
)
}}
"#
    );
    check_embed_src(&src).unwrap();
}

#[test]
fn embed_model_defaults_to_minilm_and_prefer_cpu_is_accepted() {
    check_embed(&format!(
        "loop::let(:as(vec), tensor::infer(:from($release.text), :prefer(CPU))),\n{WRITE_INCIDENT}"
    ))
    .unwrap();
}

#[test]
fn embed_unknown_model_is_rejected() {
    expect_embed_err(
        &SITE_RELEASE_EMBED.replace("minilm-l6-v2", "not-a-model"),
        &["loop::let", "unknown embedding model `not-a-model`"],
    );
}

#[test]
fn embed_prefer_cuda_is_rejected() {
    expect_embed_err(
        r#"loop::let(:as(vec), tensor::infer(:from($release.text), :prefer(CUDA)))"#,
        &["CPU-only", "CUDA"],
    );
    expect_embed_err(
        r#"loop::let(:as(vec), tensor::infer(:from($release.text), :prefer(TPU)))"#,
        &["only CPU is accepted"],
    );
}

#[test]
fn embed_vector_cannot_be_interpolated_into_a_template() {
    expect_embed_err(
        &format!(
            r#"{SITE_RELEASE_EMBED},
        loop::notify(:to("Maintainers"), :text("vector {{$vec}}"), :key("{{$dep.id}}:{{$release.version}}"))"#
        ),
        &[
            ":text",
            "`{$vec}` is a vector (Vec[num32; 384])",
            "cannot be interpolated",
        ],
    );
    expect_embed_err(
        &format!(
            r#"{SITE_RELEASE_EMBED},
        loop::notify(:to("Maintainers"), :text("new release"), :key("{{$dep.id}}:{{$vec}}"))"#
        ),
        &[":key", "is a vector"],
    );
    expect_embed_err(
        r#"loop::find(:as(hit), :from(Incidents.list), :one),
        loop::notify(:to("Maintainers"), :text("{$hit.vector_embedding}"), :key("{$dep.id}"))"#,
        &["`{$hit.vector_embedding}` is a vector"],
    );
}

#[test]
fn embed_cost_report_counts_embeds_apart_from_model_calls() {
    let program = check_embed(&format!("{SITE_RELEASE_EMBED},\n{WRITE_INCIDENT}")).unwrap();
    let b = sil_core::loop_bounds(&program.loops[0]);
    assert_eq!(b.embeds, 20, "one embed per item, scaled by :max(20)");
    assert_eq!(b.model_calls, 0, "embeds are not model calls");
    assert_eq!(b.reads, 60);
    let report = sil_core::format_loop_cost_report(&program).unwrap();
    assert!(report.contains("model calls 0"), "{report}");
    assert!(
        report.contains("embeds 20 (minilm-l6-v2, CPython embed worker; not model calls)"),
        "{report}"
    );

    let with_ask = check_embed(&format!(
        r#"{SITE_RELEASE_EMBED},
        loop::ask(:as(draft), :into(Draft), :from($release.text), :prompt("Summarize {{$release.version}}")),
        loop::gate(:that($draft.text != ""), :reason("never write an empty note")),
        {WRITE_INCIDENT}"#
    ))
    .unwrap();
    let b = sil_core::loop_bounds(&with_ask.loops[0]);
    assert_eq!((b.embeds, b.model_calls), (20, 40));

    let plain = check_embed("loop::skip(:reason(\"nothing\"))").unwrap();
    let report = sil_core::format_loop_cost_report(&plain).unwrap();
    assert!(!report.contains("embeds"), "{report}");
}

#[test]
fn embed_binding_is_a_vector_of_the_model_dimension() {
    expect_embed_err(
        &format!(
            r#"{SITE_RELEASE_EMBED},
        loop::write(:to(WideIncidents.create), :key("{{$dep.id}}"), :value(WideIncident.new(:title($release.version), :vector_embedding($vec))))"#
        ),
        &[
            "field `vector_embedding`",
            "Vec[num32; 768]",
            "`$vec` is Vec[num32; 384]",
        ],
    );
    expect_embed_err(
        &format!(
            r#"{SITE_RELEASE_EMBED},
        loop::write(:to(Incidents.create), :key("{{$dep.id}}"), :value(Incident.new(:title($vec))))"#
        ),
        &["field `title`", "expects Str", "`$vec` is Vec[num32; 384]"],
    );
    let src = embed_src(&format!(
        r#"loop::let(:as(vec), tensor::infer(:model("nomic-embed-text"), :from($release.text))),
        loop::write(:to(WideIncidents.create), :key("{{$dep.id}}"), :value(WideIncident.new(:title($release.version), :vector_embedding($vec))))"#
    ));
    check_embed_src(&src.replace("query nearest;", "")).unwrap();
    let err = check_embed_src(&src).unwrap_err();
    assert!(
        err.contains("has dimension 384 but model `nomic-embed-text` declares 768"),
        "{err}"
    );
}

#[test]
fn embed_from_must_be_a_typed_str() {
    expect_embed_err(
        r#"loop::let(:as(vec), tensor::infer(:from($release)))"#,
        &["embeds a Str", "`$release` is Release"],
    );
    expect_embed_err(
        r#"loop::let(:as(vec), tensor::infer(:model("minilm-l6-v2")))"#,
        &["requires `:from(...)`"],
    );
    expect_embed_err(
        r#"loop::let(:as(vec), tensor::infer(:from($nope.text)))"#,
        &["unknown binding `$nope`"],
    );
    check_embed(r#"loop::let(:as(vec), tensor::infer(:from($dep.id))), loop::skip(:reason("ok"))"#)
        .unwrap();
}

#[test]
fn let_takes_a_value_or_one_embed() {
    expect_embed_err(
        r#"loop::let(:as(vec), :value("x"), tensor::infer(:from($release.text)))"#,
        &["either `:value(...)` or a nested `tensor::infer(...)`"],
    );
    expect_embed_err(
        r#"loop::let(:as(vec), scrape::page(:url($dep.changelog)))"#,
        &["`scrape::page` is a read", "loop::read"],
    );
    expect_embed_err(
        r#"loop::let(:as(vec), :from($release.text))"#,
        &[":from belongs to a nested `tensor::infer(...)`"],
    );
    expect_embed_err(
        r#"loop::let(:as(vec))"#,
        &["requires `:value(...)` or a nested"],
    );
    expect_embed_err(
        r#"loop::read(:as(vec), tensor::infer(:from($release.text)))"#,
        &["`tensor::infer` is not a read", "loop::let"],
    );
}

#[test]
fn embed_of_unchecked_model_output_cannot_reach_an_effect() {
    expect_embed_err(
        &format!(
            r#"loop::ask(:as(draft), :into(Draft), :from($release.text), :prompt("Summarize {{$release.version}}")),
        loop::let(:as(vec), tensor::infer(:from($draft.text))),
        {WRITE_INCIDENT}"#
        ),
        &["model output from loop::ask"],
    );
}

#[test]
fn one_embedding_model_per_program() {
    let src = format!(
        "{}\nloop Other {{\nloop::flow(\n    loop::manual(),\n    loop::let(:as(v2), tensor::infer(:model(\"nomic-embed-text\"), :from(\"hello\"))),\n    loop::stop()\n)\n}}\n",
        embed_src(&format!("{SITE_RELEASE_EMBED},\n{WRITE_INCIDENT}"))
    );
    let err = check_embed_src(&src).unwrap_err();
    assert!(
        err.contains("one embedding model per program")
            && err.contains("minilm-l6-v2")
            && err.contains("nomic-embed-text"),
        "{err}"
    );
}

#[test]
fn text_score_still_cannot_share_a_program_with_a_loop_embed() {
    let src = format!(
        "{}\nprocessor Scorer {{\n    method score(Release $r) {{\n        $r.text ==> text::score()\n    }}\n}}\n",
        embed_src(&format!("{SITE_RELEASE_EMBED},\n{WRITE_INCIDENT}"))
    );
    let err = check_embed_src(&src).unwrap_err();
    assert!(
        err.contains("cannot mix text::score with tensor::infer"),
        "{err}"
    );
}

// ------------------------------------------- site, render, select, score (THO-579)

fn check_scrape_fixture() -> sil_core::Program {
    let program = parse(include_str!(
        "../../sil-codegen/tests/fixtures/loop_scrape_score.silc"
    ))
    .expect("parse fixture");
    program.validate().expect("validate fixture");
    program
}

#[test]
fn site_render_select_score_and_ask_share_one_each() {
    let program = check_scrape_fixture();
    let graph = sil_core::infer_graph(&program)
        .expect("infer graph")
        .expect("runnable graph");
    assert!(graph.needs_llm(), "loop::ask still provisions the model");
    assert!(
        graph.needs_loop_browser(),
        "scrape::render needs Playwright"
    );
}

#[test]
fn scrape_cost_report_counts_the_site_cap_and_lists_scores() {
    let program = check_scrape_fixture();
    let b = sil_core::loop_bounds(&program.loops[0]);
    // Per item: site 4 pages × (1 + :retry(1)), render and select 1 + 2 retries.
    assert_eq!(b.reads, 10 * (4 * 2 + 3 + 3));
    assert_eq!(b.scores, 10);
    assert_eq!(b.model_calls, 10 * 2, "scores are not model calls");
    let report = sil_core::format_loop_cost_report(&program).unwrap();
    assert!(
        report.contains("model calls 20 ·") && report.contains("reads 140 · scores 10 ·"),
        "{report}"
    );
}

const SITE_READ: &str = r#"loop::read(:as(pages), scrape::site(:url($dep.changelog), :max(5)))"#;

fn notify_after(text: &str) -> String {
    format!(r#"loop::notify(:to("Maintainers"), :text("{text}"), :key("{{$dep.id}}"))"#)
}

#[test]
fn scrape_reads_bind_lists_with_counts() {
    check_page(&page_loop(SITE_READ, &notify_after("{$pages.count} pages"))).unwrap();
    check_page(&page_loop(
        r#"loop::read(:as(pages), scrape::site(:url($dep.changelog), :max(5), :depth(3), :same_host(false), :link_css("nav a"), :select("main")))"#,
        r#"loop::each(:in($pages), :as(p), :max(5), loop::notify(:to("Maintainers"), :text("{$p.title}"), :key("{$p.url}")))"#,
    ))
    .unwrap();
    check_page(&page_loop(
        r#"loop::read(:as(links), scrape::select(:url($dep.changelog), :css("a.release"), :max(3)))"#,
        r#"loop::each(:in($links), :as(l), :max(3), loop::notify(:to("Maintainers"), :text("{$l.text}"), :key("{$l.href}")))"#,
    ))
    .unwrap();
    check_page(&page_loop(
        r#"loop::read(:as(status), :into(Article), scrape::render(:url($dep.changelog), :select("main")))"#,
        &notify_after("{$status.title}"),
    ))
    .unwrap();
}

#[test]
fn scrape_site_requires_a_page_cap() {
    expect_page_err(
        &page_loop(
            r#"loop::read(:as(pages), scrape::site(:url($dep.changelog)))"#,
            &notify_after("x"),
        ),
        "`scrape::site` requires :max(N)",
    );
    expect_page_err(
        &page_loop(
            r#"loop::read(:as(pages), scrape::site(:url($dep.changelog), :max(500)))"#,
            &notify_after("x"),
        ),
        "between 1 and 100",
    );
}

#[test]
fn scrape_select_requires_css_and_max() {
    expect_page_err(
        &page_loop(
            r#"loop::read(:as(links), scrape::select(:url($dep.changelog), :max(3)))"#,
            &notify_after("x"),
        ),
        ":css",
    );
    expect_page_err(
        &page_loop(
            r#"loop::read(:as(links), scrape::select(:url($dep.changelog), :css("a")))"#,
            &notify_after("x"),
        ),
        ":max",
    );
}

#[test]
fn scrape_options_name_the_reads_they_apply_to() {
    expect_page_err(
        &page_loop(
            r#"loop::read(:as(p), scrape::page(:url($dep.changelog), :depth(2)))"#,
            &notify_after("x"),
        ),
        "`scrape::site`",
    );
    expect_page_err(
        &page_loop(
            r#"loop::read(:as(p), scrape::render(:url($dep.changelog), :css("a")))"#,
            &notify_after("x"),
        ),
        "`scrape::select`",
    );
}

#[test]
fn scrape_ops_outside_loop_read_are_rejected() {
    expect_page_err(
        &page_loop(
            r#"loop::let(:as(pages), scrape::site(:url($dep.changelog), :max(5)))"#,
            &notify_after("x"),
        ),
        "`scrape::site` is a read; inside a loop it must sit in loop::read",
    );
    expect_page_err(
        &page_loop(
            r#"loop::read(:as(s), text::score(:text($dep.changelog)))"#,
            &notify_after("x"),
        ),
        "`text::score` is not a read",
    );
}

#[test]
fn text_score_binds_a_number() {
    check_page(&page_loop(
        r#"loop::read(:as(page), scrape::page(:url($dep.changelog)))"#,
        r#"loop::let(:as(quality), text::score(:text($page.text))),
        loop::gate(:that($quality > 0.5), :reason("readable"), loop::otherwise(loop::skip(:reason("noise")))),
        loop::notify(:to("Maintainers"), :text("{$quality}"), :key("{$dep.id}"))"#,
    ))
    .unwrap();
    expect_page_err(
        &page_loop(
            r#"loop::read(:as(page), scrape::page(:url($dep.changelog)))"#,
            r#"loop::let(:as(quality), text::score(:text($page))),
        loop::notify(:to("Maintainers"), :text("x"), :key("{$dep.id}"))"#,
        ),
        "text::score",
    );
}

#[test]
fn text_score_cannot_score_unchecked_model_output() {
    expect_page_err(
        &page_loop(
            r#"loop::ask(:as(draft), :into(Draft), :from($dep), :prompt("Draft"))"#,
            r#"loop::let(:as(quality), text::score(:text($draft.text))),
        loop::notify(:to("Maintainers"), :text("x"), :key("{$dep.id}"))"#,
        ),
        "`text::score` cannot score",
    );
    check_page(&page_loop(
        r#"loop::ask(:as(draft), :into(Draft), :from($dep), :prompt("Draft"))"#,
        r#"loop::gate(:that($draft.text != ""), :reason("never empty")),
        loop::let(:as(quality), text::score(:text($draft.text))),
        loop::notify(:to("Maintainers"), :text("{$quality}"), :key("{$dep.id}"))"#,
    ))
    .unwrap();
}

#[test]
fn scrape_pipelines_still_cannot_share_a_program_with_loops() {
    let src = format!(
        "{PRELUDE}{PAGE_PRELUDE}\nservice Grab {{\n  method run() {{\n    target_url ==> scrape::page(:depth(1))\n  }}\n}}\nloop Watch {{\n{}\n}}\n",
        page_loop(SITE_READ, &notify_after("x"))
    );
    let err = parse(&src).unwrap().validate().unwrap_err();
    assert!(
        err.contains("inside a loop, put scrape::page, scrape::site"),
        "{err}"
    );
}

// ---------------------------------------------------------------- find nearest (THO-553)

const NEAREST_PRELUDE: &str = r#"
@version("0.8")
subset Emb384 of Vec[num32; 384];
subset Emb128 of Vec[num32; 128];
contract Dependency { has Str $.id; has Str $.name; has Str $.changelog; has Emb384 $.vector_embedding; has Emb128 $.small; }
contract Release { has Str $.version; has Str $.text; }
contract Incident { has Str $.id; has Str $.summary; has Str $.status; has Emb384 $.vector_embedding; }
contract Impact { has Str $.version; has Str $.risk; has Str $.summary; }
contract Ticket { has Str $.id; has Str $.title; has Str $.risk; }
resource Dependencies for Dependency {
    query list;
}
resource Incidents for Incident {
    query list;
    query nearest;
}
resource Archive for Incident {
    query list;
}
resource Tickets for Ticket {
    query list;
    mutation create;
}
"#;

/// The site's UpstreamWatch nearest-match line, verbatim.
const SITE_NEAREST_FIND: &str = r#"loop::find(:as(seen), :max(5),
  :from(Incidents.nearest(:to($vec)))),"#;

/// The UpstreamWatch loop around a nearest find. `$vec` comes from a stored
/// Emb384 here; the loop embedding step binds it from `$release.text`.
fn nearest_loop(find: &str) -> String {
    format!(
        r#"loop::flow(
    loop::schedule(:cron("0 7 * * 1-5"), :tz("America/Chicago")),
    loop::find(:as(deps), :from(Dependencies.list), :max(40)),
    loop::each(:in($deps), :as(dep), :max(40),
      loop::read(:as(release), :into(Release),
        scrape::page(:url($dep.changelog), :select("article"))),
      loop::let(:as(vec), :value($dep.vector_embedding)),
      {find}
      loop::ask(:as(impact), :into(Impact), :retry(2),
        :from($release, $seen),
        :prompt("Does {{$release.version}} repeat any of these {{$seen.count}} incidents?"),
        loop::otherwise(loop::skip(:reason("no answer that fits Impact")))),
      loop::gate(:that($impact.risk != "none"),
        :reason("only real risk reaches a person"),
        loop::otherwise(loop::skip(:reason("no risk")))),
      loop::write(:to(Tickets.create),
        :key("upgrade:{{$dep.name}}:{{$release.version}}"),
        :value(Ticket.new(:title($impact.summary), :risk($impact.risk))))
    )
)"#
    )
}

fn nearest_program(find: &str) -> Result<sil_core::Program, String> {
    let src = format!(
        "{NEAREST_PRELUDE}\nloop UpstreamWatch {{\n{}\n}}\n",
        nearest_loop(find)
    );
    let program = parse(&src).map_err(|e| format!("parse: {}", e.message))?;
    program.validate()?;
    Ok(program)
}

fn expect_nearest_err(find: &str, needle: &str) {
    match nearest_program(find) {
        Ok(_) => panic!("expected error containing `{needle}`, but it validated:\n{find}"),
        Err(e) => assert!(e.contains(needle), "expected `{needle}` in error, got: {e}"),
    }
}

#[test]
fn site_nearest_find_parses_validates_and_builds() {
    let program = nearest_program(SITE_NEAREST_FIND).unwrap();
    let graph = sil_core::infer_graph(&program)
        .unwrap()
        .expect("runnable graph");
    let plan = graph
        .embedding
        .expect("query nearest needs the embed worker");
    assert_eq!(plan.nearest[0].table, "incidents");

    let each = &program.loops[0].steps()[1];
    let find = &each.children[2];
    assert_eq!(find.ref_prop("from"), Some(("Incidents", "nearest")));
    assert_eq!(find.nearest_to(), Some(&sil_core::Expr::Var("vec".into())));

    let b = sil_core::loop_bounds(&program.loops[0]);
    assert_eq!(
        b.rows_scanned,
        40 + 40 * 5,
        "rows scanned = :max per call, scaled by each"
    );
    assert_eq!(b.model_calls, 40 * 3, "a nearest find is not a model call");
}

#[test]
fn nearest_find_keeps_where_one_and_list_binding() {
    nearest_program(
        r#"loop::find(:as(seen), :max(5), :from(Incidents.nearest(:to($vec))), :where($.status != "closed")),"#,
    )
    .unwrap();
    // `:one` binds a row, so `$seen.count` no longer applies.
    expect_nearest_err(
        r#"loop::find(:as(seen), :one, :from(Incidents.nearest(:to($vec)))),"#,
        "`$seen` has no field `count`",
    );
    expect_nearest_err(
        r#"loop::find(:as(seen), :max(5), :from(Incidents.nearest(:to($vec))), :where($.severity == "high")),"#,
        "unknown binding `$severity`",
    );
}

#[test]
fn nearest_needs_query_nearest_on_the_resource() {
    expect_nearest_err(
        &SITE_NEAREST_FIND.replace("Incidents.nearest", "Archive.nearest"),
        "`Archive` does not declare `query nearest;`",
    );
}

#[test]
fn nearest_to_must_be_an_embedding_of_the_stored_dimension() {
    expect_nearest_err(
        &SITE_NEAREST_FIND.replace("$vec", "$dep.name"),
        "`:to` must be a 384-float embedding (from the loop embedding step), but `$dep.name` is Str",
    );
    expect_nearest_err(
        &SITE_NEAREST_FIND.replace("$vec", "$release"),
        "but `$release` is Release",
    );
    expect_nearest_err(
        &SITE_NEAREST_FIND.replace("$vec", "$dep.small"),
        "must be a 384-float embedding to match `Incident.vector_embedding`, but `$dep.small` has 128 floats",
    );
    expect_nearest_err(
        &SITE_NEAREST_FIND.replace("$vec", "$missing"),
        "unknown binding `$missing`",
    );
    expect_nearest_err(&SITE_NEAREST_FIND.replace("$vec", "\"text\""), "is Str");
}

#[test]
fn nearest_requires_max() {
    expect_nearest_err(
        &SITE_NEAREST_FIND.replace(":max(5),", ""),
        "requires `:max(N)` (or `:one`)",
    );
    expect_nearest_err(
        &SITE_NEAREST_FIND.replace(":max(5)", ":max(10001)"),
        ":max must be between 1 and 10000",
    );
}

#[test]
fn nearest_rejects_order_and_desc() {
    expect_nearest_err(
        &SITE_NEAREST_FIND.replace(":max(5),", ":max(5), :order($.summary),"),
        ":order does not apply to `Incidents.nearest`",
    );
    expect_nearest_err(
        &SITE_NEAREST_FIND.replace(":max(5),", ":max(5), :desc,"),
        ":desc does not apply to `Incidents.nearest`",
    );
}

#[test]
fn nearest_takes_exactly_one_to() {
    for (find, needle) in [
        ("Incidents.nearest", "`Incidents.nearest` needs `:to(...)`"),
        (
            "Incidents.nearest()",
            "`Incidents.nearest` needs `:to(...)`",
        ),
        ("Incidents.nearest($vec)", "found a positional argument"),
        (
            "Incidents.nearest(:near($vec))",
            "found unknown option `:near`",
        ),
        (
            "Incidents.nearest(:to($vec), :to($vec))",
            "takes `:to(...)` once",
        ),
    ] {
        expect_nearest_err(
            &SITE_NEAREST_FIND.replace("Incidents.nearest(:to($vec))", find),
            needle,
        );
    }
}

#[test]
fn only_nearest_takes_options() {
    expect_nearest_err(
        &SITE_NEAREST_FIND.replace("Incidents.nearest(:to($vec))", "Incidents.list(:to($vec))"),
        "`Incidents.list` takes no options",
    );
    let write = nearest_loop(SITE_NEAREST_FIND)
        .replace(":to(Tickets.create)", ":to(Tickets.create(:to($vec)))");
    let src = format!("{NEAREST_PRELUDE}\nloop UpstreamWatch {{\n{write}\n}}\n");
    let err = parse(&src).unwrap().validate().unwrap_err();
    assert!(err.contains("`Tickets.create` takes no options"), "{err}");
    // The option form is only a resource reference; it is not a value.
    let as_value = nearest_loop(SITE_NEAREST_FIND).replace(
        ":value($dep.vector_embedding)",
        ":value(Incidents.nearest(:to($dep.vector_embedding)))",
    );
    let src = format!("{NEAREST_PRELUDE}\nloop UpstreamWatch {{\n{as_value}\n}}\n");
    let err = parse(&src).unwrap().validate().unwrap_err();
    assert!(err.contains("calls are not allowed"), "{err}");
}
