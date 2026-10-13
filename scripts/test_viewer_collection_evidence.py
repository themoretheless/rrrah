"""Reject partial, stale and contradictory collection qualification logs."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'viewer_qualification', Path(__file__).with_name('qualify-viewer-storage-gpu.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def log(output='4000, 3000'):
    return (
        f'long-lived collection adapter: Metal test adapter; duration=120s; frames=8; payload=192000000; hdr_output=[{output}]\n'
        'long-lived collection complete: cycles=150; elapsed=120.803s; reads=150; writes=8; root=0; queue=0\n'
        f'parallel collection: accelerated=150; mid_batch_cancelled=150; exact_sdr_full_frames=150; exact_hdr_half_frames=150; hdr_output=[{output}]; root=0; queue=0\n')


class CollectionEvidence(unittest.TestCase):
    def valid(self, text, full=True, passed=1, ignored=0):
        return module.collection_evidence(text, 120, full, passed, ignored)

    def test_full_and_small_are_distinct(self):
        self.assertTrue(self.valid(log()))
        self.assertTrue(self.valid(log('63, 65'), full=False))
        self.assertFalse(self.valid(log('63, 65')))
        self.assertFalse(self.valid(log(), full=False))

    def test_serial_host_without_parallel_scratch(self):
        self.assertTrue(self.valid(log().replace('accelerated=150', 'accelerated=0')
                                   .replace('mid_batch_cancelled=150', 'mid_batch_cancelled=0')))

    def test_incomplete_or_contradictory_workload(self):
        for old, new in [
            ('120.803s', '119.999s'), ('duration=120s', 'duration=60s'),
            ('reads=150', 'reads=149'), ('writes=8', 'writes=9'),
            ('exact_sdr_full_frames=150', 'exact_sdr_full_frames=149'),
            ('exact_hdr_half_frames=150', 'exact_hdr_half_frames=149'),
            ('mid_batch_cancelled=150', 'mid_batch_cancelled=149'),
            ('root=0', 'root=192000000'), ('queue=0', 'queue=192000000'),
            ('Metal test adapter', 'Vulkan test adapter'),
            ('payload=192000000', 'payload=16'),
        ]:
            with self.subTest(old=old):
                self.assertFalse(self.valid(log().replace(old, new)))
        self.assertFalse(self.valid(log().replace('150', '7')))

    def test_missing_duplicate_or_skipped(self):
        for line in log().splitlines(keepends=True):
            self.assertFalse(self.valid(log().replace(line, '')))
            self.assertFalse(self.valid(log() + line))
        self.assertFalse(self.valid(log(), passed=0))
        self.assertFalse(self.valid(log(), ignored=1))


if __name__ == '__main__':
    unittest.main()
