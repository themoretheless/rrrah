# -*- coding: utf-8 -*-
"""Comprehensive test runner for dmca-takedown-assistant.

This script runs all test suites in the correct order and provides
detailed reporting of test results.
"""
from __future__ import annotations

import json
import logging
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional


logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger(__name__)


class TestResult:
    """Individual test result."""
    def __init__(
        self,
        name: str,
        success: bool,
        duration_ms: float,
        output: str,
        error: Optional[str] = None,
    ):
        self.name = name
        self.success = success
        self.duration_ms = duration_ms
        self.output = output
        self.error = error


class TestSuite:
    """Collection of test results."""

    def __init__(self, name: str):
        self.name = name
        self.results: List[TestResult] = []
        self.start_time: Optional[datetime] = None
        self.end_time: Optional[datetime] = None

    def add_result(self, result: TestResult) -> None:
        """Add a test result."""
        self.results.append(result)

    @property
    def total_duration_ms(self) -> float:
        """Total duration for all tests."""
        return sum(r.duration_ms for r in self.results)

    @property
    def passed_count(self) -> int:
        """Number of passed tests."""
        return sum(1 for r in self.results if r.success)

    @property
    def failed_count(self) -> int:
        """Number of failed tests."""
        return sum(1 for r in self.results if not r.success)

    @property
    def success_rate(self) -> float:
        """Success rate (0.0-1.0)."""
        if not self.results:
            return 1.0
        return self.passed_count / len(self.results)


def run_test_script(
    script_path: Path,
    project_dir: Path,
    timeout: int = 300,
) -> TestResult:
    """Run a test script and capture results."""
    script_name = script_path.stem
    logger.info(f"Running test: {script_name}")

    start_time = datetime.now()
    try:
        result = subprocess.run(
            [sys.executable, str(script_path)],
            cwd=project_dir,
            capture_output=True,
            text=True,
            timeout=timeout,
        )

        duration_ms = (datetime.now() - start_time).total_seconds() * 1000
        success = result.returncode == 0

        return TestResult(
            name=script_name,
            success=success,
            duration_ms=duration_ms,
            output=result.stdout,
            error=result.stderr if result.stderr else None,
        )

    except subprocess.TimeoutExpired:
        duration_ms = (datetime.now() - start_time).total_seconds() * 1000
        logger.error(f"Test timed out: {script_name}")
        return TestResult(
            name=script_name,
            success=False,
            duration_ms=duration_ms,
            output="",
            error="Test timed out",
        )
    except Exception as e:
        duration_ms = (datetime.now() - start_time).total_seconds() * 1000
        logger.error(f"Test failed with exception: {e}")
        return TestResult(
            name=script_name,
            success=False,
            duration_ms=duration_ms,
            output="",
            error=str(e),
        )


def run_all_tests(
    project_dir: Optional[Path] = None,
    output_file: Optional[Path] = None,
) -> bool:
    """Run all test suites and generate report."""
    if project_dir is None:
        project_dir = Path(__file__).parent.parent

    logger.info("=" * 60)
    logger.info("DMCA Takedown Assistant - Test Suite Runner")
    logger.info("=" * 60)

    # Define test suites
    test_suites = [
        ("Unit Tests - Knowledge Updater", "tools/test_knowledge_updater.py"),
        ("Unit Tests - DMCA Suite", "tools/test_dmca_suite.py"),
        ("Project Validation", "tools/validate_project.py"),
        ("Test Scenarios", "tools/run_test_scenarios.py"),
    ]

    all_suites: List[TestSuite] = []

    # Run each test suite
    for suite_name, script_path in test_suites:
        suite = TestSuite(suite_name)
        suite.start_time = datetime.now()

        script = project_dir / script_path
        if not script.exists():
            logger.warning(f"Test script not found: {script}")
            continue

        result = run_test_script(script, project_dir)
        suite.add_result(result)
        suite.end_time = datetime.now()
        all_suites.append(suite)

        # Log immediate result
        status = "✓ PASS" if result.success else "✗ FAIL"
        logger.info(
            f"{status} - {suite_name} "
            f"({result.duration_ms:.0f}ms)"
        )

        # Show error output if failed
        if not result.success:
            logger.error(f"Error output:\n{result.error or result.output}")

    # Generate summary
    logger.info("=" * 60)
    logger.info("Test Summary")
    logger.info("=" * 60)

    total_tests = len(all_suites)
    total_passed = sum(s.passed_count for s in all_suites)
    total_failed = sum(s.failed_count for s in all_suites)
    total_duration = sum(s.total_duration_ms for s in all_suites)

    logger.info(f"Total test suites: {total_tests}")
    logger.info(f"Total passed: {total_passed}")
    logger.info(f"Total failed: {total_failed}")
    logger.info(f"Total duration: {total_duration:.0f}ms ({total_duration/1000:.1f}s)")

    # Detailed breakdown
    for suite in all_suites:
        logger.info("")
        logger.info(f"{suite.name}:")
        logger.info(f"  Status: {'PASS' if suite.failed_count == 0 else 'FAIL'}")
        logger.info(f"  Duration: {suite.total_duration_ms:.0f}ms")
        logger.info(f"  Success rate: {suite.success_rate:.1%}")

        for result in suite.results:
            status = "✓" if result.success else "✗"
            logger.info(f"    {status} {result.name} ({result.duration_ms:.0f}ms)")

    # Overall success
    overall_success = all(s.failed_count == 0 for s in all_suites)
    logger.info("")
    logger.info("=" * 60)
    if overall_success:
        logger.info("✓ ALL TESTS PASSED")
    else:
        logger.error("✗ SOME TESTS FAILED")
    logger.info("=" * 60)

    # Save report if requested
    if output_file:
        save_test_report(all_suites, output_file)

    return overall_success


def save_test_report(suites: List[TestSuite], output_path: Path) -> None:
    """Save test report to JSON file."""
    report = {
        "timestamp": datetime.now().isoformat(),
        "summary": {
            "total_suites": len(suites),
            "total_passed": sum(s.passed_count for s in suites),
            "total_failed": sum(s.failed_count for s in suites),
            "total_duration_ms": sum(s.total_duration_ms for s in suites),
        },
        "suites": [],
    }

    for suite in suites:
        suite_data = {
            "name": suite.name,
            "passed": suite.passed_count,
            "failed": suite.failed_count,
            "duration_ms": suite.total_duration_ms,
            "success_rate": suite.success_rate,
            "tests": [
                {
                    "name": r.name,
                    "success": r.success,
                    "duration_ms": r.duration_ms,
                    "error": r.error,
                }
                for r in suite.results
            ],
        }
        report["suites"].append(suite_data)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)

    logger.info(f"Test report saved to: {output_path}")


def main():
    """Main entry point."""
    import argparse

    parser = argparse.ArgumentParser(
        description="Run all dmca-takedown-assistant tests"
    )
    parser.add_argument(
        "--output",
        "-o",
        type=Path,
        help="Path to save test report JSON",
    )
    parser.add_argument(
        "--project-dir",
        type=Path,
        help="Project directory (default: auto-detect)",
    )
    parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Enable verbose output",
    )

    args = parser.parse_args()

    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)

    success = run_all_tests(
        project_dir=args.project_dir,
        output_file=args.output,
    )

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
