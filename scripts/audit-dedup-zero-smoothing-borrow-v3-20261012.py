#!/usr/bin/env python3
import hashlib, json, re, shutil, subprocess
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def function(text, name):
    start = text.index('fn ' + name + '(')
    cursor = text.index('{', start) + 1
    depth = 1
    while depth:
        depth += (text[cursor] == '{') - (text[cursor] == '}')
        cursor += 1
    return text[start:cursor]

def main():
    mp = D / 'dedup-zero-smoothing-borrow-source-v3-20261012.json'
    m = json.loads(mp.read_text())
    parent = Path(m['parent_manifest'])
    assert sha(parent) == m['parent_sha256']
    p = json.loads(parent.read_text())
    source = Path(m['snapshot'])
    assert all(sha(source / k) == v for k, v in m['file_hashes'].items())
    assert [k for k, v in m['file_hashes'].items() if p['file_hashes'][k] != v] == m['deltas_from_v2'] == ['crates/rrrah-dedup/src/local_collection.rs']
    key = 'crates/rrrah-dedup/src/local_collection.rs'
    before = (Path(p['snapshot']) / key).read_text()
    after = (source / key).read_text()
    assert function(before, 'extract_candidate_union_proposals').replace('extract_candidate_union_proposals', 'baseline_union') == function(after, 'baseline_union')
    assert function(before, 'extract_candidate_union_portfolio_proposals').replace('extract_candidate_union_portfolio_proposals', 'baseline_portfolio').replace('extract_candidate_union_proposals', 'baseline_union') == function(after, 'baseline_portfolio')
    log = T / 'rrrah-zero-smoothing-borrow-v3-collection-tests-20261012.log'
    text = log.read_text()
    assert '2 passed; 0 failed; 0 ignored; 0 measured; 93 filtered out' in text
    names = set(re.findall(r'^test (\S+) \.\.\. ok$', text, re.M))
    assert names == {
        'local_collection::zero_collection_candidate_tests::zero_collection_features_equal_owned_recipe_for_base_and_portfolio',
        'local_collection::zero_collection_candidate_tests::zero_collection_refusals_release_original_feature_credit',
    }
    binary = Path(re.search(r'Running unittests src/lib.rs \(([^)]+)\)', text)[1])
    dest = T / 'rrrah-zero-smoothing-borrow-v3-collection-executable-20261012'
    assert not dest.exists()
    shutil.copy2(binary, dest)
    assert sha(binary) == sha(dest)
    listed = subprocess.check_output([str(dest), '--list'], text=True)
    assert names <= set(re.findall(r'^(.*): test$', listed, re.M))
    out = D / 'dedup-zero-smoothing-borrow-v3-collection-verification-20261012.json'
    assert not out.exists()
    out.write_text(json.dumps({
        'status': 'verified_two_isolated_collection_zero_borrow_controls',
        'passed': 2, 'ignored': 0, 'filtered': 93, 'descriptor_configurations': 18,
        'pins': {str(x): sha(x) for x in [mp, parent, log, dest, Path(__file__)]},
        'scope': 'Three image shapes, base radii 0/2 and portfolios empty/[0]/[4,0]; every feature position and descriptor value equals copied original extraction functions and is nonempty. One-short work refusal, cancellation and healthy retry release all credit. Filtered tests are not passes. V3 full unit/file/collection gate and production adoption pending; no whole-pair RSS or universal coverage claim.',
    }, indent=2) + '\n')
    print('Verified two v3 collection controls; 18 descriptor configurations equal original recipe')

if __name__ == '__main__':
    main()
