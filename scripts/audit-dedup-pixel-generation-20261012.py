#!/usr/bin/env python3
"""Audit two public pixel-confirmation cancellation regressions."""
import hashlib
import json
import re
import shutil
import subprocess
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    rp = D / 'dedup-pixel-generation-red-20261012.json'
    red = json.loads(rp.read_text())
    assert all(sha(p) == h for p, h in red['pins'].items())
    before = (T / 'rrrah-pixel-generation-source-before-20261012.rs').read_text()
    source = R / 'crates/rrrah-dedup/src/scan.rs'
    text = source.read_text()
    start = text.index('    let mut tokens = Vec::new();', text.index('pub fn confirm_pixels('))
    end = text.index('    let mut candidates = std::collections::HashSet::new();', start)
    doc = ('/// Cancellation of any admitted request generation invalidates the batch,\n'
           '/// including when no candidate pairs are supplied or during final publication.\n')
    assert (text[:start] + text[end:]).replace(doc, '', 1) == before
    test_source = R / 'crates/rrrah-dedup/tests/scan.rs'
    assert sha(test_source) == sha(T / 'rrrah-pixel-generation-red-tests-20261012.rs')
    log = T / 'rrrah-pixel-generation-green-20261012.log'
    output = log.read_text()
    assert '10 passed; 0 failed; 0 ignored' in output
    names = re.findall(r'^test (\S+) \.\.\. ok$', output, re.M)
    assert len(names) == len(set(names)) == 10
    assert 'pixel_generation_cancelled_empty_pair_batch_is_not_successful' in names
    assert 'pixel_generation_final_publication_discards_equal_pair' in names
    exe = Path(re.search(r'Running tests/scan.rs \(([^)]+)\)', output)[1])
    dest = T / 'rrrah-pixel-generation-green-executable-20261012'
    assert not dest.exists()
    shutil.copy2(exe, dest)
    listed = subprocess.check_output([str(dest), '--list'], text=True)
    assert sorted(names) == sorted(re.findall(r'^(.*): test$', listed, re.M))
    frozen = T / 'rrrah-pixel-generation-green-source-20261012'
    assert not frozen.exists()
    inputs = [source, test_source, R / 'crates/rrrah-dedup/tests/fixtures/timeline/base.png']
    pins = {str(p): sha(p) for p in [rp, log, dest, Path(__file__)]}
    for path in inputs:
        q = frozen / path.relative_to(R)
        q.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, q)
        assert sha(q) == sha(path)
        pins[str(q)] = sha(q)
    out = D / 'dedup-pixel-generation-verification-20261012.json'
    assert not out.exists()
    out.write_text(json.dumps({
        'status': 'verified_two_pixel_generation_cancellation_fixes_and_full_scan_suite',
        'passed': 10, 'ignored': 0, 'tests': names, 'pins': pins,
        'scope': 'Actual macOS ARM64 full decode-feature visual/pixel scan integration suite. New public controls reproduce empty-candidate and final-equality publication defects before fix and pass after. Production delta reconstructed exactly; test bytes unchanged from red. Frozen source/test/fixture and executable. No whole-dependency snapshot, physical race, other platforms, all458/full500/formats/total resources qualification.',
    }, indent=2) + '\n')
    print('Verified10 scan tests; two pixel generation cancellation regressions')

if __name__ == '__main__':
    main()
