"""Tiny no-codec/no-network preflight control; safe during coordinated GPU window."""
import pathlib
import runpy
import shutil
import sys
import tempfile
from unittest.mock import patch

project = pathlib.Path(__file__).resolve().parents[3]
retained = pathlib.Path('/Users/themoretheless/.codex/tmp/rrrah-w3c-png-conformance')
script = project / 'scripts/fetch-w3c-hdr-png-conformance.py'
first = 'PNG-HLG-FancyColorBars-16bit-CICP-FR.png'
second = 'PNG-HLG-FancyColorBars-16bit-CICP-NR.png'
with tempfile.TemporaryDirectory() as directory:
    root = pathlib.Path(directory)
    shutil.copyfile(retained / first, root / first)
    (root / second).write_bytes(b'invalid-source')
    oracle = root / (first + '.rgb48le')
    oracle.write_bytes(b'retained-oracle')
    manifest = root / 'oracle-manifest.json'
    manifest.write_bytes(b'retained-manifest')
    with patch.object(sys, 'argv', [str(script), str(root)]), patch('subprocess.run', side_effect=AssertionError('codec started before all sources were validated')), patch('urllib.request.urlopen', side_effect=AssertionError('network not allowed in this control')):
        try:
            runpy.run_path(str(script), run_name='__main__')
        except ValueError as error:
            assert 'hash mismatch' in str(error)
        else:
            raise AssertionError('invalid later source was not refused')
    assert oracle.read_bytes() == b'retained-oracle'
    assert manifest.read_bytes() == b'retained-manifest'
    assert (root / second).read_bytes() == b'invalid-source'
print('PASS: all source admission precedes codecs; invalid later source preserves oracles and manifest; no network/codec executed')
