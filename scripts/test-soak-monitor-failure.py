"""Monitor failure controls; synthetic child output is not hardware evidence."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    'soak_monitor', Path(__file__).with_name('run-metal-collection-soak.py'))
monitor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(monitor)


class ExitedChild:
    pid = 123456789
    returncode = 0

    def __init__(self, *args, **kwargs):
        kwargs['stdout'].write(b'test result: ok. 1 passed; 0 failed; 0 ignored;\n')
        kwargs['stdout'].flush()

    def poll(self):
        return self.returncode

    def wait(self, **kwargs):
        return self.returncode


class MonitorFailure(unittest.TestCase):
    def test_validator_exception_saves_terminal_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / 'synthetic-child'
            binary.write_text('#!/bin/sh\nexit 0\n')
            binary.chmod(0o700)
            argv = ['monitor', '--suite', 'float-delayed', '--binary', str(binary),
                    '--output', str(root / 'out'), '--scratch', str(root / 'scratch'),
                    '--seconds', '10']
            with patch('sys.argv', argv), patch.object(monitor.subprocess, 'Popen', ExitedChild), \
                 patch.object(monitor.runpy, 'run_path', side_effect=ValueError('broken evidence')):
                self.assertEqual(monitor.main(), 1)
            state = json.loads((root / 'out/state.json').read_text())
            self.assertEqual(state['status'], 'failed')
            self.assertEqual(state['exit_code'], 0)
            self.assertFalse(state['workload_evidence_valid'])
            self.assertEqual(state['validation_error'], 'ValueError: broken evidence')
            self.assertIn('ended_utc', state)

    def test_worker_injection_requires_one_exact_confirmation(self):
        # Replayed old successful output cannot qualify requested new instrumentation.
        original = (monitor.ROOT / 'docs/research/metal-prefetch-boundaries-2026-11-02/window-soak.log').read_bytes()
        marker = b'large soak injected swap codec I/O: delay_ms=1 chunk_bytes=16384\n'
        for prefix, expected in [(b'', False), (marker, True), (marker+marker, False),
                                 (marker.replace(b'delay_ms=1', b'delay_ms=2'), False)]:
            with self.subTest(prefix=prefix), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                binary = root / 'synthetic-child'
                binary.write_text('#!/bin/sh\nexit 0\n')
                binary.chmod(0o700)
                def spawn(*args, **kwargs):
                    self.assertEqual(kwargs['env']['RRRAH_QUALIFY_SWAP_DELAY_MS'], '1')
                    self.assertEqual(kwargs['env']['RRRAH_QUALIFY_SWAP_CHUNK_BYTES'], '16384')
                    kwargs['stdout'].write(prefix + original)
                    kwargs['stdout'].flush()
                    child = object.__new__(ExitedChild)
                    return child
                argv = ['monitor', '--suite', 'worker-mixed', '--binary', str(binary),
                        '--output', str(root / 'out'), '--scratch', str(root / 'scratch'),
                        '--seconds', '60', '--swap-delay-ms', '1', '--swap-chunk-bytes', '16384']
                with patch('sys.argv', argv), patch.object(monitor.subprocess, 'Popen', spawn), \
                     patch.object(monitor.runpy, 'run_path', return_value={}):
                    self.assertEqual(monitor.main(), 0 if expected else 1)
                state = json.loads((root / 'out/state.json').read_text())
                self.assertEqual(state['io_injection_confirmed'], expected)
                self.assertEqual(state['workload_evidence_valid'], expected)
                self.assertEqual(state['io_injection_scope'], 'swap_payload_codec')


if __name__ == '__main__':
    unittest.main()
