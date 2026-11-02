import gzip
import json
import os
import subprocess
import sys
import tempfile
import unittest
from dataclasses import asdict
from pathlib import Path

from minhash_dedup.partition import ReaderPlan
from minhash_dedup.minhash import MinhashConfig


class TestElasticStages(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.scripts = Path(__file__).resolve().parents[1] / "scripts"
        self.inputs = [self.root / name for name in ("plain", "empty", "compressed")]
        self.expected_ids = set()
        document_number = 0
        for directory, count in zip(self.inputs, (2, 0, 4)):
            directory.mkdir()
            for index in range(count):
                prefix = f"{directory.name}-{index}"
                texts = {
                    "shared": "The common duplicate text appears in every source file in this fixture.",
                    "unique": chr(65 + document_number) * 40,
                    "short": "x",
                }
                rows = [
                    {"text": text, "meta": {"id": f"{prefix}-{kind}"}, "subset": "study"}
                    for kind, text in texts.items()
                ]
                self.expected_ids.update(row["meta"]["id"] for row in rows)
                payload = 'not json\n{"text":""}\n' + "".join(json.dumps(row) + "\n" for row in rows)
                if directory.name == "compressed":
                    with gzip.open(directory / f"{index}.jsonl.gz", "wt") as stream:
                        stream.write(payload)
                else:
                    (directory / f"{index}.jsonl").write_text(payload)
                document_number += 1

    def run_stage(self, stage, output, launched=1, rank=None, task_size=None, extra=(), succeeds=True, task_env=""):
        environment = os.environ | {
            "S3_INPUT": json.dumps([str(path) for path in self.inputs]),
            "DATA_LABEL": '["plain", "empty", "compressed"]',
            "OUTPUT_PATH": str(output),
            "MINHASH_NUM_BUCKETS": "2",
            "MINHASH_HASHES_PER_BUCKET": "2",
            "MINHASH_SEED": "1",
            "MINHASH_ANALYZE_CLUSTERING": "false",
            "SLURM_NTASKS": str(launched),
            "TASK_SIZE": task_env,
            "NUMBA_NUM_THREADS": "2",
        }
        command = [sys.executable, str(self.scripts / f"minhash_stage{stage}.py")]
        if rank is not None:
            command += ["--task-id", str(rank)]
        if task_size is not None:
            command += ["--task-size", str(task_size)]
        result = subprocess.run(
            command + list(extra),
            cwd=self.root,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=120,
        )
        if succeeds:
            self.assertEqual(result.returncode, 0, result.stdout)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout)
        return result.stdout

    def pipeline(self, output, world_size, stage1_workers, stage4_workers):
        self.run_stage("1", output, stage1_workers, task_size=world_size, extra=("--prepare-only",))
        for rank in range(stage1_workers):
            self.run_stage("1", output, stage1_workers, rank)
        self.run_stage("1", output, extra=("--check-complete",))
        for stage in ("2", "2.5"):
            self.run_stage("_state", output, 2, extra=("begin", stage))
            for rank in range(2):
                self.run_stage(stage, output, 2, rank)
            self.run_stage("_state", output, extra=("complete", stage))
        self.run_stage("_state", output, extra=("begin", "3"))
        self.run_stage("3", output)
        self.run_stage("_state", output, extra=("complete", "3"))
        self.run_stage("_state", output, stage4_workers, extra=("begin", "4"))
        for rank in range(stage4_workers):
            self.run_stage("4", output, stage4_workers, rank)
        self.run_stage("_state", output, extra=("complete", "4"))
        self.run_stage("_state", output, extra=("check", "4"))

    def test_stage2_completion_requires_every_worker_output(self):
        output = self.root / "partial-stage2"
        for rank in range(2):
            self.run_stage("1", output, 2, rank, task_size=2)
        self.run_stage("_state", output, 2, extra=("begin", "2"))
        self.run_stage("2", output, 2, 0)
        message = self.run_stage("_state", output, extra=("complete", "2"), succeeds=False)
        self.assertIn("did not produce buckets/001_00.dups", message)
        self.run_stage("_state", output, extra=("check", "2"), succeeds=False)
        self.run_stage("2", output, 2, 1)
        self.run_stage("_state", output, extra=("complete", "2"))
        self.run_stage("_state", output, extra=("check", "2"))
        self.run_stage("_state", output, 4, extra=("begin", "2"))
        self.assertFalse((output / "buckets").exists())
        self.run_stage("_state", output, extra=("check", "2"), succeeds=False)

    def test_retry_refuses_to_clear_output_overlapping_source_data(self):
        output = self.inputs[0]
        signatures = output / "signatures"
        plan = ReaderPlan.prepare(
            signatures, self.inputs, 2, 2,
            asdict(MinhashConfig(num_buckets=2, hashes_per_bucket=2)),
        )
        for rank in range(plan.active_ranks):
            plan.complete_rank(signatures, rank)
        protected = output / "buckets" / "keep.jsonl"
        protected.parent.mkdir()
        protected.write_text("source data")
        message = self.run_stage("_state", output, 2, extra=("begin", "2"), succeeds=False)
        self.assertIn("overlapping source data", message)
        self.assertEqual(protected.read_text(), "source data")

    def artifact_bytes(self, output, category, pattern="*"):
        directory = output / category
        return {
            path.relative_to(directory).as_posix(): path.read_bytes()
            for path in sorted(directory.rglob(pattern))
            if path.is_file()
        }

    def records(self, output, category):
        records = []
        for path in sorted((output / category).rglob("*.jsonl.gz")):
            with gzip.open(path, "rt") as stream:
                records.extend(json.loads(line) for line in stream)
        return sorted(records, key=lambda row: row["meta"]["id"])

    def test_elastic_pipeline_matches_one_worker_per_logical_rank(self):
        for world_size, stage1_workers, stage4_workers in ((4, 3, 2), (8, 2, 4)):
            with self.subTest(world_size=world_size):
                baseline = self.root / f"baseline-{world_size}"
                elastic = self.root / f"elastic-{world_size}"
                self.pipeline(baseline, world_size, world_size, world_size)
                self.pipeline(elastic, world_size, stage1_workers, stage4_workers)
                plan = ReaderPlan.load(elastic / "signatures")
                self.assertEqual(plan.world_size, world_size)
                self.assertEqual(plan.active_ranks, min(world_size, 6))
                self.assertEqual(
                    self.artifact_bytes(baseline, "signatures", "*.minhash.sig"),
                    self.artifact_bytes(elastic, "signatures", "*.minhash.sig"),
                )
                for category in ("buckets", "buckets_pruned", "remove_ids"):
                    self.assertEqual(self.artifact_bytes(baseline, category), self.artifact_bytes(elastic, category))
                for category in ("deduped", "removed"):
                    self.assertEqual(self.records(baseline, category), self.records(elastic, category))
                kept, removed = self.records(elastic, "deduped"), self.records(elastic, "removed")
                self.assertEqual(len(kept), 13)
                self.assertEqual(len(removed), 5)
                self.assertEqual({row["meta"]["id"] for row in kept + removed}, self.expected_ids)

    def test_stage4_rejects_wrong_world_size_and_incomplete_active_ranks(self):
        output = self.root / "incomplete"
        self.run_stage("1", output, launched=4, task_size=8, extra=("--prepare-only",))
        self.run_stage("1", output, launched=4, rank=0)
        message = self.run_stage("4", output, rank=0, task_size=6, succeeds=False)
        self.assertIn("world_size mismatch", message)
        message = self.run_stage("4", output, rank=0, succeeds=False)
        self.assertIn("logical rank 1 has not completed", message)
        self.assertFalse((output / "deduped").exists())
        self.run_stage("1", output, extra=("--check-complete",), succeeds=False)

    def test_invalid_launch_rank_does_not_create_a_plan(self):
        for rank, launched in ((-1, 2), (2, 2), (0, 0)):
            output = self.root / f"invalid-{rank}-{launched}"
            self.run_stage("1", output, launched, rank, task_size=8, succeeds=False)
            self.assertFalse((output / "signatures").exists())

    def test_task_size_environment_and_cli_precedence(self):
        output = self.root / "environment"
        self.run_stage("1", output, launched=2, task_env="8", extra=("--prepare-only",))
        plan = ReaderPlan.load(output / "signatures")
        self.assertEqual(plan.world_size, 8)
        self.assertEqual(plan.active_ranks, 6)
        message = self.run_stage("1", output, launched=2, rank=1, task_env="8")
        self.assertIn("logical rank 5", message)
        message = self.run_stage("1", output, launched=2, rank=0, task_env="8", task_size=4, succeeds=False)
        self.assertIn("requested 4, Stage 1 plan uses 8", message)
        message = self.run_stage("4", output, rank=0, task_env="6", succeeds=False)
        self.assertIn("requested 6, Stage 1 plan uses 8", message)


if __name__ == "__main__":
    unittest.main()
