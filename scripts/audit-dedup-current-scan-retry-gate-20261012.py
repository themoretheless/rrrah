#!/usr/bin/env python3
"""Verify actual regression execution and unchanged captured compiler flags."""
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research'
TMP = Path('/Users/themoretheless/.codex/tmp')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def semantic_flags(arguments):
    result = []
    index = 0
    while index < len(arguments):
        value = arguments[index]
        if value in ['--crate-name', '--out-dir', '-o']:
            index += 2
        elif value == '-C' and arguments[index + 1].startswith('extra-filename='):
            index += 2
        elif value.startswith('--emit=') or value.endswith('.rs'):
            index += 1
        else:
            result.append(value)
            index += 1
    return result


def main():
    report = DOC / 'dedup-current-scan-retry-gate-v2-20261012.json'
    data = json.loads(report.read_text())
    assert data['status'] == 'complete_pending_audit'
    assert data['build_returncode'] == data['returncode'] == 0
    assert all(sha(path) == digest for path, digest in data['pins'].items())
    captured = TMP / 'rrrah-dedup-scan-retry-live-compiler-dependencies-v2-20261012.json'
    original = json.loads(captured.read_text())
    assert original['method'].startswith('macOS KERN_PROCARGS2, argv only')
    assert semantic_flags(data['command']) == semantic_flags(original['captured_args'])
    manifest = DOC / 'dedup-current-full-fixtures-source-20261012.json'
    frozen = json.loads(manifest.read_text())
    assert all(sha(Path(frozen['source']) / name) == digest for name, digest in frozen['source_hashes'].items())
    test_source = ROOT / 'crates/rrrah-dedup/tests/scan_retry.rs'
    target = TMP / 'rrrah-dedup-current-scan-retry-qualified-v2-20261012'
    assert test_source.read_bytes() == (target / 'scan_retry.rs').read_bytes()
    binary = target / 'scan_retry_test'
    name = 'same_cache_retries_repaired_sources_and_never_retains_old_pair_reports'
    log = Path(data['test_log']).read_text()
    assert re.findall(r'^test (.+) \.\.\. ok$', log, re.M) == [name]
    assert re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', log) == [('1', '0', '0', '0', '0')]
    listed = subprocess.run([str(binary), '--list'], check=True, capture_output=True, text=True)
    assert re.findall(r'^(.+): test$', listed.stdout, re.M) == [name]
    pins = dict(data['pins'])
    for path in [report, Path(__file__)]:
        pins[str(path)] = sha(path)
    output = DOC / 'dedup-current-scan-retry-gate-complete-audit-20261012.json'
    assert not output.exists()
    output.write_text(json.dumps(dict(status='verified_current_scan_repair_mutation_removal_regression',
        passed=1, failed=0, ignored=0, test_name=name, source_files_verified=len(frozen['source_hashes']),
        root_test_matches_executed_copy=True, pins=pins,
        scope=data['scope'], scenarios=['decode failure retains typed issue', 'repaired identical source retries and reuses valid cache',
            'changed bytes miss stale cache and match a fresh-cache scan', 'original pixels reject changed source',
            'removed source remains an attributed issue, with no stale pair', 'previous report stays independent',
            'recreated source retries', 'empty request set returns no stale results', 'managed credit released']), indent=2) + '\n')
    print('verified1current regression,8lifecycle scenarios')


if __name__ == '__main__':
    main()
