import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest


spec = importlib.util.spec_from_file_location('viewer_gate', Path(__file__).with_name('qualify-viewer-storage-gpu.py'))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class SuiteTimeoutTests(unittest.TestCase):
    def test_success_and_failure_keep_exit_code_and_stderr(self):
        with tempfile.TemporaryFile(mode='w+') as log:
            code, timed_out = gate.run_suite(
                [sys.executable, '-c', 'import sys; print("diagnostic",file=sys.stderr); sys.exit(7)'],
                log, os.environ.copy(), 10)
            self.assertEqual((code, timed_out), (7, False))
            log.seek(0)
            self.assertIn('diagnostic', log.read())

    @unittest.skipUnless(os.name == 'posix', 'POSIX process group test')
    def test_timeout_stops_child_tree_and_preserves_output(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryFile(mode='w+') as log:
            output = Path(directory) / 'alive'
            child = 'import time, pathlib; p=pathlib.Path(' + repr(str(output)) + '); p.write_text("child"); time.sleep(60); p.write_text("survived")'
            parent = 'import subprocess,sys,time; subprocess.Popen([sys.executable,"-c",' + repr(child) + ']); print("started",flush=True); time.sleep(60)'
            code, timed_out = gate.run_suite([sys.executable, '-c', parent], log, os.environ.copy(), 0.5)
            self.assertTrue(timed_out)
            self.assertNotEqual(code, 0)
            self.assertEqual(output.read_text(), 'child')
            log.seek(0)
            text = log.read()
            self.assertIn('started', text)
            self.assertIn('process tree termination requested', text)


if __name__ == '__main__':
    unittest.main()
