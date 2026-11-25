//! Static loop rules (ADR-014): each negative fixture must fail with a
//! diagnostic that names the rule.

use sil_parser::parse;

const PRELUDE: &str = r#"
@version("0.7.1")
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

#[test]
fn ask_cannot_mix_with_text_score() {
    let src = format!(
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
    }}
}}
loop Chase {{
{GOOD}
}}
"#
    );
    let err = parse(&src).unwrap().validate().unwrap_err();
    assert!(
        err.contains("cannot mix") && err.contains("text::score") && err.contains("loop::ask"),
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
@version("0.7.1")
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
