#!/usr/bin/env python3
"""Summarize alternating local split CPU probes; no page/display inference."""
import argparse
import json
from pathlib import Path
import random
import re
import statistics


def summarize(rows, seed=20261021):
    if not rows:
        return None
    values = [100 * (a - b) / a for a, b in rows]
    rng = random.Random(seed)
    bootstrap = sorted(statistics.mean(rng.choices(values, k=len(values)))
                       for _ in range(20000))
    return {"pairs": len(rows), "mean_percent_reduction": statistics.mean(values),
            "median_percent_reduction": statistics.median(values),
            "bootstrap_mean_95_interval": [bootstrap[500], bootstrap[19499]],
            "first_median_ns": statistics.median(a for a, _ in rows),
            "second_median_ns": statistics.median(b for _, b in rows)}


def analyze(text):
    result = {}
    for label, prefix in [("triangles", "SPLIT_CPU"), ("pair", "DENSE_CPU pair"),
                          ("dense50", "DENSE_CPU dense50"), ("dense75", "DENSE_CPU dense75"),
                          ("sparse64", "DENSE_CPU sparse64")]:
        def pairs(kind, first, second):
            return [tuple(map(int, row)) for row in re.findall(
                re.escape(prefix) + rf" {kind} pair=\d+ {first}=(\d+) {second}=(\d+)", text)]
        control = pairs("control", "first_ns", "second_ns")
        candidate = pairs("candidate", "before_ns", "after_ns")
        if control or candidate:
            if len(control) != 6 or len(candidate) != 16:
                raise ValueError(f"incomplete probe for {label}")
            result[label] = {"control": summarize(control), "candidate": summarize(candidate)}
    if not result:
        raise ValueError("no completed CPU probes")
    return {"scope": "Local optimized-test-build elapsed CPU-kernel time; no page/display/GPU claim",
            "estimator": "Mean paired percentage reduction; deterministic percentile bootstrap, 20000 resamples",
            "workloads": result}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    args = parser.parse_args()
    print(json.dumps(analyze(args.log.read_text()), indent=2))
