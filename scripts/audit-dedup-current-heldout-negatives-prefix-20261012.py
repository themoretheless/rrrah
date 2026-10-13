#!/usr/bin/env python3
"""Audit completed immutable negative records; unknown is never a rejection."""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research'
TMP = Path('/Users/themoretheless/.codex/tmp')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    raw = (DOC / 'dedup-current-heldout-negatives-20261012.json').read_bytes()
    data = json.loads(raw)
    n = data['completed']
    assert 0 < n <= 16 and len(data['results']) == n
    assert all(sha(path) == digest for path, digest in data['pins'].items())
    original = json.loads((DOC / 'dedup-disc21-heldout-negatives-runtime-20261012.json').read_text())
    current = json.loads((TMP / 'rrrah-current-native-bank-prefix-0003-20261012.json').read_text())
    frozen = json.loads(Path(current['source_manifest']).read_text())
    assert all(sha(Path(frozen['source']) / name) == digest for name, digest in frozen['source_hashes'].items())
    assert len(data['plan']) == 16
    for index, (item, old) in enumerate(zip(data['plan'], original['plan'])):
        assert item['index'] == index
        assert all(item[key] == value for key, value in old.items())
        assert item['published_label'] == 'no_reference_match'
        assert [item['left'], item['right'], item['weights']] == original['results'][index]['command'][-5:-2]
    counts = dict(false_acceptance=0, inconclusive=0, rejected=0, execution_failed=0)
    pins = dict(data['pins'])
    rows = []
    for index, row in enumerate(data['results']):
        item = data['plan'][index]
        path = Path(row['report'])
        assert sha(path) == row['sha256'] and row['index'] == index
        case = json.loads(path.read_text())
        assert case['index'] == index
        expected = ['/usr/bin/time', '-l', '/usr/bin/env',
            'DYLD_LIBRARY_PATH=' + str(TMP / 'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),
            current['executable'], item['left'], item['right'], item['weights'], '640', '2.0', '1000000']
        assert case['command'] == expected
        kind = 'execution_failed'
        if case['returncode'] == 0:
            value = json.loads(case['stdout'])
            assert value['files'] == [2, 9] and value['managed_used'] == 0
            assert value['opposite_trials'] == 1000000 and len(value['pairs']) == 1
            pair = value['pairs'][0]
            assert (pair['left'], pair['right']) == (2, 9)
            support = pair['support']
            assert support is None or type(support) is bool
            assert support is not None or (pair['model_attempted'] is True and pair['detail']['terminal'] == 'inconclusive')
            kind = 'false_acceptance' if support is True else ('rejected' if support is False else 'inconclusive')
            assert case['rss_bytes'] == int(re.search(r'(\d+)\s+maximum resident set size', case['stderr'])[1])
        assert case['status'] == row['status'] == kind
        counts[kind] += 1
        rows.append(dict(index=index, query=item['query'], reference=item['reference'], status=kind))
        pins[str(path)] = row['sha256']
    assert counts == data['counts']
    capture = TMP / f'rrrah-dedup-current-heldout-negatives-prefix-{n:02d}-20261012.json'
    output = DOC / f'dedup-current-heldout-negatives-prefix-{n:02d}-audit-20261012.json'
    assert not capture.exists() and not output.exists()
    capture.write_bytes(raw)
    pins.update({str(path): sha(path) for path in [capture, Path(__file__)]})
    output.write_text(json.dumps(dict(status='verified_current_fixed_negative_completed_prefix',
        completed=n, required=16, producer_status_at_capture=data['status'], counts=counts,
        rows=rows, pins=pins, scope=data['scope']), indent=2) + '\n')
    print(n, counts)


if __name__ == '__main__':
    main()
