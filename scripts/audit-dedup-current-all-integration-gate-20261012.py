#!/usr/bin/env python3
"""Verify every Cargo test target, retaining immutable executed binaries."""
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
    report = DOC / 'dedup-current-all-integration-gate-20261012.json'
    data = json.loads(report.read_text())
    assert data['status'] == 'complete_pending_audit' and data['returncode'] == 0
    assert data['source_unchanged']
    assert all(sha(path) == digest for path, digest in data['pins'].items())
    source = Path(data['source'])
    frozen = json.loads(Path(data['source_manifest']).read_text())
    assert all(sha(source / name) == digest for name, digest in frozen['source_hashes'].items())
    metadata_command = ['cargo', 'metadata', '--locked', '--offline', '--no-deps', '--format-version', '1']
    metadata = json.loads(subprocess.check_output(metadata_command, cwd=source))
    package = next(item for item in metadata['packages'] if item['name'] == 'rrrah-dedup')
    enabled = {'decode', 'raster'}
    expected = {str(Path(target['src_path']).relative_to(source / 'crates/rrrah-dedup'))
        for target in package['targets'] if target.get('test')
        and set(target['kind']) & {'lib', 'test', 'bin'}
        and set(target.get('required-features', [])) <= enabled}
    log = Path(data['log']).read_text()
    markers = list(re.finditer(r'Running (?:unittests )?(src/[^\n ]+\.rs|tests/[^\n ]+\.rs) \(([^)]+)\)', log))
    assert len(markers) == len(expected) and {marker[1] for marker in markers} == expected
    destination = TMP / 'rrrah-dedup-current-all-integration-qualified-executables-20261012'
    output = DOC / 'dedup-current-all-integration-gate-complete-audit-20261012.json'
    assert not destination.exists() and not output.exists()
    rows = []
    for index, marker in enumerate(markers):
        section = log[marker.end():markers[index + 1].start() if index + 1 < len(markers) else len(log)]
        summaries = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', section)
        assert summaries, marker[1]
        # Cache crash/concurrency controls can print nested helper summaries.
        passed, failed, ignored, measured, filtered = map(int, summaries[-1])
        assert failed == measured == filtered == 0, marker[1]
        listed = subprocess.run([marker[2], '--list'], check=True, capture_output=True, text=True)
        names = re.findall(r'^(.+): test$', listed.stdout, re.M)
        assert len(names) == len(set(names)) == passed + ignored, marker[1]
        successful = set(re.findall(r'^test (.+) \.\.\. ok$', section, re.M))
        skipped = re.findall(r'^test (.+) \.\.\. ignored(?:, (.*))?$', section, re.M)
        assert successful | {name for name, _ in skipped} == set(names), marker[1]
        assert len(successful) == passed and len(skipped) == ignored, marker[1]
        rows.append(dict(target=marker[1], passed=passed, ignored=ignored,
            names=names, skipped=skipped, original_executable=marker[2], executable_sha256=sha(marker[2])))
    destination.mkdir()
    pins = {str(path): sha(path) for path in [report, Path(data['source_manifest']), Path(data['log']), Path(__file__)]}
    for row in rows:
        saved = destination / Path(row['original_executable']).name
        shutil.copy2(row['original_executable'], saved)
        assert sha(saved) == row['executable_sha256']
        row['saved_executable'] = str(saved)
        pins[str(saved)] = sha(saved)
    drift = [name for name, digest in frozen['source_hashes'].items()
        if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
    output.write_text(json.dumps(dict(status='verified_complete_current_decode_all_test_targets',
        targets=rows, targets_verified=len(rows), passed=sum(row['passed'] for row in rows),
        ignored=sum(row['ignored'] for row in rows), frozen_files_verified=len(frozen['source_hashes']),
        root_drift_at_audit=drift, pins=pins, scope=data['scope']), indent=2) + '\n')
    print(len(rows), sum(row['passed'] for row in rows), sum(row['ignored'] for row in rows))


if __name__ == '__main__':
    main()
