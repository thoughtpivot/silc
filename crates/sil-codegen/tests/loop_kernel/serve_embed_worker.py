"""Serve a generated embed worker for the loop kernel's find-nearest tests.

argv: <python dir holding embed_worker.py> <socket path> <app.db path>

The kernel sends vectors, so the model is a stub that refuses to embed; the
ranking itself is the worker's real sqlite-vec query.
"""
import os
import sys

python_dir, socket_path, db_path = sys.argv[1:4]
os.environ["SILC_EMBED_SOCKET"] = socket_path
os.environ["SILC_DB_PATH"] = db_path
sys.path.insert(0, python_dir)

import embed_worker  # noqa: E402


class NoEmbeds:
    dimension = embed_worker.MODEL_SPEC["dimension"]

    def embed(self, texts):
        raise RuntimeError("find nearest sends a vector; nothing should be embedded")


embed_worker._model = NoEmbeds()
embed_worker.main()
