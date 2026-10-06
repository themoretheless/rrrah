import itertools
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import numpy as np

from minhash_dedup import minhash


@unittest.skipIf(minhash._numba is None, "numba is required for signature analysis")
class TestMinhashAnalysis(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp_dir.cleanup)
        self.root = Path(self.temp_dir.name)
        self.config = minhash.MinhashConfig(
            num_buckets=2,
            hashes_per_bucket=2,
            hash_config=minhash.HashConfig(precision=64, hash_fc="sha1"),
        )
        self.documents = {
            (0, 7): (1, 1, 9, 9),
            (0, 1_000_000): (1, 2, 9, 9),
            (0, (1 << 32) - 1): (9, 9, 1, 1),
        }
        self.addCleanup(minhash._numba.set_num_threads, minhash._numba.get_num_threads())

    def _write_band(self, band, read_id, records, filename=None):
        folder = self.root / f"bucket_{band:03d}"
        folder.mkdir(exist_ok=True)
        path = folder / (filename or f"{read_id:05d}.minhash.sig")
        record = struct.Struct(f"<3{self.config.hash_config.struct_format}I")
        with path.open("wb") as stream:
            for doc_id, hashes in sorted(records, key=lambda item: (item[1], item[0])):
                stream.write(record.pack(*hashes, 0, doc_id))

    def _write_signatures(self):
        for band in range(2):
            for read_id in sorted({key[0] for key in self.documents}):
                records = [
                    (doc_id, signature[2 * band : 2 * band + 2])
                    for (reader, doc_id), signature in self.documents.items()
                    if reader == read_id
                ]
                self._write_band(band, read_id, records)

    def _analyze(self, remove_keys=(), **kwargs):
        minhash.analyze_cluster_signature(
            list(remove_keys), str(self.root), self.config,
            num_workers=1, block_size=2, **kwargs,
        )

    def _capture_analysis(self, remove_keys=(), **kwargs):
        captured = {}
        kernel = minhash._hist_all_blocks_numba

        def capture(signatures, *args):
            captured["signatures"] = signatures.copy()
            captured["histogram"] = kernel(signatures, *args)
            return captured["histogram"]

        with patch.object(minhash, "_hist_all_blocks_numba", side_effect=capture):
            self._analyze(remove_keys, **kwargs)
        return captured

    def _assert_rows_and_histogram(self, captured, expected):
        self.assertEqual(
            sorted(map(tuple, captured["signatures"].tolist())), sorted(expected),
        )
        histogram = [0, 0, 0]
        for left, right in itertools.combinations(expected, 2):
            differences = sum(a != b for a, b in zip(left, right))
            histogram[min(differences, 2)] += 1
        np.testing.assert_array_equal(captured["histogram"], histogram)

    def test_independently_sorted_bands_preserve_close_pair(self):
        for precision, strict in itertools.product((32, 64), (False, True)):
            with self.subTest(precision=precision, strict=strict):
                self.config.hash_config = minhash.HashConfig(precision=precision, hash_fc="sha1")
                self._write_signatures()
                captured = self._capture_analysis(strict=strict)
                self._assert_rows_and_histogram(captured, list(self.documents.values()))
                np.testing.assert_array_equal(captured["histogram"], [0, 1, 2])

    def test_removing_documents_preserves_close_pair(self):
        self.documents[(0, 99)] = (0, 0, 2, 2)
        self._write_signatures()
        for strict in (False, True):
            with self.subTest(strict=strict):
                captured = self._capture_analysis([(0, 99)], strict=strict)
                expected = [sig for key, sig in self.documents.items() if key != (0, 99)]
                self._assert_rows_and_histogram(captured, expected)
                np.testing.assert_array_equal(captured["histogram"], [0, 1, 2])

    def test_doc_ids_are_scoped_to_reader(self):
        self.documents.update({(5, 7): (9, 8, 3, 4), (5, 1_000_000): (2, 3, 5, 6)})
        self._write_signatures()
        captured = self._capture_analysis([(0, 7)])
        expected = [sig for key, sig in self.documents.items() if key != (0, 7)]
        self._assert_rows_and_histogram(captured, expected)

    def test_sampling_uses_aligned_signatures(self):
        self._write_signatures()
        captured = self._capture_analysis(max_docs_exact=2, sample_seed=42)
        indices = np.sort(np.random.default_rng(42).choice(3, size=2, replace=False))
        expected = [list(self.documents.values())[index] for index in indices]
        self._assert_rows_and_histogram(captured, expected)

    def test_duplicate_ids_with_unchanged_counts_are_rejected(self):
        for band, strict in itertools.product((0, 1), (False, True)):
            with self.subTest(band=band, strict=strict):
                self._write_signatures()
                self._write_band(band, 0, [(7, (1, 1)), (7, (1, 2)), ((1 << 32) - 1, (9, 9))])
                with self.assertRaisesRegex(ValueError, "Duplicate doc_id=7"):
                    self._analyze(strict=strict)

    def test_unknown_id_with_unchanged_counts_is_rejected(self):
        self._write_signatures()
        self._write_band(1, 0, [(7, (9, 9)), (99, (9, 9)), ((1 << 32) - 1, (1, 1))])
        for strict in (False, True):
            with self.subTest(strict=strict), self.assertRaisesRegex(ValueError, "Unexpected doc_id=99"):
                self._analyze(strict=strict)

    def test_missing_retained_id_is_rejected(self):
        self._write_signatures()
        self._write_band(1, 0, [(7, (9, 9)), ((1 << 32) - 1, (1, 1))])
        for strict in (False, True):
            with self.subTest(strict=strict), self.assertRaisesRegex(ValueError, "Missing doc_id=1000000"):
                self._analyze(strict=strict)

    def test_missing_reader_file_is_rejected(self):
        self._write_signatures()
        (self.root / "bucket_001" / "00000.minhash.sig").unlink()
        for strict in (False, True):
            with self.subTest(strict=strict), self.assertRaises(FileNotFoundError):
                self._analyze(strict=strict)

    def test_duplicate_reader_file_is_rejected(self):
        for band in (0, 1):
            with self.subTest(band=band):
                self._write_signatures()
                self._write_band(band, 0, [(7, (1, 1))], filename="0.minhash.sig")
                with self.assertRaisesRegex(ValueError, "Duplicate signature file"):
                    self._analyze()
                (self.root / f"bucket_{band:03d}" / "0.minhash.sig").unlink()

    def test_all_documents_removed_skips_histogram(self):
        self._write_signatures()
        with patch.object(minhash, "_hist_all_blocks_numba") as kernel:
            self._analyze(self.documents)
        kernel.assert_not_called()

    def test_empty_signature_files_skip_histogram(self):
        for band in (0, 1):
            self._write_band(band, 0, [])
        with patch.object(minhash, "_hist_all_blocks_numba") as kernel:
            self._analyze()
        kernel.assert_not_called()

    def test_single_retained_document_has_no_pairs(self):
        self._write_signatures()
        captured = self._capture_analysis([(0, 1_000_000), (0, (1 << 32) - 1)])
        self._assert_rows_and_histogram(captured, [self.documents[(0, 7)]])

    def test_strict_validates_unmatched_removals(self):
        self._write_signatures()
        for missing in ((0, 99), (5, 7)):
            with self.subTest(missing=missing):
                with self.assertRaisesRegex(ValueError, "Unmatched removals"):
                    self._analyze([missing])
                captured = self._capture_analysis([missing], strict=False)
                self._assert_rows_and_histogram(captured, list(self.documents.values()))


if __name__ == "__main__":
    unittest.main()
