#!/usr/bin/env python3
"""Qualify actual ordinary-viewer GLB swap publication; requires an unlocked Mac.

Uses original BoxTextured/Box fixtures, a one-entry model cache and a 1 MiB
managed CPU root. No cold-disk, RSS, throughput or physical-HDR claim is made.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--rounds", type=int, default=1000)
    args = parser.parse_args()
    if not 1 <= args.rounds <= 10000:
        parser.error("rounds must be in 1..10000")
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    repo = Path(__file__).resolve().parents[1]
    inputs = [repo / "tests/fixtures/glb" / name for name in ["BoxTextured.glb", "Box.glb"]]
    env = {k: v for k, v in os.environ.items() if not k.startswith("RRRAH_QUALIFY_")}
    env.update(RRRAH_QUALIFY_GPU_RECOVERY="1", RRRAH_QUALIFY_MODEL_SWITCH="1",
               RRRAH_QUALIFY_MODEL_REQUIRE_SWAP="1", RUST_LOG="info")
    common = ["--gpu-backend", "metal", "--managed-memory-mb", "1",
              "--model-gpu-mb", "32", "--model-cache-mb", "1", "--model-cache-count", "1",
              "--model-swap-mb", "2", "--model-swap-count", "2", "--model-swap-queue-mb", "1",
              "--model-swap-restore-mb", "1", "--prefetch-ahead", "0", "--prefetch-behind", "0"]
    result = {"binary_sha256": sha(binary), "inputs": {str(p.relative_to(repo)): sha(p) for p in inputs},
              "rounds": args.rounds, "managed_cpu_limit": 1048576, "model_cache_count": 1,
              "physical_hdr_qualified": False, "cold_disk_qualified": False, "rss_qualified": False,
              "command_args": common, "cases": {}}
    with tempfile.TemporaryDirectory(prefix="rrrah-glb-swap-qualification-") as folder:
        folder = Path(folder)
        for i, source in enumerate(inputs, 1):
            shutil.copyfile(source, folder / f"{i:02}.glb")
        env["RRRAH_QUALIFY_RASTER_SWITCH"] = str(folder / "02.glb")
        for name, rounds, extra in [("positive", args.rounds, []), ("no-cache-negative", 1, ["--no-cache"])]:
            env["RRRAH_QUALIFY_NAV_ROUNDS"] = str(rounds)
            command = [str(binary), str(folder / "01.glb"), *common,
                       "--cache-dir", str(folder / (name + "-cache")), *extra]
            log = output / (name + ".log")
            with log.open("w") as stream:
                run = subprocess.run(command, env=env, stdout=stream, stderr=subprocess.STDOUT,
                                     timeout=60 + rounds * 2, check=False)
            data = log.read_bytes()
            text = data.decode("utf-8", errors="replace")
            with (output / (name + ".log.gz")).open("wb") as stream:
                # Fixed metadata makes the compressed proof deterministic for these bytes.
                with gzip.GzipFile(filename="", mode="wb", fileobj=stream, mtime=0) as compressed:
                    compressed.write(data)
            passes = [int(n) for n in re.findall(r"LIVE_MODEL_SWAP_PASS round=(\d+)", text)]
            hits = [(int(a), int(b)) for a, b in re.findall(
                r"LIVE_MODEL_CACHE_HIT .* tier=swap swap_reads_before=(\d+) swap_reads_after=(\d+)", text)]
            credits = [(int(n), int(a), int(b)) for n, a, b in re.findall(
                r"LIVE_MODEL_GPU_STABLE round=(\d+) used=(\d+) baseline=(\d+)", text)]
            case = {"exit_code": run.returncode, "log_sha256": sha(log),
                    "compressed_log_sha256": sha(output / (name + ".log.gz")),
                    "guarded_swap_returns": len(hits), "presented_rounds": passes,
                    "gpu_credit_samples": len(credits),
                    "gpu_credit_values": sorted({a for _, a, _ in credits}),
                    "decoder_refused": "LIVE_MODEL_DECODE_REFUSED" in text}
            result["cases"][name] = case
            # Preserve compact readable evidence as well as the complete compressed log.
            lines = text.splitlines()
            (output / (name + "-excerpt.log")).write_text("\n".join(lines[:12] + ["..."] + lines[-16:]) + "\n")
            (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
            if name == "positive":
                assert run.returncode == 0 and "LIVE_RECOVERY_PASS" in text, case
                assert passes == list(range(1, rounds + 1)), case
                assert len(hits) == rounds and all(b == a + 1 for a, b in hits), case
                assert [n for n, _, _ in credits] == passes and all(a == b for _, a, b in credits), case
                assert len(case["gpu_credit_values"]) == 1 and not case["decoder_refused"], case
                assert "LIVE_RECOVERY_FAIL" not in text, case
            else:
                assert run.returncode != 0 and case["decoder_refused"] and not passes, case
                assert "guarded native model decoder fallback refused" in text, case
            log.unlink()  # Complete proof retained losslessly in .log.gz.
    result["verified"] = True
    (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"verified": True, "rounds": args.rounds, "result": str(output / "result.json")}))


if __name__ == "__main__":
    main()
