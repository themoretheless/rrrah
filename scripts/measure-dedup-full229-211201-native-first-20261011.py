#!/usr/bin/env python3
"""Alternative-order diagnostic for one frozen full229 failure, not a baseline rerun."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
BASE = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT = D / 'dedup-full229-211201-native-first-20261011.json'
BINARY = BASE / 'tile_collection_files-gradient-batch-production-qualified-20261011'
WEIGHTS = BASE / 'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
TORCH = '/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    assert not OUT.exists()
    failure = json.loads((D / 'dedup-full229-211201-failure-20261011.json').read_text())
    row = failure['row']
    manifest = json.loads((D / failure['snapshot'].split('/')[-1]).read_text())
    def verify_snapshot():
        assert all(sha(Path(manifest['snapshot']) / p) == h for p, h in manifest['file_hashes'].items())
    pins = {str(p): sha(p) for p in [BINARY, WEIGHTS, row['source'], row['target'], Path(__file__), D / 'dedup-full229-211201-failure-20261011.json', D / 'dedup-gradient-batch-production-snapshot-20261011.json', D / 'dedup-loftr-native-20261010-runtime-pins.json']}
    runtime = json.loads((D / 'dedup-loftr-native-20261010-runtime-pins.json').read_text())
    pins.update({p: h for p, h in runtime['pins'].items() if TORCH in p})
    command = ['/usr/bin/time', '-l', '/usr/bin/env', f'DYLD_LIBRARY_PATH={TORCH}', str(BINARY), str(WEIGHTS), '640', str(row['source_tolerance']), 'native-first', row['source'], row['target']]
    report = {'status': 'running', 'scope': __doc__, 'producer_pid': os.getpid(), 'pins': pins, 'command': command}
    def save():
        temp = OUT.with_suffix('.json.tmp')
        temp.write_text(json.dumps(report, indent=2) + '\n'); temp.replace(OUT)
    save()
    try:
        verify_snapshot()
        assert all(sha(p) == h for p, h in pins.items())
        start = time.monotonic()
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        report['process_pid'] = process.pid; save()
        stdout, stderr = process.communicate()
        report.update(returncode=process.returncode, stdout=stdout, stderr=stderr, elapsed_seconds=time.monotonic() - start)
        if process.returncode == 0:
            report['evidence'] = json.loads(stdout)
        verify_snapshot()
        assert all(sha(p) == h for p, h in pins.items())
        report['status'] = 'complete_bounded_native_first_alternative_execution'
        print(report['status'], report['returncode'], flush=True)
    except Exception as error:
        report.update(status='failed_native_first_alternative_diagnostic', error=str(error)); raise
    finally:
        save()

if __name__ == '__main__':
    main()
