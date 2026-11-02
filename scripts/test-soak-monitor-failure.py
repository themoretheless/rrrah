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
        totals = (b'large soak injected swap codec totals: read_calls=1025 read_bytes=16777280 '
                  b'max_read_request=16384 write_calls=1025 write_bytes=16777280 '
                  b'max_write_request=16384 completed_spills=1 completed_restores=1\n')
        for prefix, expected, stats in [(b'', False, totals), (marker, True, totals),
                                        (marker+marker, False, totals),
                                        (marker.replace(b'delay_ms=1', b'delay_ms=2'), False, totals),
                                        (marker, False, b'')]:
            with self.subTest(prefix=prefix), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                binary = root / 'synthetic-child'
                binary.write_text('#!/bin/sh\nexit 0\n')
                binary.chmod(0o700)
                def spawn(*args, **kwargs):
                    self.assertEqual(kwargs['env']['RRRAH_QUALIFY_SWAP_DELAY_MS'], '1')
                    self.assertEqual(kwargs['env']['RRRAH_QUALIFY_SWAP_CHUNK_BYTES'], '16384')
                    kwargs['stdout'].write(prefix + stats + original)
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
                self.assertEqual(state['io_injection_confirmed'], prefix == marker)
                self.assertEqual(state['io_directions_confirmed'], bool(stats))
                self.assertEqual(state['workload_evidence_valid'], expected)
                self.assertEqual(state['io_injection_scope'], 'swap_payload_codec')

    def test_injected_io_totals_reject_missing_duplicate_and_inconsistent_directions(self):
        line = ('large soak injected swap codec totals: read_calls=1025 read_bytes=16777280 '
                'max_read_request=16384 write_calls=1025 write_bytes=16777280 '
                'max_write_request=16384 completed_spills=1 completed_restores=1\n')
        self.assertIsNotNone(monitor.injected_io_evidence(line, 16384))
        for changed in ['', line + line,
                        line.replace('completed_restores=1', 'completed_restores=0'),
                        line.replace('completed_spills=1', 'completed_spills=0'),
                        line.replace('read_bytes=16777280', 'read_bytes=64'),
                        line.replace('write_bytes=16777280', 'write_bytes=64'),
                        line.replace('max_read_request=16384', 'max_read_request=16385'),
                        line.replace('max_write_request=16384', 'max_write_request=16385'),
                        line.replace('read_calls=1025', 'read_calls=1'),
                        line.replace('completed_restores=1', 'completed_restores=2')]:
            self.assertIsNone(monitor.injected_io_evidence(changed, 16384))


if __name__ == '__main__':
    unittest.main()
