#!/usr/bin/env python3
"""Bump the Silc workspace version in Cargo.toml and Cargo.lock.

Published crates inherit ``[workspace.package].version`` and declare their
intra-workspace requirements in ``[workspace.dependencies]``. Both must move
together. Language ``@version("...")`` annotations are not crate versions and
are left alone.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

VERSION_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))


def read_workspace_version(text: str) -> str:
    section = None
    for line in text.splitlines():
        header = re.match(r"^\[(.+)\]\s*$", line.strip())
        if header:
            section = header.group(1)
            continue
        if section == "workspace.package":
            match = re.match(r'^version\s*=\s*"([^"]+)"\s*$', line.strip())
            if match:
                return match.group(1)
    raise SystemExit("Cargo.toml is missing [workspace.package] version")


def bump_manifest(text: str, version: str) -> str:
    section = None
    replaced_package = False
    replaced_deps = 0
    out: list[str] = []
    for line in text.splitlines(keepends=True):
        header = re.match(r"^\[(.+)\]\s*$", line.strip())
        if header:
            section = header.group(1)
        if section == "workspace.package" and re.match(r"^version\s*=", line):
            line = re.sub(
                r'version\s*=\s*"[^"]*"',
                f'version = "{version}"',
                line,
                count=1,
            )
            replaced_package = True
        elif (
            section == "workspace.dependencies"
            and "path =" in line
            and 'version = "' in line
        ):
            line = re.sub(
                r'version\s*=\s*"[^"]*"',
                f'version = "{version}"',
                line,
                count=1,
            )
            replaced_deps += 1
        out.append(line)
    if not replaced_package:
        raise SystemExit("did not update [workspace.package] version")
    if replaced_deps < 1:
        raise SystemExit("did not update [workspace.dependencies] versions")
    return "".join(out)


def member_names(text: str) -> list[str]:
    """Package names for workspace members, in manifest order."""
    in_members = False
    paths: list[str] = []
    for line in text.splitlines():
        if re.match(r"^members\s*=\s*\[", line.strip()):
            in_members = True
            continue
        if in_members:
            if line.strip().startswith("]"):
                break
            match = re.search(r'"([^"]+)"', line)
            if match:
                paths.append(match.group(1))
    names: list[str] = []
    for rel in paths:
        manifest = os.path.join(ROOT, rel, "Cargo.toml")
        with open(manifest, encoding="utf-8") as handle:
            for line in handle:
                match = re.match(r'^name\s*=\s*"([^"]+)"\s*$', line.strip())
                if match:
                    names.append(match.group(1))
                    break
            else:
                raise SystemExit(f"{manifest} is missing package name")
    if not names:
        raise SystemExit("workspace members list is empty")
    return names


def bump_lock(text: str, version: str, names: set[str]) -> str:
    """Set path-package versions. Registry packages keep their own versions."""
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    index = 0
    changed = 0
    while index < len(lines):
        if lines[index].strip() != "[[package]]":
            out.append(lines[index])
            index += 1
            continue
        block = [lines[index]]
        index += 1
        while index < len(lines) and lines[index].strip() != "[[package]]":
            block.append(lines[index])
            index += 1
        name = None
        has_source = False
        for line in block:
            match = re.match(r'^name = "([^"]+)"\s*$', line.strip())
            if match:
                name = match.group(1)
            if line.startswith("source ="):
                has_source = True
        if name in names and not has_source:
            rewritten: list[str] = []
            saw_version = False
            for line in block:
                if re.match(r"^version = ", line):
                    line = re.sub(
                        r'version = "[^"]*"',
                        f'version = "{version}"',
                        line,
                        count=1,
                    )
                    saw_version = True
                    changed += 1
                rewritten.append(line)
            if not saw_version:
                raise SystemExit(f"Cargo.lock package {name} has no version")
            block = rewritten
        out.extend(block)
    if changed != len(names):
        raise SystemExit(
            f"updated {changed} Cargo.lock packages, expected {len(names)}"
        )
    return "".join(out)


def verify_lock(root: str) -> None:
    """Confirm Cargo.lock still resolves. CI has network; offline is not required."""
    subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--quiet"],
        cwd=root,
        check=True,
        stdout=subprocess.DEVNULL,
    )


def self_test() -> None:
    sample = """[workspace]
members = [
    "crates/silc",
]

[workspace.package]
version = "0.7.0"
rust-version = "1.82"

[workspace.dependencies]
sil-core = { version = "0.7.0", path = "crates/sil-core" }
"""
    bumped = bump_manifest(sample, "0.8.0-dev.1")
    if 'version = "0.8.0-dev.1"' not in bumped or bumped.count("0.7.0") != 0:
        raise SystemExit("manifest bump failed")
    if 'rust-version = "1.82"' not in bumped:
        raise SystemExit("rust-version was rewritten")
    lock = """[[package]]
name = "sil-core"
version = "0.7.0"

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"""
    rewritten = bump_lock(lock, "0.8.0-dev.1", {"sil-core"})
    if 'name = "sil-core"' not in rewritten or rewritten.count('version = "0.8.0-dev.1"') != 1:
        raise SystemExit(f"lock bump failed:\n{rewritten}")
    if 'version = "1.0.0"' not in rewritten:
        raise SystemExit("registry package version was rewritten")
    print("bump-workspace-version self-test ok")


def main(argv: list[str]) -> None:
    if argv == ["--self-test"]:
        self_test()
        return
    manifest_path = os.path.join(ROOT, "Cargo.toml")
    with open(manifest_path, encoding="utf-8") as handle:
        manifest = handle.read()
    if argv == ["--print"]:
        print(read_workspace_version(manifest))
        return
    if len(argv) != 1 or not VERSION_RE.fullmatch(argv[0]):
        raise SystemExit(f"usage: {sys.argv[0]} <x.y.z[-prerelease]> | --print | --self-test")
    version = argv[0]
    names = member_names(manifest)
    updated = bump_manifest(manifest, version)
    with open(manifest_path, "w", encoding="utf-8") as handle:
        handle.write(updated)
    lock_path = os.path.join(ROOT, "Cargo.lock")
    with open(lock_path, encoding="utf-8") as handle:
        lock = handle.read()
    with open(lock_path, "w", encoding="utf-8") as handle:
        handle.write(bump_lock(lock, version, set(names)))
    verify_lock(ROOT)


if __name__ == "__main__":
    main(sys.argv[1:])
