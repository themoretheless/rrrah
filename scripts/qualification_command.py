"""Capture one bounded qualification command using the shared process-tree watchdog."""
import importlib.util
from pathlib import Path
import tempfile

_spec = importlib.util.spec_from_file_location(
    '_rrrah_viewer_watchdog', Path(__file__).with_name('qualify-viewer-storage-gpu.py'))
_gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_gate)


def run_logged(command, logs, timeout_seconds, env=None):
    command = [str(value) for value in command]
    # A file avoids pipe deadlock and keeps arbitrarily verbose child output out
    # of memory until the bounded child has stopped.
    with tempfile.TemporaryFile(mode='w+', encoding='utf-8') as output:
        code, timed_out = _gate.run_suite(command, output, env, timeout_seconds)
        output.seek(0)
        text = output.read()
    logs.append({'command': command, 'exit_code': code, 'timed_out': timed_out,
                 'timeout_seconds': timeout_seconds, 'output': text})
    if timed_out:
        raise TimeoutError(f'Command timed out after {timeout_seconds}s: {command!r}\n{text[-6000:]}')
    if code:
        raise RuntimeError(text[-6000:])
    return text
