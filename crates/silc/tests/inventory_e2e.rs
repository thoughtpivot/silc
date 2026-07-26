use std::io::{Read, Write};
use std::net::TcpStream;
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

fn read_until(stream: &mut TcpStream, needle: &str, timeout: Duration) -> String {
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    let start = Instant::now();
    let mut out = Vec::new();
    let mut buffer = [0u8; 2048];
    while start.elapsed() < timeout {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&buffer[..n]);
                if String::from_utf8_lossy(&out).contains(needle) {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => panic!("terminal read failed: {error}"),
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn inventory_app_builds_with_chat_context() {
    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/inventoryApp/main.silc");
    let build = Command::new(silc_bin())
        .args(["build", example.to_str().unwrap()])
        .output()
        .expect("build");
    assert!(
        build.status.success(),
        "build failed: {}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/inventoryApp/.runtime/main");
    let app = std::fs::read_to_string(root.join("typescript/src/App.tsx")).unwrap();
    assert!(
        app.contains("context: items"),
        "assistant chat must send live inventory as context"
    );
    assert!(
        app.contains("persona: \"You are the Inventory Assistant"),
        "assistant chat must declare its inventory persona"
    );
    assert!(app.contains("/admin") && app.contains("/assistant"));
    assert!(
        app.contains("<DataTable rows={items}")
            && app.contains("filterValue={category_filter}")
            && app.contains("filterColumn={\"category\"}"),
        "browse page must render the inventory as a filterable data grid"
    );
    assert!(
        app.contains("<Section") && app.contains("<Alert"),
        "browse page must use section/alert primitives"
    );
    let terminal_app =
        std::fs::read_to_string(root.join("typescript/src/TerminalApp.tsx")).unwrap();
    assert!(
        terminal_app.contains("__silcNavigate")
            && terminal_app.contains("DataTable")
            && terminal_app.contains("<Section")
            && terminal_app.contains("<Alert"),
        "OpenTUI TerminalApp must mirror browse section/alert/table"
    );
    let pkg = std::fs::read_to_string(root.join("typescript/package.json")).unwrap();
    assert!(
        pkg.contains("@opentui/core"),
        "inventory runtime must pin OpenTUI"
    );
    assert!(
        app.contains("filterColumn={\"category\"} sortable searchable />"),
        "browse grid must opt into sorting and fuzzy search"
    );
    let data_table =
        std::fs::read_to_string(root.join("typescript/src/components/ui/data-table.tsx")).unwrap();
    assert!(
        data_table.contains("toggleSort")
            && data_table.contains("compareValues")
            && data_table.contains("aria-sort")
            && data_table.contains("sortable = false"),
        "DataTable must support opt-in per-column sorting"
    );
    assert!(
        data_table.contains("levenshtein")
            && data_table.contains("fuzzyMatches")
            && data_table.contains("searchable = false"),
        "DataTable must support opt-in Levenshtein fuzzy search"
    );
    assert!(
        app.contains("await fetch(\"/api/inventory_items\", { method: \"POST\"")
            && app.contains("`/api/inventory_items/${")
            && !app.contains("\"/api/inventory\""),
        "admin mutations must target the resource's declared table route"
    );
    let worker = std::fs::read_to_string(root.join("typescript/worker.ts")).unwrap();
    assert!(worker.contains("normalizeContext") && worker.contains("inventory_items"));
    assert!(worker.contains("normalizePersona"));
    let py = std::fs::read_to_string(root.join("python/worker.py")).unwrap();
    assert!(py.contains("compose_llm_prompt") && py.contains("APPLICATION CONTEXT"));
    assert!(
        py.contains("SILCLM_IDENTITY") && py.contains("ASSISTANT ROLE"),
        "silclm worker must compose persona over the fixed silclm identity"
    );
    let manifest = std::fs::read_to_string(root.join("manifest.json")).unwrap();
    assert!(manifest.contains("silclm"));
    assert!(manifest.contains("inventory_items"));
    assert!(!manifest.contains("portal_kind"));
}

#[test]
fn inventory_app_resources_web_and_terminal_e2e() {
    free_port(18096);
    free_port(18097);
    let temp = std::env::temp_dir().join(format!(
        "silc-inventory-e2e-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&temp).unwrap();

    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/inventoryApp/main.silc");
    let build = Command::new(silc_bin())
        .args(["build", example.to_str().unwrap()])
        .output()
        .expect("build");
    assert!(
        build.status.success(),
        "build failed: {}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/inventoryApp/.runtime/main/data");
    for name in ["app.db", "app.db-shm", "app.db-wal"] {
        let _ = std::fs::remove_file(data_dir.join(name));
    }

    let log_path = temp.join("run.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut child = Command::new(silc_bin())
        .arg(example.to_str().unwrap())
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("run");

    let ready = Instant::now();
    let mut healthy = false;
    while ready.elapsed() < Duration::from_secs(300) {
        if let Ok(resp) = ureq::get("http://127.0.0.1:18096/health").call() {
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
    assert!(healthy, "inventory health never became ready");

    let page = ureq::get("http://127.0.0.1:18096/")
        .call()
        .expect("get ui")
        .into_string()
        .unwrap();
    assert!(page.contains("app.js") || page.contains("<!DOCTYPE html"));

    let list = ureq::get("http://127.0.0.1:18096/api/inventory_items")
        .call()
        .expect("list inventory")
        .into_string()
        .unwrap();
    assert!(
        list.trim() == "[]" || list.contains("[]"),
        "expected empty inventory: {list}"
    );

    let created = ureq::post("http://127.0.0.1:18096/api/inventory_items")
        .set("content-type", "application/json")
        .send_string(
            r#"{"name":"USB Cable","category":"Electronics","location":"Aisle B","quantity":"4","reorder_level":"10","notes":"low stock"}"#,
        )
        .expect("create item")
        .into_string()
        .unwrap();
    assert!(created.contains("USB Cable"), "create failed: {created}");

    let list2 = ureq::get("http://127.0.0.1:18096/api/inventory_items")
        .call()
        .expect("list again")
        .into_string()
        .unwrap();
    assert!(
        list2.contains("USB Cable") && list2.contains("Electronics"),
        "inventory missing item: {list2}"
    );

    let id = serde_json::from_str::<serde_json::Value>(&created)
        .ok()
        .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(str::to_string))
        .or_else(|| {
            // Fall back to listing for the id.
            let rows: Vec<serde_json::Value> = serde_json::from_str(&list2).unwrap_or_default();
            rows.first()
                .and_then(|row| row.get("id"))
                .and_then(|id| id.as_str())
                .map(str::to_string)
        })
        .expect("created item id");

    let updated = ureq::put(&format!("http://127.0.0.1:18096/api/inventory_items/{id}"))
        .set("content-type", "application/json")
        .send_string(
            r#"{"name":"USB Cable","category":"Electronics","location":"Aisle B","quantity":"12","reorder_level":"10","notes":"restocked"}"#,
        )
        .expect("update item")
        .into_string()
        .unwrap();
    assert!(
        updated.contains("restocked") || updated.contains("\"id\""),
        "update failed: {updated}"
    );

    let mut terminal = TcpStream::connect("127.0.0.1:18097").expect("connect terminal");
    let banner = read_until(&mut terminal, ">", Duration::from_secs(5));
    assert!(
        banner.contains("Silc") || banner.contains("/list"),
        "missing terminal help: {banner:?}"
    );
    terminal.write_all(b"/list inventory_items\n").unwrap();
    let listed = read_until(&mut terminal, "USB", Duration::from_secs(5));
    assert!(listed.contains("USB"), "terminal list failed: {listed:?}");
    terminal.write_all(b"/quit\n").ok();

    let _ = ureq::delete(&format!("http://127.0.0.1:18096/api/inventory_items/{id}")).call();
    let list3 = ureq::get("http://127.0.0.1:18096/api/inventory_items")
        .call()
        .expect("list after delete")
        .into_string()
        .unwrap();
    assert!(
        !list3.contains("USB Cable"),
        "delete did not remove item: {list3}"
    );

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGKILL);
    }
    let _ = child.wait();
    for name in ["app.db", "app.db-shm", "app.db-wal"] {
        let _ = std::fs::remove_file(data_dir.join(name));
    }
    let _ = std::fs::remove_dir_all(temp);
}
