#!/usr/bin/env python3
"""Verify pinned external Basis inputs, then run actual GPU preservation tests.

No independent pixel oracle: frames are compared with admitted initial renders.
External assets stay outside the repository; --fetch downloads pinned URLs.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--target-dir", type=Path)
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    report = json.loads((project / "docs/research/basis-upstream-corpus-2026-10-06.json").read_text())
    args.root.mkdir(parents=True, exist_ok=True)
    for row in report["files"]:
        path = args.root / row["file"]
        data = path.read_bytes() if path.exists() else None
        if data is None and args.fetch:
            data = urllib.request.urlopen(row["url"], timeout=60).read()
            if hashlib.sha256(data).hexdigest() != row["sha256"]:
                raise SystemExit(f"download hash mismatch: {row['file']}")
            path.write_bytes(data)
        if data is None or hashlib.sha256(data).hexdigest() != row["sha256"]:
            raise SystemExit(f"missing or modified pinned input: {path}")
    command = ["cargo", "test", "-p", "rrrah", "--test", "basis_readback"]
    if args.target_dir:
        command += ["--target-dir", str(args.target_dir.resolve())]
    command += ["--", "--ignored", "--nocapture"]
    environment = dict(os.environ, RRRAH_BASIS_CORPUS=str(args.root.resolve()))
    subprocess.run(command, cwd=project, env=environment, check=True)


if __name__ == "__main__":
    main()
