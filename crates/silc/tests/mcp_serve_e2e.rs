use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn silc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_silc"))
}

fn free_port(port: u16) {
    let _ = Command::new("bash")
        .args([
            "-c",
            &format!("lsof -ti:{port} | xargs kill -9 2>/dev/null || true"),
        ])
        .status();
    thread::sleep(Duration::from_millis(200));
}

fn mcp_post(port: u16, token: Option<&str>, body: &str) -> (u16, String) {
    let mut req = ureq::post(&format!("http://127.0.0.1:{port}/mcp"))
        .set("content-type", "application/json")
        .set("accept", "application/json, text/event-stream");
    if let Some(token) = token {
        req = req.set("authorization", &format!("Bearer {token}"));
    }
    match req.send_string(body) {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.into_string().unwrap_or_default();
            (status, text)
        }
        Err(ureq::Error::Status(status, resp)) => {
            let text = resp.into_string().unwrap_or_default();
            (status, text)
        }
        Err(err) => panic!("mcp post failed: {err}"),
    }
}

#[test]
fn talk_today_mcp_server_lists_calls_and_rejects_unauth() {
    const PORT: u16 = 18223;
    free_port(PORT);

    let temp = std::env::temp_dir().join(format!(
        "silc-mcp-e2e-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&temp).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sil-codegen/tests/fixtures/talk_today.silc");
    let entry = temp.join("main.silc");
    std::fs::copy(&fixture, &entry).expect("copy fixture");

    let build = Command::new(silc_bin())
        .args(["build", entry.to_str().unwrap()])
        .output()
        .expect("build");
    let build_out = format!(
        "{}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(build.status.success(), "build failed: {build_out}");
    assert!(
        build_out.contains("SILC_MCP_TOKEN")
            && build_out.contains("talks_list")
            && build_out.contains("talk_today_run"),
        "silc build must print MCP tools and auth env:\n{build_out}"
    );

    let log_path = temp.join("run.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut child = Command::new(silc_bin())
        .arg(entry.to_str().unwrap())
        .env("SILC_HTTP_PORT", PORT.to_string())
        .env("SILC_TERMINAL_PORT", "0")
        .env("SILC_MCP_TOKEN", "test-mcp-token")
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("run");

    let ready = Instant::now();
    let mut healthy = false;
    while ready.elapsed() < Duration::from_secs(300) {
        if let Ok(resp) = ureq::get(&format!("http://127.0.0.1:{PORT}/health")).call() {
            let body = resp.into_string().unwrap_or_default();
            if body.contains("\"ok\":true") || body.contains("\"ok\": true") {
                healthy = true;
                break;
            }
        }
        if let Ok(Some(status)) = child.try_wait() {
            let log = std::fs::read_to_string(&log_path).unwrap_or_default();
            panic!("silc exited early: {status}\n{log}");
        }
        thread::sleep(Duration::from_millis(500));
    }
    assert!(healthy, "talk_today health never became ready");

    let (unauth_status, unauth_body) = mcp_post(
        PORT,
        None,
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );
    assert_eq!(unauth_status, 401, "expected 401 without token: {unauth_body}");

    let (list_status, list_body) = mcp_post(
        PORT,
        Some("test-mcp-token"),
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
    );
    assert_eq!(list_status, 200, "tools/list failed: {list_body}");
    assert!(
        list_body.contains("talks_list") && list_body.contains("talk_today_run"),
        "tools/list missing expected tools: {list_body}"
    );

    let api_talks = ureq::get(&format!("http://127.0.0.1:{PORT}/api/talks"))
        .call()
        .expect("GET /api/talks")
        .into_string()
        .unwrap();
    let (call_status, call_body) = mcp_post(
        PORT,
        Some("test-mcp-token"),
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"talks_list","arguments":{}}}"#,
    );
    assert_eq!(call_status, 200, "talks_list call failed: {call_body}");
    let call_json: serde_json::Value = serde_json::from_str(&call_body).unwrap();
    let structured = &call_json["result"]["structuredContent"];
    let api_json: serde_json::Value = serde_json::from_str(&api_talks).unwrap();
    assert_eq!(
        structured, &api_json,
        "talks_list must match GET /api/talks"
    );

    let (run1_status, run1_body) = mcp_post(
        PORT,
        Some("test-mcp-token"),
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"talk_today_run","arguments":{"requested_by":"agent"}}}"#,
    );
    assert_eq!(run1_status, 200, "talk_today_run failed: {run1_body}");
    assert!(
        run1_body.contains("pending") || run1_body.contains("TalkToday"),
        "first run_now should create a request: {run1_body}"
    );

    let (run2_status, run2_body) = mcp_post(
        PORT,
        Some("test-mcp-token"),
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"talk_today_run","arguments":{"requested_by":"agent"}}}"#,
    );
    assert_eq!(run2_status, 200, "second talk_today_run failed: {run2_body}");
    assert!(
        run2_body.contains("no new run started") || run2_body.contains("already pending"),
        "second run_now must not start another: {run2_body}"
    );

    let requests = ureq::get(&format!("http://127.0.0.1:{PORT}/api/loop_requests"))
        .call()
        .expect("list loop_requests")
        .into_string()
        .unwrap();
    let req_rows: Vec<serde_json::Value> = serde_json::from_str(&requests).unwrap_or_default();
    let talk_reqs: Vec<_> = req_rows
        .iter()
        .filter(|r| r.get("loop").and_then(|v| v.as_str()) == Some("TalkToday"))
        .collect();
    assert_eq!(
        talk_reqs.len(),
        1,
        "exactly one TalkToday request expected: {requests}"
    );

    let (unknown_status, unknown_body) = mcp_post(
        PORT,
        Some("test-mcp-token"),
        r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"no_such_tool","arguments":{}}}"#,
    );
    assert!(
        unknown_status >= 400 || unknown_body.contains("unknown tool"),
        "unknown tool must error: {unknown_status} {unknown_body}"
    );

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGKILL);
    }
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(temp);
}
