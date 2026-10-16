import importlib.util
import os
import signal
import subprocess
import time
from pathlib import Path
import sys
import tempfile
import unittest


spec = importlib.util.spec_from_file_location('viewer_gate', Path(__file__).with_name('qualify-viewer-storage-gpu.py'))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class SuiteTimeoutTests(unittest.TestCase):
    def test_failure_keeps_exit_code_and_stderr(self):
        with tempfile.TemporaryFile(mode='w+') as log:
            code, timed_out = gate.run_suite(
                [sys.executable, '-c', 'import sys; print("diagnostic",file=sys.stderr); sys.exit(7)'],
                log, os.environ.copy(), 10)
            self.assertEqual((code, timed_out), (7, False))
            log.seek(0)
            self.assertIn('diagnostic', log.read())

    def test_success_keeps_stdout_and_exit_code(self):
        with tempfile.TemporaryFile(mode='w+') as log:
            result = gate.run_suite(
                [sys.executable, '-c', 'print("completed",flush=True)'],
                log, os.environ.copy(), 10)
            self.assertEqual(result, (0, False))
            log.seek(0)
            self.assertEqual(log.read().strip(), 'completed')

    @unittest.skipUnless(os.name == 'posix', 'POSIX process group test')
    def test_timeout_stops_child_tree_and_preserves_output(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryFile(mode='w+') as log:
            output = Path(directory) / 'alive'
            pid_file = Path(directory) / 'child.pid'
            child = (
                'import os,signal,time,pathlib; '
                'signal.signal(signal.SIGTERM,signal.SIG_IGN); '
                f'pathlib.Path({str(pid_file)!r}).write_text(str(os.getpid())); '
                f'p=pathlib.Path({str(output)!r}); p.write_text("child"); '
                'time.sleep(60); p.write_text("survived")')
            parent = (
                'import subprocess,sys,time; '
                f'subprocess.Popen([sys.executable,"-c",{child!r}]); '
                'print("started",flush=True); time.sleep(60)')
            child_pid = None
            confirmed_dead = False
            try:
                started = time.monotonic()
                code, timed_out = gate.run_suite([sys.executable, '-c', parent], log, os.environ.copy(), 1.5)
                self.assertLess(time.monotonic() - started, 5)
                self.assertTrue(timed_out)
                self.assertNotEqual(code, 0)
                self.assertEqual(output.read_text(), 'child')
                child_pid = int(pid_file.read_text())
                # kill(pid, 0) alone can report a dead zombie until the OS reaps it.
                # Check execution state, including a child that ignores SIGTERM.
                deadline = time.monotonic() + 3
                while True:
                    probe = subprocess.run(['ps', '-p', str(child_pid), '-o', 'stat='],
                                           capture_output=True, text=True, timeout=1)
                    self.assertIn(probe.returncode, (0, 1), probe.stderr)
                    self.assertFalse(probe.stderr.strip(), probe.stderr)
                    state = probe.stdout.strip()
                    if not state or state.startswith('Z'):
                        confirmed_dead = True
                        break
                    self.assertLess(time.monotonic(), deadline, f'child still executing: {state}')
                    time.sleep(0.05)
            finally:
                if not confirmed_dead and child_pid is None and pid_file.exists():
                    child_pid = int(pid_file.read_text())
                if not confirmed_dead and child_pid is not None:
                    try:
                        os.kill(child_pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
            log.seek(0)
            text = log.read()
            self.assertIn('started', text)
            self.assertIn('process tree termination requested', text)


if __name__ == '__main__':
    unittest.main()
