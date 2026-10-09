#!/usr/bin/env python3
"""Publish stable Silc crates to crates.io in dependency order.

``vX.Y.Z`` tags publish. ``vX.Y.Z-dev.N`` tags do not. Versions already on
crates.io are left as they are, so the baseline ``v0.7.0`` tag is a no-op.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
STABLE_TAG = re.compile(r"^v(\d+\.\d+\.\d+)$")
USER_AGENT = "silc-release (https://github.com/thoughtpivot/silc)"
ATTEMPTS = 8
PAUSE_SECONDS = 20


def cargo_metadata() -> dict:
    proc = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(proc.stdout)


def publish_order(meta: dict) -> list[str]:
    members = set(meta["workspace_members"])
    by_id = {pkg["id"]: pkg for pkg in meta["packages"]}
    graph: dict[str, set[str]] = {}
    versions: dict[str, str] = {}
    for member in members:
        pkg = by_id[member]
        if pkg.get("publish") == []:
            continue
        versions[pkg["name"]] = pkg["version"]
        deps = set()
        for dep in pkg["dependencies"]:
            if dep.get("kind") == "dev":
                continue
            deps.add(dep["name"])
        graph[pkg["name"]] = deps
    names = set(graph)
    incoming = {name: {dep for dep in deps if dep in names} for name, deps in graph.items()}
    ready = sorted(name for name, deps in incoming.items() if not deps)
    ordered: list[str] = []
    while ready:
        name = ready.pop(0)
        ordered.append(name)
        for other, deps in incoming.items():
            if name in deps:
                deps.remove(name)
                if not deps and other not in ordered and other not in ready:
                    ready.append(other)
                    ready.sort()
    if len(ordered) != len(graph):
        leftover = sorted(set(graph) - set(ordered))
        raise SystemExit(f"crate dependency cycle: {leftover}")
    return ordered, versions


def tag_version() -> str | None:
    ref = os.environ.get("GITHUB_REF", "")
    if not ref.startswith("refs/tags/"):
        return None
    match = STABLE_TAG.fullmatch(ref.removeprefix("refs/tags/"))
    return match.group(1) if match else None


def on_crates_io(name: str, version: str) -> bool:
    url = f"https://crates.io/api/v1/crates/{name}/{version}"
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.status == 200
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return False
        raise


def missing_crates(order: list[str], version: str) -> list[str]:
    missing = []
    for name in order:
        if on_crates_io(name, version):
            print(f"{name} {version} is already on crates.io", file=sys.stderr)
        else:
            print(f"{name} {version} is not on crates.io", file=sys.stderr)
            missing.append(name)
    return missing


def publish_one(name: str) -> None:
    command = ["cargo", "publish", "-p", name, "--locked", "--no-verify"]
    for attempt in range(1, ATTEMPTS + 1):
        proc = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        output = (proc.stdout or "") + (proc.stderr or "")
        if proc.returncode == 0 or "already uploaded" in output or "already exists" in output:
            sys.stderr.write(output)
            print(f"published {name}", file=sys.stderr)
            return
        sys.stderr.write(output)
        retryable = any(
            phrase in output
            for phrase in (
                "failed to select a version",
                "no matching package",
                "temporarily unavailable",
                "updating crates.io index",
            )
        )
        if not retryable or attempt == ATTEMPTS:
            raise SystemExit(f"cargo publish -p {name} failed")
        print(
            f"waiting {PAUSE_SECONDS}s for the crates.io index before retrying {name}",
            file=sys.stderr,
        )
        time.sleep(PAUSE_SECONDS)


def main(argv: list[str]) -> None:
    plan_only = argv == ["--plan"]
    order_only = argv == ["--order"]
    if argv and not plan_only and not order_only:
        raise SystemExit(f"usage: {sys.argv[0]} [--plan|--order]")
    meta = cargo_metadata()
    order, versions = publish_order(meta)
    if order_only:
        print("\n".join(order))
        return
    version = tag_version()
    if version is None:
        print("not a stable vX.Y.Z tag; crates.io publish skipped", file=sys.stderr)
        if plan_only:
            print("publish=false")
        return
    mismatched = sorted(name for name, crate_version in versions.items() if crate_version != version)
    if mismatched:
        raise SystemExit(
            f"tag v{version} does not match crate versions for {mismatched}"
        )
    missing = missing_crates(order, version)
    if plan_only:
        print("publish=true" if missing else "publish=false")
        return
    if not missing:
        return
    if not os.environ.get("CARGO_REGISTRY_TOKEN"):
        raise SystemExit(
            "CARGO_REGISTRY_TOKEN is empty. Trusted publishing did not provide a token. "
            "See docs/RELEASING.md."
        )
    subprocess.run(
        ["cargo", "check", "--workspace", "--locked"],
        cwd=ROOT,
        check=True,
    )
    for name in missing:
        publish_one(name)


if __name__ == "__main__":
    main(sys.argv[1:])
