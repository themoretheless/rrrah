"""Evidence controls for distinct-key swap qualification, not runtime evidence."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'qualification', Path(__file__).with_name('qualify-viewer-storage-gpu.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
TEARDOWN = '100k swap asynchronous teardown: 2.123ms; physical files/bytes and root/queue=0\n'
RESULT = '100k swap: seconds=26.045 writes=100000 reads=100000 pressure_refusals=1000 root_peak=131072 queue_peak=4 final=0\n'


class ChurnEvidence(unittest.TestCase):
    def test_complete(self):
        self.assertTrue(module.hundred_thousand_storage_evidence(TEARDOWN + RESULT))

    def test_exact_serial_test_heading(self):
        prefix = 'test hundred_thousand_distinct_rasters_keep_swap_index_and_memory_bounded ... '
        self.assertTrue(module.hundred_thousand_storage_evidence(prefix + TEARDOWN + RESULT))
        self.assertFalse(module.hundred_thousand_storage_evidence('test unrelated ... ' + TEARDOWN + RESULT))

    def test_missing_or_duplicate_markers(self):
        for text in (RESULT, TEARDOWN, TEARDOWN * 2 + RESULT, TEARDOWN + RESULT * 2):
            with self.subTest(text=text):
                self.assertFalse(module.hundred_thousand_storage_evidence(text))

    def test_incomplete_work_and_memory_overruns(self):
        for old, new in (('writes=100000', 'writes=99999'), ('reads=100000', 'reads=99999'),
                         ('pressure_refusals=1000', 'pressure_refusals=999'),
                         ('root_peak=131072', 'root_peak=131073'),
                         ('root_peak=131072', 'root_peak=0'),
                         ('queue_peak=4', 'queue_peak=4097'), ('queue_peak=4', 'queue_peak=0'),
                         ('final=0', 'final=1'), ('seconds=26.045', 'seconds=0'),
                         ('seconds=26.045', 'seconds=1.2.3'),
                         ('root/queue=0', 'root/queue=1')):
            with self.subTest(old=old, new=new):
                self.assertFalse(module.hundred_thousand_storage_evidence(
                    (TEARDOWN + RESULT).replace(old, new)))

    def test_complete_lines_required(self):
        for text in (TEARDOWN + 'embedded ' + RESULT,
                     TEARDOWN + RESULT.rstrip() + ' extra\n'):
            self.assertFalse(module.hundred_thousand_storage_evidence(text))


if __name__ == '__main__':
    unittest.main()
