//! Smoke coverage for `doc::extract` upload → resource list (ADR-011).

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

fn wait_http(port: u16, timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
            let mut buf = Vec::new();
            let _ = stream.read_to_end(&mut buf);
            if String::from_utf8_lossy(&buf).contains("\"ok\":true") {
                return;
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    panic!("dataExtractorApp did not become healthy on {port}");
}

#[test]
fn data_extractor_app_emits_upload_pipeline() {
    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dataExtractorApp/main.silc");
    let build = Command::new(silc_bin())
        .args(["build", example.to_str().unwrap()])
        .env("SILC_HTTP_PORT", "18132")
        .env("SILC_TERMINAL_PORT", "18133")
        .output()
        .expect("build");
    assert!(
        build.status.success(),
        "build failed: {}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/dataExtractorApp/.runtime/main");
    let worker = std::fs::read_to_string(root.join("typescript/worker.ts")).unwrap();
    assert!(worker.contains("HAS_DOC = true") || worker.contains("const HAS_DOC = true"));
    assert!(worker.contains("/upload"));
    assert!(worker.contains("DOC_TABLE = \"documents\"") || worker.contains("const DOC_TABLE = \"documents\""));
    assert!(root.join("python/doc_extract_worker.py").is_file());
    assert!(root.join("python/doc_requirements.txt").is_file());

    let app = std::fs::read_to_string(root.join("typescript/src/App.tsx")).unwrap();
    assert!(app.contains("type=\"file\"") || app.contains("type='file'"));
    assert!(app.contains("/documents"));
}

#[test]
#[ignore = "starts full dual-surface runtime and installs doc extract wheels"]
fn data_extractor_upload_persists_extracted_row() {
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/dataExtractorApp");
    let port = 18134u16;
    free_port(port);

    let mut child = Command::new(silc_bin())
        .current_dir(&example)
        .arg("main.silc")
        .env("SILC_HTTP_PORT", port.to_string())
        .env("SILC_TERMINAL_PORT", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn dataExtractorApp");

    wait_http(port, Duration::from_secs(120));

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sil-codegen/templates/fixtures/doc/sample.txt");
    let upload = Command::new("curl")
        .args([
            "-sS",
            "-m",
            "30",
            "-F",
            &format!("upload=@{};filename=sample.txt", fixture.display()),
            &format!("http://127.0.0.1:{port}/upload"),
        ])
        .output()
        .expect("curl upload");
    assert!(
        upload.status.success(),
        "upload curl failed: {}",
        String::from_utf8_lossy(&upload.stderr)
    );
    let body = String::from_utf8_lossy(&upload.stdout);
    assert!(body.contains("\"ok\":true"), "upload response: {body}");
    assert!(body.contains("Fixture Title"), "upload response: {body}");

    let list = Command::new("curl")
        .args([
            "-sS",
            "-m",
            "10",
            &format!("http://127.0.0.1:{port}/api/documents"),
        ])
        .output()
        .expect("curl list");
    let listed = String::from_utf8_lossy(&list.stdout);
    assert!(listed.contains("Fixture Title"), "list response: {listed}");

    let _ = child.kill();
    let _ = child.wait();
    free_port(port);
}
