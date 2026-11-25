"""Embed worker checks run by `embed_worker_python` (crates/sil-codegen/tests).

argv[1] is a generated `python/` directory holding embed_worker.py and
embedding_models.py. The OpenAI-compatible provider is checked against a
local mock server. sqlite-vec ranking runs only when `sqlite_vec` imports;
otherwise this prints `skip nearest` and the Rust test reports it.
"""
import json
import os
import sqlite3
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer

sys.path.insert(0, sys.argv[1])
import embedding_models  # noqa: E402


class MockEmbeddings(BaseHTTPRequestHandler):
    seen = []

    def log_message(self, *args):
        return

    def do_POST(self):  # noqa: N802
        body = json.loads(self.rfile.read(int(self.headers["content-length"])))
        MockEmbeddings.seen.append((self.path, self.headers.get("authorization"), body))
        width = 3 if body["model"] == "mock-3" else 2
        data = [
            {"index": i, "embedding": [float(len(text)), 0.0, 0.0][:width]}
            for i, text in enumerate(body["input"])
        ]
        data.reverse()
        out = json.dumps({"data": data}).encode()
        self.send_response(200)
        self.send_header("content-length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)


def check_openai_compatible():
    server = HTTPServer(("127.0.0.1", 0), MockEmbeddings)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{server.server_port}/v1"
    os.environ["MOCK_EMBED_KEY"] = "test-token"
    spec = {
        "id": "mock-embed",
        "dimension": 3,
        "provider": "openai-compatible",
        "base_url": "http://127.0.0.1:9/unused",
        "base_url_env": "MOCK_EMBED_BASE_URL",
        "api_key_env": "MOCK_EMBED_KEY",
        "remote_model": "mock-3",
    }
    os.environ["MOCK_EMBED_BASE_URL"] = base
    model = embedding_models.load_model(spec)
    assert isinstance(model, embedding_models.EmbeddingModel)
    assert model.dimension == 3
    vectors = model.embed(["abc", "a"])
    assert vectors == [[1.0, 0.0, 0.0], [1.0, 0.0, 0.0]], vectors
    path, auth, body = MockEmbeddings.seen[-1]
    assert path == "/v1/embeddings", path
    assert auth == "Bearer test-token", auth
    assert body == {"model": "mock-3", "input": ["abc", "a"]}, body

    wrong = embedding_models.load_model({**spec, "remote_model": "mock-2"})
    try:
        wrong.embed(["abc"])
    except RuntimeError as exc:
        assert "expected 3" in str(exc), exc
    else:
        raise AssertionError("a wrong-dimension reply must fail")

    del os.environ["MOCK_EMBED_KEY"]
    try:
        embedding_models.load_model(spec)
    except RuntimeError as exc:
        assert "MOCK_EMBED_KEY" in str(exc), exc
    else:
        raise AssertionError("a missing API key must fail")
    server.shutdown()
    print("ok openai-compatible")


def check_nearest():
    try:
        import sqlite_vec  # noqa: F401
    except ImportError:
        print("skip nearest")
        return
    db = os.path.join(tempfile.mkdtemp(), "app.db")
    os.environ["SILC_DB_PATH"] = db
    import embed_worker

    dim = embed_worker.MODEL_SPEC["dimension"]

    def unit(i):
        v = [0.0] * dim
        v[i] = 1.0
        return v

    class FakeModel:
        dimension = dim

        def embed(self, texts):
            return [unit(0) for _ in texts]

    embed_worker._model = FakeModel()
    embed_worker.DB_PATH = db
    conn = sqlite3.connect(db)
    conn.execute(
        "CREATE TABLE files (id TEXT PRIMARY KEY, data TEXT NOT NULL,"
        " created_at TEXT NOT NULL DEFAULT (datetime('now')),"
        " updated_at TEXT NOT NULL DEFAULT (datetime('now')))"
    )
    near = [0.0] * dim
    near[0], near[1] = 0.8, 0.6
    rows = [
        ("far", {"name": "far", "vector_embedding": unit(2)}),
        ("best", {"name": "best", "vector_embedding": unit(0)}),
        ("near", {"name": "near", "vector_embedding": near}),
        ("none", {"name": "no vector"}),
        ("short", {"name": "short", "vector_embedding": [1.0, 0.0]}),
    ]
    for row_id, data in rows:
        conn.execute("INSERT INTO files (id, data) VALUES (?, ?)", (row_id, json.dumps(data)))
    conn.commit()
    conn.close()

    hits = embed_worker.nearest({"table": "files", "needle": "anything", "limit": 50})
    assert [h["id"] for h in hits] == ["best", "near", "far"], hits
    assert all("vector_embedding" not in h for h in hits), hits
    assert hits[0]["_score"] > hits[1]["_score"] > hits[2]["_score"], hits
    assert [h["id"] for h in embed_worker.nearest({"table": "files", "needle": "x", "limit": 1})] == ["best"]
    assert embed_worker.nearest({"table": "files", "needle": "   ", "limit": 5}) == []
    capped = embed_worker.nearest({"table": "files", "vector": unit(0), "limit": 10 ** 9})
    assert len(capped) == 3, capped
    for bad in (
        {"table": "secrets", "needle": "x", "limit": 1},
        {"table": "files", "needle": "x", "limit": 0},
        {"table": "files", "needle": "x", "limit": True},
        {"table": "files", "vector": [1.0], "limit": 1},
    ):
        try:
            embed_worker.nearest(bad)
        except ValueError:
            pass
        else:
            raise AssertionError(f"expected ValueError for {bad}")
    print("ok nearest")


check_openai_compatible()
check_nearest()
