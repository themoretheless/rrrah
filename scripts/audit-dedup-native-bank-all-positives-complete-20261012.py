#!/usr/bin/env python3
"""Qualify terminal historical458 execution separately from current recall."""
import hashlib
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research'
TMP = Path('/Users/themoretheless/.codex/tmp')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    report = DOC / 'dedup-native-bank-all-positives-20261011.json'
    prefix = DOC / 'dedup-native-bank-all-positives-prefix-0458-audit-20261011.json'
    data = json.loads(report.read_text())
    audit = json.loads(prefix.read_text())
    assert data['status'] == 'complete_bounded_publisher_positive_execution'
    assert data['required'] == data['completed'] == len(data['results']) == len(data['plan']) == 458
    assert audit['completed'] == 458 and audit['counts'] == dict(supported=458, inconclusive=0, rejected=0, execution_failed=0)
    assert Path(audit['captured_report']).read_bytes() == report.read_bytes()
    assert all(sha(path) == digest for path, digest in audit['pins'].items())
    priority = [3, 7, 27, 57, 67, 153, 255]
    assert [row['baseline_index'] for row in data['plan']] == priority + [index for index in range(458) if index not in priority]
    unordered = Counter(tuple(sorted((row['left'], row['right']))) for row in data['plan'])
    directed = {(row['left'], row['right']) for row in data['plan']}
    assert len(unordered) == 229 and set(unordered.values()) == {2}
    assert len(directed) == 458 and all((right, left) in directed for left, right in directed)
    binary = TMP / 'rrrah-native-bank-all-positives-qualified-20261011/native_bank_outcome_files'
    weights = TMP / 'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors'
    for index, (row, result) in enumerate(zip(data['plan'], data['results'])):
        expected = ['/usr/bin/time', '-l', '/usr/bin/env',
            'DYLD_LIBRARY_PATH=' + str(TMP / 'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),
            str(binary), row['left'], row['right'], str(weights), '640',
            str(row['source_tolerance']), '1000000']
        assert result['index'] == index and result['command'] == expected
        assert result['returncode'] == 0 and result['status'] == 'publisher_positive_supported'
        value = json.loads(result['stdout'])
        assert value['opposite_trials'] == 1000000 and value['managed_used'] == 0
        assert value['pairs'][0]['support'] is True
    pins = dict(audit['pins'])
    for path in [report, prefix, Path(__file__)]:
        pins[str(path)] = sha(path)
    output = DOC / 'dedup-native-bank-all-positives-complete-audit-20261012.json'
    assert not output.exists()
    output.write_text(json.dumps(dict(status='verified_complete_historical_fixed458_positive_directions',
        completed=458, unordered_positive_pairs=229, both_directions=True,
        counts=audit['counts'], model_attempted_directions=sum(row['evidence']['model_attempted'] for row in data['results']),
        process_peak_rss_bytes_max=max(row['rss_bytes'] for row in data['results']),
        frozen_source_manifest='dedup-native-bank-outcome-native-snapshot-20261011.json', pins=pins,
        scope='All fixed publisher229positive pairs in both directions supported on historical frozen executable. Original fixed policy/tolerances/order, all complete logs and pins verified. Does not qualify current optimized composition, semantic negative precision, all formats/platforms, full500 source review or hard RSS/latency bounds.'), indent=2) + '\n')
    print('verified historical458/458 supported; current composition remains separate')


if __name__ == '__main__':
    main()
