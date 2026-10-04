//! Smoke coverage for the `files` capability (ADR-018): the synthesized
//! `/files/list` and `/files/download` routes over a sysop-shared directory,
//! including folder zips and path-traversal rejection.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn silc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_silc"))
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/files_app")
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), dest).unwrap();
        }
    }
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

fn get(url: &str) -> (u16, String) {
    let response = ureq::get(url).call().expect(&format!("GET {url}"));
    let status = response.status();
    let body = response.into_string().unwrap_or_default();
    (status, body)
}

fn get_bytes(url: &str) -> (u16, Vec<u8>, String) {
    let response = ureq::get(url).call().expect(&format!("GET {url}"));
    let status = response.status();
    let disposition = response
        .header("content-disposition")
        .unwrap_or("")
        .to_string();
    let mut bytes = Vec::new();
    response.into_reader().read_to_end(&mut bytes).expect("read");
    (status, bytes, disposition)
}

#[test]
fn files_app_lists_downloads_and_rejects_traversal() {
    let port = 18136u16;
    free_port(port);

    // Copy the fixture so the symlink-escape probe never touches the repo.
    let temp = std::env::temp_dir().join(format!(
        "silc-files-e2e-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let dir = temp.join("app");
    copy_dir(&fixture(), &dir);
    let log_path = temp.join("run.log");
    let log = std::fs::File::create(&log_path).unwrap();

    let mut child = Command::new(silc_bin())
        .current_dir(&dir)
        .arg("main.silc")
        .env("SILC_HTTP_PORT", port.to_string())
        .env("SILC_TERMINAL_PORT", "0")
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("spawn files app");

    let started = Instant::now();
    let mut healthy = false;
    while started.elapsed() < Duration::from_secs(180) {
        if let Ok(resp) = ureq::get(&format!("http://127.0.0.1:{port}/health")).call() {
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
        thread::sleep(Duration::from_millis(400));
    }
    assert!(
        healthy,
        "files app never became healthy\n{}",
        std::fs::read_to_string(&log_path).unwrap_or_default()
    );

    let base = format!("http://127.0.0.1:{port}");

    // Root listing: directories first, dot-files hidden.
    let (status, body) = get(&format!("{base}/files/list"));
    assert_eq!(status, 200, "{body}");
    let listing: serde_json::Value = serde_json::from_str(&body).expect(&body);
    assert_eq!(listing["path"], "");
    assert!(listing["parent"].is_null());
    let names: Vec<&str> = listing["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["textfiles", "utils", "README.txt"], "{body}");
    assert!(!body.contains(".secret"), "dot-files must be hidden: {body}");

    // Subfolder listing.
    let (status, body) = get(&format!("{base}/files/list?path=textfiles"));
    assert_eq!(status, 200, "{body}");
    let sub: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(sub["path"], "textfiles");
    assert_eq!(sub["parent"], "");
    assert!(body.contains("bbs-history.txt"), "{body}");
    assert!(body.contains("house-rules.txt"), "{body}");

    // File download streams the bytes with an attachment disposition.
    let (status, bytes, disposition) = get_bytes(&format!("{base}/files/download?path=README.txt"));
    assert_eq!(status, 200);
    assert_eq!(bytes, b"Welcome to the fixture file library.\n");
    assert!(disposition.contains("attachment"), "{disposition}");
    assert!(disposition.contains("README.txt"), "{disposition}");

    // Folder download is a zip holding the folder's files.
    let (status, bytes, disposition) =
        get_bytes(&format!("{base}/files/download?path=textfiles"));
    assert_eq!(status, 200);
    assert!(disposition.contains("textfiles.zip"), "{disposition}");
    assert!(bytes.starts_with(b"PK"), "expected zip bytes");
    let zip_path = temp.join("textfiles.zip");
    std::fs::write(&zip_path, &bytes).unwrap();
    let listed = Command::new("unzip")
        .args(["-l", zip_path.to_str().unwrap()])
        .output()
        .expect("unzip");
    let listing = String::from_utf8_lossy(&listed.stdout);
    assert!(listing.contains("bbs-history.txt"), "{listing}");
    assert!(listing.contains("house-rules.txt"), "{listing}");
    assert!(!listing.contains(".secret"), "{listing}");

    // A single file can be zipped on demand too.
    let (status, bytes, disposition) =
        get_bytes(&format!("{base}/files/download?path=utils/hello.sh&zip=1"));
    assert_eq!(status, 200);
    assert!(disposition.contains("hello.sh.zip"), "{disposition}");
    assert!(bytes.starts_with(b"PK"));

    // Path traversal and hidden names are not found.
    for bad in ["..", "../..", "textfiles/../../", ".secret", "textfiles/../.secret"] {
        let response = ureq::get(&format!(
            "{base}/files/download?path={}",
            urlencoding_minimal(bad)
        ))
        .call();
        let status = match response {
            Ok(resp) => resp.status(),
            Err(ureq::Error::Status(code, _)) => code,
            Err(other) => panic!("unexpected error for {bad}: {other}"),
        };
        assert_eq!(status, 404, "expected 404 for {bad}");
    }

    // A symlink escaping the share is not served.
    let outside = temp.join("outside.txt");
    std::fs::write(&outside, b"secret outside").unwrap();
    let link = dir.join("files/escape");
    let _ = std::os::unix::fs::symlink(&outside, &link);
    let response = ureq::get(&format!("{base}/files/download?path=escape")).call();
    let status = match response {
        Ok(resp) => resp.status(),
        Err(ureq::Error::Status(code, _)) => code,
        Err(other) => panic!("unexpected error: {other}"),
    };
    assert_eq!(status, 404, "symlink escape must be 404");
    let _ = std::fs::remove_file(&link);

    let _ = child.kill();
    let _ = child.wait();
    free_port(port);
}

/// Enough percent-encoding for the traversal probes above.
fn urlencoding_minimal(s: &str) -> String {
    s.replace("%", "%25")
        .replace(" ", "%20")
        .replace("?", "%3F")
        .replace("#", "%23")
        .replace("/", "%2F")
}
