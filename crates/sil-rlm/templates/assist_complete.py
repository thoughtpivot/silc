#!/usr/bin/env python3
"""One-shot silclm completion for `silc assist` (ADR-008).

Reads the prompt from stdin (UTF-8). Writes the completion text to stdout.
Environment:
  SILC_LLM_MODEL_PATH — absolute path to silclm GGUF
  SILC_LLM_N_CTX — context window (default 8192)
  SILC_LLM_MAX_TOKENS — max new tokens (default 512)
"""

from __future__ import annotations

import os
import sys

from llama_cpp import Llama

MODEL_PATH = os.environ["SILC_LLM_MODEL_PATH"]
N_CTX = int(os.environ.get("SILC_LLM_N_CTX", "8192"))
MAX_TOKENS = int(os.environ.get("SILC_LLM_MAX_TOKENS", "512"))

prompt = sys.stdin.read()
llm = Llama(model_path=MODEL_PATH, n_ctx=N_CTX, verbose=False)
out = llm(
    prompt,
    max_tokens=MAX_TOKENS,
    temperature=0.2,
    stop=["</s>", "\n# Tool result", "\n# Next\n#", "\n# Task"],
)
sys.stdout.write(out["choices"][0]["text"].strip())
