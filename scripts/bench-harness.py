#!/usr/bin/env python3
"""Reproducible process-boundary benchmark runner for the RAW path.

The runner deliberately does not claim a true cold OS-page-cache run: flushing
the page cache is privileged and platform-specific.  ``--inspect --no-cache``
means *no persistent decoded-mosaic cache*; the manifest records the OS cache
state as unknown.  Every sample is JSONL and contains the host/toolchain
manifest so results can be compared without relying on implicit machine state.

Example:
    scripts/bench-harness.py /tmp/rrrah-sample-1.cr2 --repetitions 7 \
        --output target/bench/runs.jsonl
    scripts/bench-report.py target/bench/runs.jsonl
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path
from typing import Any


_DURATION_RE = re.compile(r"(?P<value>[0-9]+(?:\.[0-9]+)?)\s*(?P<unit>ns|us|µs|ms|s)$")
_RAW_RE = re.compile(
    r"raw:\s+(?P<w>\d+)x(?P<h>\d+)\s+(?P<bits>\d+)-bit\s+cpp=(?P<cpp>\d+)\s+"
    r"pixels=(?P<pixels>\d+)\s+bytes=(?P<bytes>\d+)"
)
_TIMING_RE = re.compile(
    r"cache_hit:\s+(?P<hit>true|false),\s+decode_or_cache:\s+(?P<decode>[^,]+),\s+"
    r"total:\s+(?P<total>[^\s]+)"
)


def now_utc() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="milliseconds")


def run_quiet(command: list[str], timeout: float = 15.0) -> str | None:
    try:
        completed = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if completed.returncode != 0:
        return None
    return completed.stdout.strip()


def host_manifest(binary: Path) -> dict[str, Any]:
    rustc = run_quiet(["rustc", "-Vv"])
    commit = run_quiet(["git", "rev-parse", "HEAD"])
    binary_sha = None
    try:
        digest = hashlib.sha256()
        with binary.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
        binary_sha = digest.hexdigest()
    except OSError:
        pass
    try:
        affinity = sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None
    except OSError:
        affinity = None
    return {
        "schema": "rrrah-bench-v1",
        "run_id": str(uuid.uuid4()),
        "started_at": now_utc(),
        "host": {
            "os": platform.platform(),
            "kernel": platform.release(),
            "arch": platform.machine(),
            "cpu_model": platform.processor() or None,
            "logical_cpus": os.cpu_count(),
            "python": platform.python_version(),
            "rustc_vv": rustc,
            "power_mode": os.environ.get("RRAH_POWER_MODE"),
            "cpu_affinity": affinity,
        },
        "build": {
            "git_commit": commit,
            "binary": str(binary),
            "binary_sha256": binary_sha,
            "profile": "release",
            "rustflags": os.environ.get("RUSTFLAGS"),
        },
    }


def file_manifest(path: Path) -> dict[str, Any]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            size += len(block)
            digest.update(block)
    return {
        "path": str(path.resolve()),
        "name": path.name,
        "size_bytes": size,
        "sha256": digest.hexdigest(),
    }


def parse_duration(text: str) -> float | None:
    match = _DURATION_RE.search(text.strip())
    if not match:
        return None
    value = float(match.group("value"))
    unit = match.group("unit")
    return value * {"ns": 1e-6, "us": 1e-3, "µs": 1e-3, "ms": 1.0, "s": 1000.0}[unit]


def parse_inspect(stdout: str) -> dict[str, Any]:
    parsed: dict[str, Any] = {"embedded_jpeg_used": False}
    for line in stdout.splitlines():
        raw = _RAW_RE.search(line)
        if raw:
            parsed["raw"] = {
                "width": int(raw.group("w")),
                "height": int(raw.group("h")),
                "bits_per_sample": int(raw.group("bits")),
                "components_per_pixel": int(raw.group("cpp")),
                "pixels": int(raw.group("pixels")),
                "bytes": int(raw.group("bytes")),
            }
        timing = _TIMING_RE.search(line)
        if timing:
            parsed["cache_hit"] = timing.group("hit") == "true"
            parsed["decode_or_cache_ms"] = parse_duration(timing.group("decode"))
            parsed["reported_total_ms"] = parse_duration(timing.group("total"))
        if "embedded JPEG is not used" in line:
            parsed["embedded_jpeg_used"] = False
    return parsed


def timed_run(command: list[str], timeout: float) -> dict[str, Any]:
    started = time.monotonic_ns()
    try:
        completed = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        error = None
    except subprocess.TimeoutExpired as exc:
        completed = None
        error = f"timeout after {timeout:.1f}s"
        stdout = exc.stdout or ""
        stderr = exc.stderr or ""
    except OSError as exc:
        completed = None
        error = str(exc)
        stdout = ""
        stderr = ""
    elapsed_ms = (time.monotonic_ns() - started) / 1_000_000.0
    if isinstance(stdout, bytes):
        stdout = stdout.decode(errors="replace")
    if isinstance(stderr, bytes):
        stderr = stderr.decode(errors="replace")
    if completed is not None:
        stdout = completed.stdout
        stderr = completed.stderr
        status = completed.returncode
    else:
        status = 124
    record: dict[str, Any] = {
        "wall_ms": elapsed_ms,
        "status": status,
        "stdout": stdout,
        "stderr_tail": stderr[-4096:],
        "error": error,
    }
    record.update(parse_inspect(stdout))
    return record


def run(args: argparse.Namespace) -> int:
    binary = Path(args.binary).resolve()
    if not binary.is_file():
        print(f"binary is not a file: {binary}", file=sys.stderr)
        return 2
    fixtures = [Path(item).resolve() for item in args.fixtures]
    for fixture in fixtures:
        if not fixture.is_file():
            print(f"fixture is not a file: {fixture}", file=sys.stderr)
            return 2
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    manifest = host_manifest(binary)
    manifest["runner"] = {
        "command": " ".join([str(binary), "--inspect"]),
        "repetitions": args.repetitions,
        "timeout_s": args.timeout,
        "workers": args.workers,
        "backend": args.backend,
        "persistent_cache_state": "disabled|warm",
        "os_page_cache_state": "unknown (not flushed)",
    }
    with output.open("w", encoding="utf-8") as stream:
        for fixture in fixtures:
            fixture_info = file_manifest(fixture)
            # Keep the cache isolated per fixture and per run. A warm series is
            # seeded once, then all repetitions exercise the same cache bytes.
            cache_dir = Path(tempfile.mkdtemp(prefix="rrrah-bench-cache-"))
            try:
                warm_seed = [str(binary), "--inspect", "--cache-dir", str(cache_dir), str(fixture)]
                seed = timed_run(warm_seed, args.timeout)
                if seed["status"] != 0:
                    print(f"cache seed failed for {fixture}: {seed['stderr_tail']}", file=sys.stderr)
                for mode in ("no-persistent-cache", "warm-persistent-cache"):
                    for iteration in range(1, args.repetitions + 1):
                        if mode == "no-persistent-cache":
                            command = [str(binary), "--inspect", "--no-cache", str(fixture)]
                        else:
                            command = [str(binary), "--inspect", "--cache-dir", str(cache_dir), str(fixture)]
                        sample = timed_run(command, args.timeout)
                        row = dict(manifest)
                        row["sample"] = {
                            "fixture": fixture_info,
                            "mode": mode,
                            "iteration": iteration,
                            "command": command,
                            "workers": args.workers,
                            "backend": args.backend,
                            "status": sample.pop("status"),
                            "error": sample.pop("error"),
                            "wall_ms": sample.pop("wall_ms"),
                            "metrics": sample,
                        }
                        stream.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")
                        stream.flush()
            finally:
                shutil.rmtree(cache_dir, ignore_errors=True)
    print(f"results: {output}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixtures", nargs="+", help="CR2/DNG files; embedded previews are not used")
    parser.add_argument("--binary", default="target/release/rrrah", help="release rrrah executable")
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=300.0)
    parser.add_argument(
        "--workers",
        type=int,
        default=int(os.environ.get("RRAH_WORKERS", "0")),
        help="worker count label for scaling matrices (0 means application default)",
    )
    parser.add_argument(
        "--backend",
        default=os.environ.get("RRAH_BACKEND", "unknown"),
        help="backend label, e.g. cpu, wgpu-metal, wgpu-vulkan",
    )
    parser.add_argument("--output", default="target/bench/runs.jsonl")
    args = parser.parse_args()
    if args.repetitions < 1:
        parser.error("--repetitions must be positive")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
