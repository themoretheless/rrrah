"""Deterministic tests for the dependency-free benchmark report.

Run with ``python3 -m unittest discover -s scripts -p 'test_*.py'``.  These
tests intentionally exercise the schema without a RAW fixture or a GPU.
"""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("bench-report.py")
SPEC = importlib.util.spec_from_file_location("rrrah_bench_report", SCRIPT)
assert SPEC and SPEC.loader
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)


class BenchmarkReportTests(unittest.TestCase):
    def test_percentile_interpolates_and_empty_is_nan(self) -> None:
        self.assertEqual(REPORT.percentile([1.0], 0.95), 1.0)
        self.assertAlmostEqual(REPORT.percentile([1.0, 2.0, 4.0, 8.0], 0.5), 3.0)
        self.assertTrue(REPORT.math.isnan(REPORT.percentile([], 0.5)))

    def test_non_finite_values_are_not_valid_samples(self) -> None:
        self.assertEqual(REPORT.finite_number("12.5"), 12.5)
        self.assertIsNone(REPORT.finite_number("nan"))
        self.assertIsNone(REPORT.finite_number(float("inf")))
        self.assertIsNone(REPORT.finite_number("not-a-number"))

    def test_mad_rule_flags_outlier_without_silently_deleting_it(self) -> None:
        values = [10.0, 10.1, 9.9, 10.0, 100.0]
        indices = REPORT.robust_outliers(values)
        self.assertEqual(indices, [4])
        metrics = REPORT.sample_metrics(values, bootstrap_samples=32, seed=7)
        self.assertEqual(metrics["n"], 5)
        self.assertEqual(metrics["outlier_count"], 1)
        self.assertEqual(metrics["outlier_values_ms"], [100.0])

    def test_jsonl_normalization_preserves_manifest_and_sample_dimensions(self) -> None:
        row = {
            "schema": "rrrah-bench-v1",
            "host": {"cpu": "synthetic"},
            "sample": {
                "fixture": {"path": "/tmp/test.cr2"},
                "mode": "warm-persistent-cache",
                "workers": 4,
                "backend": "cpu",
                "iteration": 1,
                "status": 0,
                "wall_ms": 12.0,
                "metrics": {"cache_hit": True},
            },
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sample.jsonl"
            path.write_text(json.dumps(row) + "\n", encoding="utf-8")
            rows = REPORT.read_rows(path)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["fixture"], "/tmp/test.cr2")
        self.assertEqual(rows[0]["latency_ms"], 12.0)
        self.assertTrue(rows[0]["cache_hit"])
        self.assertEqual(rows[0]["host"]["cpu"], "synthetic")

    def test_truncated_jsonl_record_is_skipped_without_crashing_reporter(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "truncated.jsonl"
            path.write_text('{"sample": {"wall_ms": 3}}\n{"sample":\n', encoding="utf-8")
            rows = REPORT.read_rows(path)
        self.assertEqual(len(rows), 1)

    def test_json_safe_converts_non_finite_for_release_schema(self) -> None:
        safe = REPORT.json_safe({"p95": float("nan"), "nested": [float("inf"), 1.0]})
        self.assertEqual(safe, {"p95": None, "nested": [None, 1.0]})


if __name__ == "__main__":
    unittest.main()
