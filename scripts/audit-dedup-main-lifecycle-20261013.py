#!/usr/bin/env python3
"""Audit the combined late-publication/RAW-cache gate on one pinned snapshot."""
import hashlib
import json
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TMP = Path('/Users/themoretheless/.codex/tmp')
DOC = ROOT / 'docs/research'


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    prior = DOC / 'dedup-raw-cache-audit-20261013.json'
    base = json.loads(prior.read_text())
    source = Path(base['source'])
    assert all(sha(source / p) == h for p, h in base['source_sha256'].items())
    affected = ['crates/rrrah-dedup/src/decode.rs',
                'crates/rrrah-dedup/src/local_collection.rs',
                'crates/rrrah-dedup/tests/local_collection.rs',
                'crates/rrrah-dedup/tests/visual_publication.rs',
                'crates/rrrah-dedup/tests/raw_selected.rs']
    assert all(sha(ROOT / p) == base['source_sha256'][p] for p in affected)
    log = TMP / 'rrrah-main-lifecycle-decode-20261013.log'
    text = log.read_text()
    summaries = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
    assert summaries == [('20', '0', '0'), ('2', '0', '0'), ('1', '0', '0')], summaries
    assert 'test result: FAILED' not in text
    binaries = re.findall(r'Running tests/(\w+)\.rs \(([^)]+)\)', text)
    assert [name for name, _ in binaries] == ['local_collection', 'raw_selected', 'visual_publication']
    dest = TMP / 'rrrah-main-lifecycle-qualified-20261013'
    output = DOC / 'dedup-main-lifecycle-audit-20261013.json'
    assert not output.exists() and not dest.exists()
    dest.mkdir()
    saved = {}
    for name, binary in binaries:
        target = dest / name
        shutil.copy2(binary, target)
        assert sha(binary) == sha(target)
        saved[str(target)] = sha(target)
    result = dict(status='verified_23_tests_on_one_snapshot', passed=23, failed=0, ignored=0,
                  source=str(source), source_sha256=base['source_sha256'],
                  affected_root_sha256={p: sha(ROOT / p) for p in affected},
                  log=str(log), log_sha256=sha(log), executables=saved,
                  prior_audit=str(prior), prior_audit_sha256=sha(prior),
                  command='cargo test --offline -p rrrah-dedup --features decode --test local_collection --test visual_publication --test raw_selected -- --test-threads=1',
                  excluded_attempt='First command omitted decode, executed zero tests and is not counted.',
                  scope='Combined local20/visual-publication1/RAW2 tests on same3750-file snapshot. Does not qualify full current integration tree, all formats/cameras, semantic precision or hard RSS.')
    output.write_text(json.dumps(result, indent=2) + '\n')
    print(output, result['status'])


if __name__ == '__main__':
    main()
