#!/usr/bin/env python3
"""Compare frozen fixed-input native test kernels with interspersed AA controls."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import runpy
import subprocess
import tempfile


TEST = "encode::ordered_pixel::shared_fragment_store::tests::captured_dense_cell_fixed_work_cpu"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--log", required=True, type=Path)
    parser.add_argument("--repetitions", type=int, default=8)
    parser.add_argument("--workload", choices=["dense1236", "occluded1024", "guarded1024"], default="dense1236")
    args = parser.parse_args()
    test_name = {"dense1236": TEST,
                 "occluded1024": "encode::ordered_pixel::shared_fragment_store::tests::occluded_suffix_fixed_work_cpu",
                 "guarded1024": "encode::ordered_pixel::shared_fragment_store::tests::guarded_suffix_fixed_work_cpu"}[args.workload]
    if not 1 <= args.repetitions <= 256:
        parser.error("repetitions must be between 1 and 256")
    result = {
        "scope": f"Fixed {args.workload} CPU kernel in supplied release test binaries; no page/display/GPU inference",
        "workload": args.workload,
        "timing": "Native Instant interval, excluding process startup; six baseline AA controls interspersed with sixteen alternating AB pairs",
        "platform": platform.platform(),
        "repetitions": args.repetitions,
        "complete": False,
        "executables": {},
        "calls": [],
    }
    lines = []
    work = {}
    error = None
    with tempfile.TemporaryDirectory(prefix="rrrah-cell-cpu-") as temporary:
        frozen = {}
        for name, source in [("before", args.before), ("after", args.after)]:
            data = source.read_bytes()
            destination = Path(temporary) / name
            destination.write_bytes(data)
            destination.chmod(0o500)
            frozen[name] = destination
            result["executables"][name] = {
                "source": str(source.resolve()),
                "sha256": hashlib.sha256(data).hexdigest(),
            }
        if result["executables"]["before"]["sha256"] == result["executables"]["after"]["sha256"]:
            parser.error("before and after executables are byte-identical")

        def run(name, phase, pair):
            completed = subprocess.run(
                [str(frozen[name]), test_name, "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                env=dict(os.environ, RRRAH_CELL_CPU_REPETITIONS=str(args.repetitions)),
                capture_output=True, text=True, timeout=60,
            )
            match = re.search(rf"DENSE_FIXED {re.escape(args.workload)} elapsed_ns=(\d+) repetitions=(\d+) total_work=(\d+)", completed.stdout)
            if completed.returncode or not match or "test result: ok. 1 passed; 0 failed; 0 ignored;" not in completed.stdout:
                raise RuntimeError(f"{name} kernel failed: {completed.stdout[-2000:]} {completed.stderr[-2000:]}")
            elapsed, repetitions, spent = map(int, match.groups())
            if elapsed <= 0 or repetitions != args.repetitions:
                raise ValueError("invalid native timing/repetition count")
            if not 0 < spent <= 100_000_000 * repetitions:
                raise ValueError("invalid native work count")
            if name in work and spent != work[name]:
                raise ValueError("native work count changed between identical inputs")
            work[name] = spent
            result["calls"].append(dict(variant=name, phase=phase, pair=pair, elapsed_ns=elapsed, total_work=spent))
            return elapsed

        try:
            run("before", "warmup", 0)
            run("after", "warmup", 0)
            control = 0
            for pair in range(16):
                if pair % 3 == 0:
                    first = run("before", "control", control)
                    second = run("before", "control", control)
                    lines.append(f"DENSE_CPU {args.workload} control pair={control} first_ns={first} second_ns={second}")
                    control += 1
                order = ["before", "after"] if pair % 2 == 0 else ["after", "before"]
                values = {name: run(name, "candidate", pair) for name in order}
                lines.append(f"DENSE_CPU {args.workload} candidate pair={pair} before_ns={values['before']} after_ns={values['after']} repetitions={args.repetitions}")
            assert control == 6
            analyze = runpy.run_path(str(Path(__file__).with_name("analyze-split-cpu.py")))["analyze"]
            result["analysis"] = analyze("\n".join(lines))
            stats = result["analysis"]["workloads"][args.workload]
            low, high = stats["control"]["bootstrap_mean_95_interval"]
            candidate_low = stats["candidate"]["bootstrap_mean_95_interval"][0]
            result["supports_local_speedup"] = (
                candidate_low > 0 and low <= 0 <= high and max(abs(low), abs(high)) < candidate_low
            )
            result["gate"] = "Candidate interval strictly positive; AA interval contains zero and its absolute envelope is below candidate lower bound"
            result["complete"] = True
        except Exception as failure:
            error = failure
            result["error"] = str(failure)
        finally:
            args.log.parent.mkdir(parents=True, exist_ok=True)
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.log.write_text("\n".join(lines) + "\n")
            result["log_sha256"] = hashlib.sha256(args.log.read_bytes()).hexdigest()
            args.report.write_text(json.dumps(result, indent=2) + "\n")
    if error:
        raise error
    print(json.dumps({"complete": result["complete"], "supports_local_speedup": result["supports_local_speedup"], "workload": result["analysis"]["workloads"][args.workload]}))


if __name__ == "__main__":
    main()
