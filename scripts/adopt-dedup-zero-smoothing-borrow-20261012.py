#!/usr/bin/env python3
"""Adopt exactly the qualified source after preserving and checking old files."""
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')
KEYS = [
    'crates/rrrah-dedup/src/gradient_scale.rs',
    'crates/rrrah-dedup/src/local_scan.rs',
    'crates/rrrah-dedup/src/local_collection.rs',
    'crates/rrrah-dedup/tests/candidate_union_files.rs',
]

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def function(text, prefix):
    start = text.index(prefix)
    end = text.index('{', start) + 1
    depth = 1
    while depth:
        depth += (text[end] == '{') - (text[end] == '}')
        end += 1
    return text[start:end]

def normalized(code):
    return subprocess.check_output(
        ['rustfmt', '--edition', '2024', '--config-path', str(R / 'rustfmt.toml'), '--emit', 'stdout'],
        input=code, text=True,
    )

def main():
    mp = D / 'dedup-zero-smoothing-borrow-source-v4-20261012.json'
    m = json.loads(mp.read_text())
    source = Path(m['snapshot'])
    baseline_mp = D / 'dedup-fingerprint-restored-mtime-source-20261012.json'
    baseline = json.loads(baseline_mp.read_text())
    assert all(sha(source / k) == v for k, v in m['file_hashes'].items())
    for k, v in baseline['file_hashes'].items():
        if k.startswith('crates/rrrah-dedup/src/') or k in KEYS:
            assert sha(R / k) == v, ('changed since baseline, preserve user edits', k)
    gates = [D / 'dedup-zero-smoothing-borrow-v4-integrated-verification-20261012.json',
             D / 'dedup-zero-smoothing-borrow-v4-raster-verification-v2-20261012.json']
    for gate in gates:
        value = json.loads(gate.read_text())
        assert all(sha(k) == v for k, v in value['pins'].items())
        assert value['pins'][str(mp)] == sha(mp)
    integrated = json.loads(gates[0].read_text())
    assert integrated['passed'] == 123 and integrated['ignored'] == 5
    memory_mp = D / 'dedup-zero-smoothing-borrow-memory-20261012.json'
    memory = json.loads(memory_mp.read_text())
    assert all(sha(k) == v for k, v in memory['pins'].items())
    v2_mp = D / 'dedup-zero-smoothing-borrow-source-v2-20261012.json'
    v2 = json.loads(v2_mp.read_text())
    key = KEYS[0]
    old = (Path(v2['snapshot']) / key).read_text()
    new = (source / key).read_text()
    helper = 'pub(crate) fn validate_unsmoothed_gradient_candidates('
    owned = 'pub fn smooth_gradient_candidates_managed('
    assert normalized(function(old, helper)) == normalized(function(new, helper))
    assert normalized(function((R / key).read_text(), owned)) == normalized(function(new, owned))
    before = T / 'rrrah-zero-smoothing-borrow-adoption-before-20261012'
    out = D / 'dedup-zero-smoothing-borrow-adoption-20261012.json'
    assert not before.exists() and not out.exists()
    before.mkdir()
    old_hashes = {k: sha(R / k) for k in KEYS}
    for k in KEYS:
        dest = before / k
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(R / k, dest)
        assert sha(dest) == old_hashes[k]
    for k in KEYS:
        assert sha(R / k) == old_hashes[k]
        shutil.copy2(source / k, R / k)
        assert sha(R / k) == m['file_hashes'][k]
    subprocess.run(['git', 'diff', '--check', '--', *KEYS], cwd=R, check=True)
    pins = {str(p): sha(p) for p in [mp, baseline_mp, v2_mp, memory_mp, *gates, Path(__file__)]}
    out.write_text(json.dumps({
        'status': 'adopted_qualified_zero_radius_borrowing_in_pairs_and_collections',
        'root_file_hashes': {k: sha(R / k) for k in KEYS},
        'preserved_before': str(before), 'before_hashes': old_hashes,
        'integrated_passed': 123, 'integrated_ignored': 5,
        'pins': pins,
        'scope': 'Exactly qualified three production files and one integration test file copied after checking existing root bytes against baseline and preserving them. Public owned smoothing function unchanged after repository formatting. Identity validator equals previously measured v2 after formatting; prior 102400000-to-52068352 RSS remains one identity-stage control, not wholepair/process cap. Full decode gate and separate raster gate overlap and are not summed. Compatible frozen dependencies; no fullworkspace/Linuxcandidate/Windows/fullcorpus/full500qualification.',
    }, indent=2) + '\n')
    print('Adopted qualified zero-radius borrowing; 123 decode-gate passes, 5 ignored')

if __name__ == '__main__':
    main()
