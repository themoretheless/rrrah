#!/usr/bin/env python3
"""Replay the fixed16 negatives through the complete current native caller."""
import hashlib
import json
import os
import re
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research'
TMP = Path('/Users/themoretheless/.codex/tmp')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    audit_path = DOC / 'dedup-current-native-bank-prefix-0003-audit-20261012.json'
    current_audit = json.loads(audit_path.read_text())
    assert current_audit['completed'] == 3
    assert all(sha(path) == digest for path, digest in current_audit['pins'].items())
    capture = TMP / 'rrrah-current-native-bank-prefix-0003-20261012.json'
    current = json.loads(capture.read_text())
    binary = Path(current['executable'])
    original_path = DOC / 'dedup-disc21-heldout-negatives-runtime-20261012.json'
    original = json.loads(original_path.read_text())
    original_audit_path = DOC / 'dedup-disc21-heldout-negatives-complete-audit-20261012.json'
    original_audit = json.loads(original_audit_path.read_text())
    assert all(sha(path) == digest for path, digest in original_audit['pins'].items())
    assert len(original['plan']) == len(original['results']) == 16
    manifest = Path(current['source_manifest'])
    frozen = json.loads(manifest.read_text())
    assert all(sha(Path(frozen['source']) / name) == digest for name, digest in frozen['source_hashes'].items())
    report = DOC / 'dedup-current-heldout-negatives-20261012.json'
    folder = TMP / 'rrrah-dedup-current-heldout-negatives-20261012'
    assert not report.exists() and not folder.exists()
    plan = []
    pins = {str(path): sha(path) for path in [audit_path, capture, original_path,
        original_audit_path, manifest, Path(__file__), binary]}
    pins.update({path: digest for path, digest in current['pins'].items()
                 if path.endswith('.dylib') or path.endswith('.safetensors')})
    for index, (row, result) in enumerate(zip(original['plan'], original['results'])):
        assert row['query'] == result['query'] and row['reference'] == result['reference']
        assert row['published_label'] == 'no_reference_match'
        old_command = result['command']
        assert old_command[-2:] == ['640', '2.0']
        left, right, weights = old_command[-5:-2]
        for path in [left, right, weights]:
            pins[path] = sha(path)
        plan.append(dict(row, index=index, left=left, right=right, weights=weights))
    folder.mkdir()
    data = dict(status='running', producer_pid=os.getpid(), required=16, completed=0,
        plan=plan, results=[], pins=pins,
        counts=dict(false_acceptance=0, inconclusive=0, rejected=0, execution_failed=0),
        scope='Fixed16 published DISC21 hash-near distractors, current exact caller/policy. Native4096/opposite1m plus model25banks/radii2and6;640input/tolerance2.0, no tuning. Previous negative caller differs; no full-JSON parity claim. Does not establish semantic burst precision, universal precision, full458 recall, hard RSS or all500 review completion.')

    def save():
        temporary = report.with_suffix('.tmp')
        temporary.write_text(json.dumps(data, indent=2) + '\n')
        temporary.replace(report)

    save()
    try:
        for row in plan:
            command = ['/usr/bin/time', '-l', '/usr/bin/env',
                'DYLD_LIBRARY_PATH=' + str(TMP / 'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),
                str(binary), row['left'], row['right'], row['weights'], '640', '2.0', '1000000']
            started = time.monotonic()
            child = subprocess.Popen(command, cwd='/tmp', stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True)
            data.update(active_index=row['index'], child_pid=child.pid)
            save()
            stdout, stderr = child.communicate()
            case = dict(index=row['index'], command=command, returncode=child.returncode,
                        stdout=stdout, stderr=stderr, elapsed_seconds=time.monotonic() - started)
            kind = 'execution_failed'
            if child.returncode == 0:
                evidence = json.loads(stdout)
                assert evidence['files'] == [2, 9] and evidence['managed_used'] == 0
                assert len(evidence['pairs']) == 1
                pair = evidence['pairs'][0]
                assert (pair['left'], pair['right']) == (2, 9)
                support = pair['support']
                assert support is None or type(support) is bool
                kind = 'false_acceptance' if support is True else ('rejected' if support is False else 'inconclusive')
                case['rss_bytes'] = int(re.search(r'(\d+)\s+maximum resident set size', stderr)[1])
            case['status'] = kind
            path = folder / f'{row["index"]:02d}.json'
            assert not path.exists()
            path.write_text(json.dumps(case, indent=2) + '\n')
            assert all(sha(p) == pins[p] for p in [row['left'], row['right'], row['weights'], str(binary)])
            data['results'].append(dict(index=row['index'], status=kind, report=str(path), sha256=sha(path)))
            data['counts'][kind] += 1
            data['completed'] += 1
            save()
            print(row['index'], kind, flush=True)
        assert all(sha(path) == digest for path, digest in pins.items())
        assert all(sha(Path(frozen['source']) / name) == digest for name, digest in frozen['source_hashes'].items())
        data['status'] = 'complete_current_fixed16_execution'
        save()
    except Exception as error:
        data.update(status='terminal_controller_failure', error=str(error))
        save()
        raise


if __name__ == '__main__':
    main()
