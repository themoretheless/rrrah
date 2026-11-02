import json
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict
from pathlib import Path
from unittest.mock import patch

from minhash_dedup.jsonl import JsonlReader
from minhash_dedup.partition import InputPartition, ReaderPlan, allocate_reader_tasks


class TestReaderAllocation(unittest.TestCase):
    def test_preserves_weighted_allocation_and_ties(self):
        cases = [
            ([10, 20, 30], [100, 200, 700], 10, [2, 2, 6]),
            ([1, 5, 10], [1000, 10, 10], 8, [1, 3, 4]),
            ([3, 3, 3], [100, 100, 100], 4, [1, 1, 2]),
            ([2, 3], [100, 200], 2048, [2, 3]),
            ([0, 5, 2], [0, 100, 5], 4, [0, 3, 1]),
            ([0, 0], [0, 0], 2048, [0, 0]),
        ]
        for counts, sizes, world_size, expected in cases:
            with self.subTest(counts=counts, world_size=world_size):
                self.assertEqual(allocate_reader_tasks(counts, sizes, world_size), expected)

    def test_invalid_allocations_fail(self):
        for counts, sizes, size in (([2], [2], 0), ([2, 2], [2, 2], 1), ([3], [0], 2), ([1], [], 1)):
            with self.subTest(counts=counts, sizes=sizes, world_size=size), self.assertRaises(ValueError):
                allocate_reader_tasks(counts, sizes, size)

    def test_elastic_ranks_cover_every_active_rank_exactly_once(self):
        for active in (0, 5, 1800, 2048):
            plan = ReaderPlan(2048, [InputPartition("unused", [], [], active)], {})
            for launched in (1, 3, 512, 1024, 4096):
                ranks = [rank for worker in range(launched) for rank in plan.ranks_for_worker(worker, launched)]
                self.assertEqual(sorted(ranks), list(range(active)))
        plan = ReaderPlan(2048, [InputPartition("unused", [], [], 2048)], {})
        self.assertEqual(list(plan.ranks_for_worker(0, 1024)), [0, 1024])
        self.assertEqual(list(plan.ranks_for_worker(511, 512)), [511, 1023, 1535, 2047])
        for rank, launched in ((0, 0), (-1, 2), (2, 2)):
            with self.assertRaises(ValueError):
                plan.ranks_for_worker(rank, launched)


class TestReaderPlan(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.inputs = [self.root / name for name in ("first", "empty", "second")]
        for directory, count in zip(self.inputs, (2, 0, 3)):
            directory.mkdir()
            for index in range(count):
                (directory / f"{index}.jsonl").write_text(json.dumps({"text": "sample", "id": str(index)}) + "\n")
        self.signatures = self.root / "signatures"

    def prepare(self, world_size=2048, launched=2):
        return ReaderPlan.prepare(self.signatures, self.inputs, world_size, launched, {"seed": 1})

    def test_world_size_is_not_replaced_by_active_count(self):
        plan = self.prepare()
        self.assertEqual(plan.world_size, 2048)
        self.assertEqual(plan.active_ranks, 5)
        self.assertEqual([source.workers for source in plan.inputs], [2, 0, 3])
        self.assertEqual(asdict(ReaderPlan.load(self.signatures)), asdict(plan))
        self.assertEqual(asdict(self.prepare(None, launched=4)), asdict(plan))
        self.assertEqual(plan.shard(2), (2, ["0.jsonl"]))

    def test_same_files_and_order_with_different_launch_sizes(self):
        plan = self.prepare(world_size=3)
        expected = [plan.shard(rank) for rank in range(plan.active_ranks)]
        for launched in (1, 2, 3, 5):
            actual = {}
            for worker in range(launched):
                for rank in plan.ranks_for_worker(worker, launched):
                    actual[rank] = plan.shard(rank)
            self.assertEqual([actual[rank] for rank in sorted(actual)], expected)

    def test_mismatch_fails_instead_of_repartitioning(self):
        plan = self.prepare()
        with self.assertRaisesRegex(ValueError, "world_size mismatch"):
            self.prepare(world_size=5)
        with self.assertRaisesRegex(ValueError, "directories or their order"):
            plan.validate(self.inputs[::-1])
        with self.assertRaisesRegex(ValueError, "configuration differs"):
            plan.validate(self.inputs, signature_config={"seed": 2})

    def test_planned_files_are_read_without_discovery(self):
        plan = self.prepare(world_size=2)
        index, files = plan.shard(0)
        (self.inputs[index] / "new.jsonl").write_text('{"text":"not in this run"}\n')
        reader = JsonlReader(plan.inputs[index].path, files=files)
        with patch.object(reader.data_folder, "list_files", side_effect=AssertionError("unexpected discovery")):
            self.assertEqual([document.id for document in reader.run()], ["0", "1"])
        self.assertEqual(plan.shard(0), (index, files))
        with self.assertRaises(ValueError):
            JsonlReader(self.inputs[0], files=[], paths_file="unused")

    def test_modified_or_missing_planned_file_is_rejected(self):
        plan = self.prepare()
        path = self.inputs[0] / "0.jsonl"
        path.write_text("changed")
        with self.assertRaisesRegex(ValueError, "Input size changed"):
            plan.shard(0)
        path.unlink()
        with self.assertRaises(FileNotFoundError):
            plan.shard(0)

    def test_completion_is_bound_to_plan_and_cleared_before_rewriting(self):
        plan = self.prepare()
        with self.assertRaisesRegex(RuntimeError, "has not completed"):
            plan.require_complete(self.signatures, 0)
        plan.complete_rank(self.signatures, 0)
        plan.require_complete(self.signatures, 0)
        changed = ReaderPlan(1024, plan.inputs, plan.signature_config)
        with self.assertRaisesRegex(ValueError, "does not match"):
            changed.require_complete(self.signatures, 0)
        plan.begin_rank(self.signatures, 0)
        with self.assertRaises(RuntimeError):
            plan.require_complete(self.signatures, 0)
        with self.assertRaises(ValueError):
            plan.complete_rank(self.signatures, 5)

    def test_missing_plan_is_not_inferred_from_legacy_signatures(self):
        bucket = self.signatures / "bucket_000"
        bucket.mkdir(parents=True)
        (bucket / "00000.minhash.sig").touch()
        with self.assertRaisesRegex(FileNotFoundError, "do not infer"):
            ReaderPlan.load(self.signatures)
        with self.assertRaisesRegex(ValueError, "no reader plan"):
            self.prepare()

    def test_failed_publication_leaves_no_partial_plan(self):
        with patch("minhash_dedup.partition.os.link", side_effect=OSError("fixture")):
            with self.assertRaises(OSError):
                self.prepare()
        self.assertEqual(list(self.signatures.iterdir()), [])

    def test_concurrent_workers_share_one_complete_plan(self):
        with ThreadPoolExecutor(max_workers=4) as workers:
            plans = list(workers.map(lambda launched: self.prepare(launched=launched), (1, 2, 3, 4)))
        self.assertTrue(all(asdict(plan) == asdict(plans[0]) for plan in plans))
        self.assertEqual(list(self.signatures.iterdir()), [ReaderPlan.path(self.signatures)])


if __name__ == "__main__":
    unittest.main()
