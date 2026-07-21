#!/usr/bin/env python3
"""Summarise RAW benchmark CSV/JSONL with uncertainty and regression gates.

The script deliberately has no third-party dependencies.  It keeps every raw
sample in the report, flags (rather than silently deleting) robust outliers,
and uses a deterministic percentile bootstrap for confidence intervals.

Examples:
  scripts/bench-report.py target/bench/results.csv --json-out report.json
  scripts/bench-report.py target/bench/results.csv --warmup 2 --exclude-outliers
  scripts/bench-report.py results.csv --baseline-json baseline.json
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import json
import math
import random
import statistics
import sys
import copy
from collections import defaultdict
from pathlib import Path
from typing import Any, Iterable


def percentile(values: list[float], q: float) -> float:
    values = sorted(values)
    if not values:
        return math.nan
    if len(values) == 1:
        return values[0]
    position = (len(values) - 1) * q
    lo, hi = math.floor(position), math.ceil(position)
    if lo == hi:
        return values[lo]
    return values[lo] + (values[hi] - values[lo]) * (position - lo)


def finite_number(value: Any) -> float | None:
    try:
        number = float(value)
    except (TypeError, ValueError):
        return None
    return number if math.isfinite(number) else None


def read_rows(path: Path) -> list[dict[str, Any]]:
    """Read the existing CSV harness format or one JSON object per line."""
    def normalize(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
        # scripts/bench-harness.py stores an immutable host manifest alongside
        # a nested sample. Flatten only benchmark dimensions here; the original
        # record remains available to callers that need hardware metadata.
        normalized: list[dict[str, Any]] = []
        for item in items:
            sample = item.get("sample")
            if isinstance(sample, dict):
                fixture = sample.get("fixture")
                fixture_path = fixture.get("path", "") if isinstance(fixture, dict) else fixture
                metrics = sample.get("metrics")
                metrics = metrics if isinstance(metrics, dict) else {}
                row = dict(item)
                row.update(sample)
                row["fixture"] = fixture_path
                row["latency_ms"] = sample.get("wall_ms")
                row.update(metrics)
                normalized.append(row)
            else:
                normalized.append(item)
        return normalized

    if path.suffix.lower() in {".json", ".jsonl"}:
        with path.open(encoding="utf-8") as stream:
            first = stream.read(256).lstrip()[:1]
            stream.seek(0)
            if first == "[":
                payload = json.load(stream)
                return normalize([item for item in payload if isinstance(item, dict)])
            # Benchmark artifacts may be truncated by a killed process. Skip
            # malformed JSONL records rather than turning report generation
            # into an unbounded/fragile parser; valid rows remain auditable and
            # the caller can compare the resulting count with the manifest.
            rows: list[dict[str, Any]] = []
            for line in stream:
                if not line.strip():
                    continue
                try:
                    item = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if isinstance(item, dict):
                    rows.append(item)
            return normalize(rows)
    with path.open(newline="", encoding="utf-8") as stream:
        return list(csv.DictReader(stream))


def bootstrap_ci(
    values: list[float], statistic: str, *, samples: int, seed: int
) -> tuple[float, float]:
    """95% percentile bootstrap CI; deterministic for reproducible CI runs."""
    if len(values) < 2 or samples <= 0:
        value = statistic_value(values, statistic)
        return value, value
    rng = random.Random(seed)
    estimates: list[float] = []
    for _ in range(samples):
        resample = [values[rng.randrange(len(values))] for _ in values]
        estimates.append(statistic_value(resample, statistic))
    return percentile(estimates, 0.025), percentile(estimates, 0.975)


def statistic_value(values: list[float], statistic: str) -> float:
    if statistic == "mean":
        return statistics.fmean(values)
    if statistic == "p50":
        return percentile(values, 0.50)
    if statistic == "p95":
        return percentile(values, 0.95)
    if statistic == "p99":
        return percentile(values, 0.99)
    raise ValueError(f"unsupported statistic: {statistic}")


def robust_outliers(values: list[float], threshold: float = 3.5) -> list[int]:
    """Return indices whose modified z-score exceeds threshold.

    MAD=0 is common for timer samples with coarse clocks; in that case only
    values different from the median are flagged, never discarded silently.
    """
    if len(values) < 4:
        return []
    median = percentile(values, 0.5)
    deviations = [abs(value - median) for value in values]
    mad = percentile(deviations, 0.5)
    if mad == 0:
        return [i for i, value in enumerate(values) if value != median]
    return [
        i
        for i, value in enumerate(values)
        if abs(0.67448975 * (value - median) / mad) > threshold
    ]


def trim_mean(values: list[float], proportion: float = 0.10) -> float:
    if not values:
        return math.nan
    ordered = sorted(values)
    cut = min(int(len(ordered) * proportion), (len(ordered) - 1) // 2)
    core = ordered[cut : len(ordered) - cut] or ordered
    return statistics.fmean(core)


def sample_metrics(
    values: list[float], *, bootstrap_samples: int, seed: int
) -> dict[str, Any]:
    outlier_indices = robust_outliers(values)
    outliers = [values[i] for i in outlier_indices]
    filtered = [v for i, v in enumerate(values) if i not in set(outlier_indices)]
    if not filtered:
        filtered = values
    metrics: dict[str, Any] = {
        "n": len(values),
        "n_used": len(filtered),
        "outlier_count": len(outliers),
        "outlier_values_ms": [round(v, 6) for v in outliers],
        "min_ms": min(values),
        "max_ms": max(values),
        "mean_ms": statistics.fmean(values),
        "trimmed_mean_ms": trim_mean(values),
        "stdev_ms": statistics.stdev(values) if len(values) > 1 else 0.0,
        "mad_ms": percentile([abs(v - percentile(values, 0.5)) for v in values], 0.5),
        "p50_ms": percentile(values, 0.50),
        "p95_ms": percentile(values, 0.95),
        "p99_ms": percentile(values, 0.99),
    }
    for name in ("mean", "p50", "p95", "p99"):
        lo, hi = bootstrap_ci(values, name, samples=bootstrap_samples, seed=seed + len(name))
        metrics[f"{name}_ci95_ms"] = [lo, hi]
    if filtered != values and len(filtered) >= 1:
        metrics["filtered_p50_ms"] = percentile(filtered, 0.50)
        metrics["filtered_p95_ms"] = percentile(filtered, 0.95)
    return metrics


def key_for(row: dict[str, Any], fields: list[str]) -> tuple[str, ...]:
    return tuple(str(row.get(field, "")) for field in fields)


def json_safe(value: Any) -> Any:
    if isinstance(value, float) and not math.isfinite(value):
        return None
    if isinstance(value, dict):
        return {k: json_safe(v) for k, v in value.items()}
    if isinstance(value, list):
        return [json_safe(v) for v in value]
    return value


def load_baseline(path: Path) -> dict[tuple[str, ...], dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    groups = payload if isinstance(payload, list) else payload.get("groups", [])
    result: dict[tuple[str, ...], dict[str, Any]] = {}
    for group in groups:
        key = tuple(str(group.get(field, "")) for field in ("fixture", "mode", "workers", "backend"))
        result[key] = group
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", nargs="?", default="target/bench/results.csv")
    parser.add_argument("--json-out", type=Path, help="write machine-readable JSON report")
    parser.add_argument("--warmup", type=int, default=0, help="discard first N iterations per group")
    parser.add_argument("--bootstrap", type=int, default=10_000, help="bootstrap resamples (default: 10000)")
    parser.add_argument("--seed", type=int, default=0x52524148, help="deterministic bootstrap seed")
    parser.add_argument("--exclude-outliers", action="store_true", help="use MAD-filtered values for headline metrics")
    parser.add_argument("--baseline-json", type=Path, help="prior JSON report for regression comparison")
    parser.add_argument("--regression-threshold", type=float, default=0.05, help="relative slowdown threshold (default: 5%%)")
    args = parser.parse_args()
    if args.warmup < 0 or args.bootstrap < 0:
        parser.error("--warmup and --bootstrap must be non-negative")
    rows = read_rows(Path(args.input))
    # Keep additional dimensions when available; old CSVs simply use empty strings.
    fields = ["fixture", "mode", "workers", "backend"]
    grouped: dict[tuple[str, ...], list[tuple[int, float]]] = defaultdict(list)
    for ordinal, row in enumerate(rows):
        status = str(row.get("status", "0"))
        if status not in {"0", "ok", "success", ""}:
            continue
        value = finite_number(row.get("real_seconds", row.get("latency_ms")))
        if value is None:
            continue
        # real_seconds is the legacy CSV unit; JSONL latency_ms is explicit.
        milliseconds = value * 1000.0 if row.get("real_seconds") is not None else value
        iteration = int(row.get("iteration", ordinal + 1) or ordinal + 1)
        grouped[key_for(row, fields)].append((iteration, milliseconds))

    groups: list[dict[str, Any]] = []
    for key, samples in sorted(grouped.items()):
        samples.sort(key=lambda item: item[0])
        warmups = samples[: args.warmup]
        values = [value for _, value in samples[args.warmup :]]
        if not values:
            continue
        metrics = sample_metrics(values, bootstrap_samples=args.bootstrap, seed=args.seed)
        if args.exclude_outliers and metrics.get("n_used", len(values)) < len(values):
            # The unfiltered metrics remain available under ``all_samples``.
            outlier_indices = set(robust_outliers(values))
            all_metrics = copy.deepcopy(metrics)
            metrics = sample_metrics(
                [v for i, v in enumerate(values) if i not in outlier_indices],
                bootstrap_samples=args.bootstrap,
                seed=args.seed,
            )
            metrics["all_samples"] = all_metrics
        row: dict[str, Any] = dict(zip(fields, key))
        row.update(metrics)
        row["warmup_discarded"] = len(warmups)
        row["samples_ms"] = values
        row["warnings"] = []
        if len(values) < 30 and str(row.get("mode", "")).lower() not in {"export", "batch-export"}:
            row["warnings"].append("n<30: latency CI is exploratory")
        if len(values) < 10:
            row["warnings"].append("n<10: do not use for release gate")
        groups.append(row)

    # Compare modes within a fixture against the no-persistent-cache series.
    # The legacy CSV called this ``cold-no-cache``; retain both spellings while
    # keeping the newer name honest about OS page-cache state.
    for fixture in sorted({group["fixture"] for group in groups}):
        baseline = next(
            (
                g
                for g in groups
                if g["fixture"] == fixture
                and g["mode"] in {"cold-no-cache", "no-persistent-cache"}
            ),
            None,
        )
        if baseline is None:
            continue
        for group in groups:
            if group["fixture"] != fixture:
                continue
            group["speedup_vs_cold"] = baseline["p50_ms"] / group["p50_ms"] if group["p50_ms"] else None

    regressions: list[dict[str, Any]] = []
    if args.baseline_json:
        previous = load_baseline(args.baseline_json)
        for group in groups:
            key = tuple(str(group.get(field, "")) for field in fields)
            old = previous.get(key)
            if not old:
                continue
            for metric in ("p50_ms", "p95_ms"):
                before = finite_number(old.get(metric))
                after = finite_number(group.get(metric))
                if before is None or after is None or before <= 0:
                    continue
                change = (after - before) / before
                old_ci_raw = old.get(f"{metric.replace('_ms', '')}_ci95_ms", [before, before])
                new_ci_raw = group.get(f"{metric.replace('_ms', '')}_ci95_ms", [after, after])
                old_ci = [finite_number(item) for item in old_ci_raw]
                new_ci = [finite_number(item) for item in new_ci_raw]
                # Conservative ratio interval: numerator lower/upper over
                # denominator upper/lower. It is intentionally wider than a
                # paired bootstrap because process-boundary runs are usually
                # independent samples.
                ratio_ci = [
                    new_ci[0] / old_ci[1] if new_ci[0] is not None and old_ci[1] else None,
                    new_ci[1] / old_ci[0] if new_ci[1] is not None and old_ci[0] else None,
                ]
                ratio_crosses_one = (
                    ratio_ci[0] is not None and ratio_ci[1] is not None
                    and ratio_ci[0] <= 1.0 <= ratio_ci[1]
                )
                if change > args.regression_threshold:
                    status = "regression"
                elif change < -args.regression_threshold:
                    status = "improvement"
                else:
                    status = "inconclusive" if ratio_crosses_one else "ok"
                regressions.append({
                    "key": dict(zip(fields, key)),
                    "metric": metric,
                    "before_ms": before,
                    "after_ms": after,
                    "absolute_change_ms": after - before,
                    "ratio": after / before,
                    "before_ci95_ms": old_ci,
                    "after_ci95_ms": new_ci,
                    "ratio_ci95": ratio_ci,
                    "relative_change": change,
                    "status": status,
                })

    report: dict[str, Any] = {
        "schema": "rrrah.benchmark-report.v2",
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "input": str(args.input),
        "statistics": {
            "warmup": args.warmup,
            "bootstrap_samples": args.bootstrap,
            "confidence": 0.95,
            "outlier_rule": "modified_z_score > 3.5 using MAD; values are flagged, not silently removed",
            "headline_includes_outliers": not args.exclude_outliers,
        },
        "groups": groups,
        "regressions": regressions,
        "warnings": [
            "at least 30 post-warmup samples are required for latency release gates"
        ] if any(group.get("warnings") for group in groups) else [],
    }
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(json_safe(report), indent=2) + "\n", encoding="utf-8")

    print("fixture\tmode\tworkers\tbackend\tn\tp50_ms\tp95_ms\tp99_ms\tmean_ms\tstdev_ms\tMAD_ms\toutliers\tCI95_p50_ms")
    for group in groups:
        ci = group["p50_ci95_ms"]
        print(
            f"{group['fixture']}\t{group['mode']}\t{group['workers']}\t{group['backend']}\t"
            f"{group['n']}\t{group['p50_ms']:.3f}\t{group['p95_ms']:.3f}\t{group['p99_ms']:.3f}\t"
            f"{group['mean_ms']:.3f}\t{group['stdev_ms']:.3f}\t{group['mad_ms']:.3f}\t"
            f"{group['outlier_count']}\t[{ci[0]:.3f},{ci[1]:.3f}]"
        )
    if regressions:
        bad = [item for item in regressions if item["status"] == "regression"]
        print(f"regression_checks={len(regressions)} regressions={len(bad)}", file=sys.stderr)
        return 1 if bad else 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
