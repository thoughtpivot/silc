#!/usr/bin/env python3
"""Persistent silclm completion worker for `silc assist` (ADR-008).

Loads the GGUF once, then serves JSON-line requests on stdin until EOF.

Environment:
  SILC_LLM_MODEL_PATH — absolute path to silclm GGUF (required)
  SILC_LLM_N_CTX — context window (default 16384 for assist)
  SILC_LLM_MAX_TOKENS — default max new tokens (default 4096)
  SILC_LLM_N_GPU_LAYERS — llama.cpp GPU layers; -1 = all (default, Metal on macOS)

Protocol:
  After model load, writes one line: {"ready":true}
  Each request is one JSON line:
    {"prompt":"...","max_tokens":N?}                         — raw completion
    {"messages":[{"role":"...","content":"..."}],"max_tokens":N?}  — chat template
    optional chat fields: {"stop":[...],"temperature":0.4}
  Each response is one JSON line:
    {"text":"...","truncated":bool} or {"error":"..."}
"""

from __future__ import annotations

import json
import os
import sys

from llama_cpp import Llama

MODEL_PATH = os.environ["SILC_LLM_MODEL_PATH"]
N_CTX = int(os.environ.get("SILC_LLM_N_CTX", "16384"))
DEFAULT_MAX_TOKENS = int(os.environ.get("SILC_LLM_MAX_TOKENS", "4096"))
N_GPU_LAYERS = int(os.environ.get("SILC_LLM_N_GPU_LAYERS", "-1"))

STOP = ["</s>", "\n# Tool result", "\n# Next\n#", "\n# Task"]
AUTHOR_STOP = ["\n# END", "\n#!/usr/bin/env silc\n"]


def emit(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def parse_max_tokens(req: dict) -> int:
    max_tokens = req.get("max_tokens", DEFAULT_MAX_TOKENS)
    try:
        return int(max_tokens)
    except (TypeError, ValueError):
        return DEFAULT_MAX_TOKENS


def main() -> int:
    try:
        llm = Llama(
            model_path=MODEL_PATH,
            n_ctx=N_CTX,
            n_gpu_layers=N_GPU_LAYERS,
            verbose=False,
        )
    except Exception as exc:  # noqa: BLE001 — surface load errors to the CLI
        emit({"error": f"model load failed: {exc}"})
        return 1

    emit({"ready": True})

    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
            max_tokens = parse_max_tokens(req)
            messages = req.get("messages")
            if isinstance(messages, list) and messages:
                stop = req.get("stop")
                if not isinstance(stop, list) or not stop:
                    stop = AUTHOR_STOP
                temperature = req.get("temperature")
                try:
                    temperature = float(temperature) if temperature is not None else 0.2
                except (TypeError, ValueError):
                    temperature = 0.2
                kwargs = {
                    "messages": messages,
                    "max_tokens": max_tokens,
                    "temperature": temperature,
                    "stop": stop,
                }
                out = llm.create_chat_completion(**kwargs)
                choice = out["choices"][0]
                text = (choice.get("message") or {}).get("content") or ""
                truncated = choice.get("finish_reason") == "length"
                emit({"text": text.strip(), "truncated": truncated})
                continue

            prompt = req.get("prompt")
            if not isinstance(prompt, str) or not prompt:
                emit({"error": "request requires `prompt` or non-empty `messages`"})
                continue
            out = llm(
                prompt,
                max_tokens=max_tokens,
                temperature=0.2,
                stop=STOP,
            )
            choice = out["choices"][0]
            text = choice.get("text") or ""
            truncated = choice.get("finish_reason") == "length"
            emit({"text": text.strip(), "truncated": truncated})
        except Exception as exc:  # noqa: BLE001 — keep the worker alive
            emit({"error": str(exc)})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
