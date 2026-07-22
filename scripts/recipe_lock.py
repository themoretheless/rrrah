#!/usr/bin/env python3
"""Compute the semantic dependency-closure digest for the Rawler adapter."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LOCK_PATH = ROOT / "Cargo.lock"
EXPECTED_PATH = ROOT / "scripts" / "rawler-semantic-lock.sha256"
ROOT_PACKAGE = ("rawler", "0.7.2")


def cargo_metadata() -> dict:
    process = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(process.stdout)


def lock_checksums() -> dict[tuple[str, str, str], str]:
    document = tomllib.loads(LOCK_PATH.read_text(encoding="utf-8"))
    result: dict[tuple[str, str, str], str] = {}
    for package in document["package"]:
        source = package.get("source", "")
        checksum = package.get("checksum", "-")
        result[(package["name"], package["version"], source)] = checksum
    return result


def closure_lines(metadata: dict) -> list[str]:
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    roots = [
        package["id"]
        for package in metadata["packages"]
        if (package["name"], package["version"]) == ROOT_PACKAGE
    ]
    if len(roots) != 1:
        raise RuntimeError(f"expected exactly one {ROOT_PACKAGE!r}, found {len(roots)}")

    pending = roots
    visited: set[str] = set()
    while pending:
        package_id = pending.pop()
        if package_id in visited:
            continue
        visited.add(package_id)
        pending.extend(nodes[package_id]["dependencies"])

    checksums = lock_checksums()
    lines = []
    for package_id in visited:
        package = packages[package_id]
        lines.append(package_record(package, nodes[package_id], checksums))
    return sorted(lines)


def package_record(package: dict, node: dict, checksums: dict[tuple[str, str, str], str]) -> str:
    source = package.get("source")
    if source is None:
        raise RuntimeError(
            f"path dependency {package['name']} {package['version']} is not immutable; "
            "recipe identity fails closed"
        )
    checksum = checksums.get((package["name"], package["version"], source), "-")
    if source.startswith("registry+") and checksum == "-":
        raise RuntimeError(f"registry package {package['name']} has no Cargo.lock checksum")
    if source.startswith("git+") and "#" not in source:
        raise RuntimeError(f"git package {package['name']} is not pinned to a commit")
    features = ",".join(sorted(node.get("features", [])))
    return "\t".join((package["name"], package["version"], source, checksum, features))


def semantic_digest(lines: list[str]) -> str:
    canonical = ("\n".join(lines) + "\n").encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="compare with the checked-in digest")
    parser.add_argument("--dump", action="store_true", help="print canonical dependency records")
    args = parser.parse_args()

    lines = closure_lines(cargo_metadata())
    digest = semantic_digest(lines)
    if args.dump:
        print("\n".join(lines))
    print(digest)

    if args.check:
        expected = EXPECTED_PATH.read_text(encoding="ascii").strip()
        if digest != expected:
            print(
                "Rawler semantic dependency closure changed; run the RAW corpus, bump the "
                "backend contract, and update both the manifest digest and lock file.",
                file=sys.stderr,
            )
            print(f"expected {expected}\nactual   {digest}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
