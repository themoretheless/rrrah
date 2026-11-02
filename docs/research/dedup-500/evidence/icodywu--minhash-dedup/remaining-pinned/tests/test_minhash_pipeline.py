import gzip
import hashlib
import json
import os
import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from minhash_dedup.minhash import (
    MinhashConfig,
    BucketStage,
    SelectionStage,
    FilterStage,
    PruneStage,
    SignatureStage,
)
from minhash_dedup.runtime import Document
from minhash_dedup.signatures import HashConfig


class TestMinhashPipeline(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)

    def test_constructors_preserve_supported_options(self):
        config = MinhashConfig(n_grams=3, num_buckets=2, hashes_per_bucket=2)
        signatures, buckets, clusters = [self.root / name for name in ("signatures", "buckets", "clusters")]
        signature = SignatureStage(signatures, config, "eng")
        bucket = BucketStage(signatures, buckets, config, 123)
        cluster = SelectionStage(signatures, buckets, clusters, config, False, 456, True)
        for stage in (signature, bucket, cluster):
            self.assertIs(stage.config, config)
        self.assertEqual(signature.num_hashes, 4)
        self.assertEqual(signature.language, "eng")
        self.assertEqual(bucket.lines_to_buffer, 123)
        self.assertEqual(cluster.lines_to_buffer, 456)
        self.assertFalse(cluster.save_cluster_size)
        self.assertTrue(cluster.analyze_clustering)

    def test_constructors_reject_removed_noop_options(self):
        cases = [
            (SignatureStage, (self.root,), "skip_existing_sigs", False),
            (BucketStage, (self.root, self.root), "index_folder", None),
            (BucketStage, (self.root, self.root), "only_dedup_in_index", False),
            (BucketStage, (self.root, self.root), "create_index_name", None),
            (SelectionStage, (self.root, self.root, self.root), "ignore_index_matches", False),
        ]
        for constructor, args, option, value in cases:
            with self.subTest(constructor=constructor.__name__, option=option):
                with self.assertRaisesRegex(TypeError, option):
                    constructor(*args, **{option: value})
                self.assertFalse(hasattr(constructor(*args), option))

    def test_binary_outputs_match_pre_consolidation_fixture(self):
        # Captured from the preserved research implementation before merging support modules.
        expected = {
            32: {
                "signatures": "ea56f3a166990d6972896a292454885fdec9a66e19e2aa730ba620fbd64e0787",
                "buckets": "a77d21ffae852aa6256da4b547130f1ffb8487ee16498727af14cb0004fc38f9",
            },
            64: {
                "signatures": "429ec7aadd8ed3ba43221728a67f2332672670671df7cd07c134250cbd240a4f",
                "buckets": "0f75d7bd5b352efeefcf9764583ceaccd3a39020a97dedf7bc80e199140040dd",
            },
        }
        texts = [
            "alpha beta gamma delta epsilon",
            "alpha beta gamma delta epsilon",
            "completely different strings here",
            "completely different strings here",
            "another unrelated sentence",
            "a",
        ]
        for bits in (32, 64):
            with self.subTest(bits=bits):
                root = self.root / str(bits)
                config = MinhashConfig(
                    n_grams=3, num_buckets=2, hashes_per_bucket=2, hash_config=HashConfig(bits, "sha1")
                )
                signatures, buckets, pruned, clusters = [
                    root / name for name in ("signatures", "buckets", "pruned", "clusters")
                ]
                for rank in range(2):
                    docs = [Document(text, f"{rank}-{index}") for index, text in enumerate(texts[rank::2])]
                    SignatureStage(signatures, config).run(docs, rank, 2)
                for rank in range(2):
                    BucketStage(signatures, buckets, config=config).run(rank=rank, world_size=2)
                for rank in range(2):
                    PruneStage(buckets, pruned).run(rank=rank, world_size=2)
                stage = SelectionStage(signatures, pruned, clusters, config)
                stage.compute_punct_bound()
                stage.compute_cov_bound()
                stage.run()
                digests = expected[bits] | {
                    "pruned": "6f7c891393e4ac9d3c8b8bb7c98882f71849a9ae6b17f93ec44c407ce17c36f0",
                    "clusters": "0a7e577aae6c7db57e339b84f7af73db37e708094aeb355acba585ba12bd2f4e",
                }
                for category, expected_digest in digests.items():
                    digest = hashlib.sha256()
                    for path in sorted((root / category).rglob("*")):
                        if path.is_file():
                            digest.update(path.relative_to(root / category).as_posix().encode())
                            digest.update(path.read_bytes())
                    self.assertEqual(digest.hexdigest(), expected_digest, category)
                kept = []
                for rank in range(2):
                    docs = [Document(text, f"{rank}-{index}") for index, text in enumerate(texts[rank::2])]
                    kept.extend(FilterStage(clusters).run(docs, rank, 2))
                self.assertEqual({doc.id for doc in kept}, {"0-0", "0-1", "0-2", "1-2"})
                for doc in kept[:2]:
                    self.assertEqual(doc.metadata["dup_signals"]["dup_doc_count"], 1)

    def test_all_stage_entrypoints_with_two_reader_ranks(self):
        scripts = Path(__file__).resolve().parents[1] / "scripts"
        inputs, output = self.root / "input", self.root / "output"
        inputs.mkdir()
        for rank in range(2):
            rows = [
                {
                    "text": "The exact duplicate text must collapse to a single retained document.",
                    "meta": {"id": f"duplicate-{rank}", "source": "fixture"},
                    "subset": "study",
                },
                {"text": "a" * 30 if rank == 0 else "z" * 30, "meta": {"id": f"unique-{rank}"}, "subset": "study"},
            ]
            (inputs / f"{rank}.jsonl").write_text("".join(json.dumps(row) + "\n" for row in rows))
        environment = os.environ | {
            "S3_INPUT": json.dumps([str(inputs)]),
            "DATA_LABEL": '["fixture"]',
            "OUTPUT_PATH": str(output),
            "MINHASH_NUM_BUCKETS": "2",
            "MINHASH_HASHES_PER_BUCKET": "2",
            "MINHASH_SEED": "1",
            "SLURM_NTASKS": "2",
            "TASK_SIZE": "2",
            "MINHASH_ANALYZE_CLUSTERING": "false",
            "NUMBA_NUM_THREADS": "2",
        }
        for stage in ("1", "2", "2.5", "3", "4"):
            for rank in [0] if stage == "3" else [0, 1]:
                command = [sys.executable, str(scripts / f"minhash_stage{stage}.py")]
                if stage != "3":
                    command += ["--task-id", str(rank)]
                if stage == "4":
                    command += ["--task-size", "2"]
                result = subprocess.run(
                    command,
                    env=environment,
                    cwd=self.root,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    timeout=120,
                )
                self.assertEqual(result.returncode, 0, result.stdout)
        records = {}
        for category in ("deduped", "removed"):
            records[category] = []
            for path in sorted((output / category).rglob("*.jsonl.gz")):
                with gzip.open(path, "rt") as stream:
                    records[category].extend(json.loads(line) for line in stream)
        self.assertEqual(len(records["deduped"]), 3)
        self.assertEqual(len(records["removed"]), 1)
        all_rows = records["deduped"] + records["removed"]
        self.assertEqual(
            {row["meta"]["id"] for row in all_rows}, {"duplicate-0", "duplicate-1", "unique-0", "unique-1"}
        )
        for row in all_rows:
            self.assertEqual(row["subset"], "study")
            self.assertEqual(row["meta"]["data_label"], "fixture")
            if row["meta"]["id"].startswith("duplicate"):
                self.assertEqual(row["meta"]["dup_signals"]["dup_doc_count"], 1)

    def test_residual_cover_export_and_equal_support_aliases(self):
        buckets, cover, clusters = [self.root / name for name in ("buckets", "cover", "clusters")]
        buckets.mkdir()
        # All keys have degree three; keys 0 and 1 have identical support.
        bands = [[(0, 1, 2), (3, 4)], [(0, 1, 3), (2, 4)], [(0, 1, 4), (2, 3)]]
        for band, rows in enumerate(bands):
            with (buckets / f"{band:03d}_00.dups").open("wb") as stream:
                for row in rows:
                    stream.write(struct.pack("<I", len(row)))
                    for key in row:
                        stream.write(struct.pack("<2I", 0, key))
        stage = SelectionStage(self.root / "signatures", buckets, cover)
        stage.compute_punct_bound()
        self.assertEqual(stage.compute_cov_bound(), (2, 2))
        SelectionStage(self.root / "signatures", buckets, clusters).run()
        # Both digests were captured from the pre-consolidation implementation.
        expected = {
            cover: "9d00739b973ca4db2fe069b4e773a5126561c91b5b5886df0b2f715002b72dde",
            clusters: "737a70621d18c2234efb070f7f3761f0cc03a20ca111d8201f4f933b980a250a",
        }
        for directory, expected_digest in expected.items():
            digest = hashlib.sha256()
            for path in sorted(directory.rglob("*")):
                if path.is_file():
                    digest.update(path.relative_to(directory).as_posix().encode())
                    digest.update(path.read_bytes())
            self.assertEqual(digest.hexdigest(), expected_digest)
        documents = [Document(str(key), str(key)) for key in range(5)]
        kept = list(FilterStage(clusters).run(documents))
        self.assertEqual([doc.id for doc in kept], ["0"])
        self.assertEqual(kept[0].metadata["dup_signals"]["dup_doc_count"], 4)

    def test_source_priority_ties_preserve_bucket_feasibility(self):
        preferred, general, third = (0, 99), (1, 0), (2, 0)
        fourth, fifth, sixth = (3, 0), (4, 0), (5, 0)
        cases = [
            ("incidence_twins", [[general, preferred], [preferred, general]], {preferred}),
            ("root_score_tie", [[general, preferred], [third, general], [preferred, third]], {preferred}),
            ("structural_priority", [[preferred, general], [preferred, third]], {general, third}),
            (
                "collision_score_before_source",
                [
                    [preferred, general, third],
                    [preferred, fourth],
                    [general, third],
                    [fourth, fifth, sixth],
                    [fifth, sixth],
                ],
                {general, fourth},
            ),
        ]
        for name, rows, expected_kept in cases:
            with self.subTest(case=name):
                root = self.root / name
                buckets, clusters = root / "buckets", root / "clusters"
                buckets.mkdir(parents=True)
                for band, row in enumerate(rows):
                    with (buckets / f"{band:03d}_00.dups").open("wb") as stream:
                        stream.write(struct.pack("<I", len(row)))
                        for read_id, doc_id in row:
                            stream.write(struct.pack("<2I", read_id, doc_id))
                SelectionStage(root / "signatures", buckets, clusters).run()
                removed = {
                    (int(path.stem), doc_id)
                    for path in clusters.glob("*.remove")
                    for (doc_id,) in struct.iter_unpack("<I", path.read_bytes())
                }
                documents = {key for row in rows for key in row}
                kept = documents - removed
                self.assertEqual(kept, expected_kept)
                for row in rows:
                    self.assertLessEqual(len(kept.intersection(row)), 1)


if __name__ == "__main__":
    unittest.main()
