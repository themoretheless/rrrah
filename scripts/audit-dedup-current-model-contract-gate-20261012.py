#!/usr/bin/env python3
"""Audit current generic model contracts without claiming Torch coverage."""
import hashlib
import json
import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research'
TMP = Path('/Users/themoretheless/.codex/tmp')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    report = DOC / 'dedup-current-model-contract-gate-20261012.json'
    data = json.loads(report.read_text())
    assert data['status'] == 'complete' and data['returncode'] == 0
    manifest = DOC / data['source_manifest']
    source_data = json.loads(manifest.read_text())
    source = Path(data['source'])
    assert str(source) == source_data['source']
    assert all(sha(source / name) == digest
               for name, digest in source_data['source_hashes'].items())
    assert all(sha(path) == digest for path, digest in data['test_source_sha256'].items())
    assert sha(data['log']) == data['log_sha256']
    assert data['command'] == ['cargo', 'test', '-p', 'rrrah-dedup', '--features',
        'decode', '--locked', '--offline', '--test', 'model_compare', '--test',
        'opposite_native', '--target-dir', str(TMP / 'rrrah-dedup-exact-portfolio-20261011')]
    log = Path(data['log']).read_text()
    markers = list(re.finditer(r'Running tests/(model_compare|opposite_native)\.rs \(([^)]+)\)', log))
    assert len(markers) == 2 and {m[1] for m in markers} == {'model_compare', 'opposite_native'}
    destination = TMP / 'rrrah-dedup-current-model-contract-qualified-executables-20261012'
    output = DOC / 'dedup-current-model-contract-gate-complete-audit-20261012.json'
    assert not output.exists() and not destination.exists()
    rows = []
    for index, marker in enumerate(markers):
        section = log[marker.end():markers[index + 1].start() if index + 1 < len(markers) else len(log)]
        summaries = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', section)
        assert len(summaries) == 1
        passed, failed, ignored, measured, filtered = map(int, summaries[0])
        assert passed > 0 and failed == ignored == measured == filtered == 0
        names = re.findall(r'^test (.+) \.\.\. ok$', section, re.M)
        assert len(names) == len(set(names)) == passed
        executable = Path(marker[2])
        listed = subprocess.run([str(executable), '--list'], check=True, capture_output=True, text=True)
        listed_names = re.findall(r'^(.+): test$', listed.stdout, re.M)
        assert set(listed_names) == set(names)
        rows.append(dict(target=marker[1], passed=passed, names=names,
                         original_executable=str(executable), executable_sha256=sha(executable)))
    destination.mkdir()
    pins = {str(path): sha(path) for path in [report, manifest, Path(data['log']), Path(__file__)]}
    for row in rows:
        saved = destination / Path(row['original_executable']).name
        shutil.copy2(row['original_executable'], saved)
        assert sha(saved) == row['executable_sha256']
        row['saved_executable'] = str(saved)
        pins[str(saved)] = sha(saved)
    drift = [name for name, digest in source_data['source_hashes'].items()
             if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
    result = dict(status='verified_complete_current_generic_model_contract_gate',
        passed=sum(row['passed'] for row in rows), failed=0, ignored=0, targets=rows,
        frozen_files_verified=len(source_data['source_hashes']), root_drift_now=drift,
        pins=pins, scope='Generic model provider and opposite native contracts on frozen current decode source; no actual Torch inference, full corpus, platform or RSS bound qualification.')
    output.write_text(json.dumps(result, indent=2) + '\n')
    print([(row['target'], row['passed']) for row in rows])


if __name__ == '__main__':
    main()
