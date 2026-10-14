#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import unittest
spec=importlib.util.spec_from_file_location('qualification',Path(__file__).with_name('qualify-viewer-storage-gpu.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
MARKER='large float delayed swap: cycles=3; seconds=101.586;2048x2048;64MiB; every signed/HDR/negative-zero bit exact; full pressure and admitted cancellation after64KiB read each cycle; disk count2/bytes bounded; final=0'
class EvidenceTests(unittest.TestCase):
    def test_complete(self):self.assertTrue(module.large_float_storage_evidence(MARKER,100))
    def test_truncated(self):self.assertFalse(module.large_float_storage_evidence(MARKER[:-8],10))
    def test_duplicate(self):self.assertFalse(module.large_float_storage_evidence(MARKER+'\n'+MARKER,10))
    def test_insufficient_cycles(self):self.assertFalse(module.large_float_storage_evidence(MARKER.replace('cycles=3','cycles=2'),10))
    def test_insufficient_duration(self):self.assertFalse(module.large_float_storage_evidence(MARKER,102))
    def test_old_timer_evidence(self):self.assertFalse(module.large_float_storage_evidence(MARKER.replace('admitted cancellation after64KiB read','mid-read cancellation'),10))
    def test_wrong_extent(self):self.assertFalse(module.large_float_storage_evidence(MARKER.replace('2048x2048','256x256'),10))
    def test_malformed_seconds(self):
        for value in ('1.2.3', '.', 'NaN', 'inf', '-101.586'):
            with self.subTest(value=value):
                self.assertFalse(module.large_float_storage_evidence(MARKER.replace('101.586',value),10))
    def test_marker_must_be_complete_line(self):
        for text in ('unrelated '+MARKER, MARKER+' contradictory', MARKER.replace('final=0','final=01')):
            with self.subTest(text=text):
                self.assertFalse(module.large_float_storage_evidence(text,10))
if __name__=='__main__':unittest.main()
