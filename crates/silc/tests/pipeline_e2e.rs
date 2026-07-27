use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::thread;

fn tempdir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("silc-pipeline-e2e-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn silc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_silc"))
}

#[test]
#[ignore = "downloads pinned runtimes, MiniLM ONNX artifacts, and tensor wheels"]
fn local_http_to_normalized_embedding_to_sqlite() {
    let root = tempdir();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data_pipeline_runnable.silc");
    let entry = root.join("main.silc");
    fs::copy(fixture, &entry).unwrap();

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        let body =
            "<html><body><h1>Silc deterministic fixture</h1><p>Pipeline embedding text.</p></body></html>";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });

    let input = format!(r#"{{"url":"http://127.0.0.1:{port}/article"}}"#);
    let run = silc()
        .arg("run")
        .arg(&entry)
        .arg("--input-json")
        .arg(input)
        .output()
        .expect("run pipeline");
    assert!(
        run.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    server.join().unwrap();
    assert!(String::from_utf8_lossy(&run.stdout).contains("\"ok\":true"));

    let db = root.join(".runtime/main/data/app.db");
    assert!(db.is_file(), "missing generated SQLite database");
    let lock: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(".silc/runtimes.lock.json")).unwrap())
            .unwrap();
    let python = lock["python_bin"].as_str().unwrap();
    let query = Command::new(python)
        .arg("-c")
        .arg(
            "import sqlite3,sys; print(sqlite3.connect(sys.argv[1]).execute('select payload from embeddings limit 1').fetchone()[0])",
        )
        .arg(&db)
        .output()
        .unwrap();
    assert!(query.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(String::from_utf8_lossy(&query.stdout).trim().as_bytes()).unwrap();
    assert!(payload["raw_content"]
        .as_str()
        .unwrap()
        .contains("Silc deterministic fixture"));
    let embedding = payload["vector_embedding"].as_array().unwrap();
    assert_eq!(embedding.len(), 384);
    let norm = embedding
        .iter()
        .map(|value| value.as_f64().unwrap().powi(2))
        .sum::<f64>()
        .sqrt();
    assert!((norm - 1.0).abs() < 1e-4, "embedding norm was {norm}");

    let _ = fs::remove_dir_all(root);
}
