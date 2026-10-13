#!/usr/bin/env python3
"""Check the frozen upstream tree and actual original/counterexample test events."""
import hashlib
import json
import tarfile
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research/dedup-500'
T = Path('/Users/themoretheless/.codex/tmp')
S = T / 'rrrah-go-find-duplicates-source-20261012'
C = T / 'rrrah-go-find-duplicates-controls-20261012'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def events(path, expected):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    assert not any(row['Action'] == 'fail' or (row['Action'] == 'skip' and 'Test' in row) for row in rows)
    for row in rows:
        if row['Action'] == 'skip':
            assert any(item.get('Package') == row['Package'] and '[no test files]' in item.get('Output', '') for item in rows)
    passed = [row['Test'] for row in rows if row['Action'] == 'pass' and 'Test' in row]
    assert sorted(passed) == sorted(expected), passed
    assert any(row['Action'] == 'pass' and 'Test' not in row for row in rows)
    return passed

def main():
    original = json.loads((D / 'reviews/026.json').read_text())
    assert original['commit'] == 'cd928ba207ba7b78d939597eb48590b1e78b52c6'
    for row in original['available_source_evidence'] + original['available_readme_license_evidence']:
        assert sha(S / row['path']) == row['sha256']
    files = {str(p.relative_to(S)): sha(p) for p in S.rglob('*') if p.is_file()}
    assert len(files) == 25
    archive = T / 'rrrah-go-find-duplicates-source-20261012.tar.gz'
    archived = {}
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            if member.isdir():
                continue
            assert member.isfile()
            relative = Path(*Path(member.name).parts[1:])
            assert '..' not in relative.parts and not relative.is_absolute()
            archived[str(relative)] = hashlib.sha256(tar.extractfile(member).read()).hexdigest()
    assert archived == files
    assert all(sha(C / name) == digest for name, digest in files.items())
    extra = {str(p.relative_to(C)) for p in C.rglob('*') if p.is_file()} - set(files)
    assert extra == {'service/rrrah_controls_test.go'}
    upstream_log = T / 'rrrah-go-find-duplicates-original-tests-20261012.jsonl'
    control_log = T / 'rrrah-go-find-duplicates-controls-20261012.jsonl'
    upstream = events(upstream_log, ['TestFormats', 'TestConfig', 'TestGetDigest', 'TestFindDuplicates', 'TestNonThoroughVsNot'])
    controls = events(control_log, [
        'TestRrrahCrossExtensionMissAndSameExtensionPositive',
        'TestRrrahUnsampledDifferenceIsFalseDuplicateOnlyInFastMode',
        'TestRrrahMissingRootReturnsNoError',
        'TestRrrahHardlinkIsCountedAsRedundantCopy',
    ])
    pins = {str(S / name): digest for name, digest in files.items()}
    for path in [archive, upstream_log, control_log, C / 'service/rrrah_controls_test.go',
                 T / 'rrrah-go-find-duplicates-original-tests-20261012.stderr',
                 T / 'rrrah-go-find-duplicates-controls-20261012.stderr',
                 T / 'rrrah-go-1.27.2-20261012/go/bin/go', Path(__file__)]:
        pins[str(path)] = sha(path)
    report = {
        'status': 'complete_pinned_tree_source_review_with_scoped_runtime_controls',
        'repository': original['repository'], 'commit': original['commit'],
        'source_file_count': 25, 'source_file_hashes': files,
        'source_review': 'All25 source/test/documentation/config/license/lock files completely read. Archive tree and original selected pins independently match. Third-party dependency source not reviewed.',
        'license': 'Apache-2.0; root LICENSE read; no upstream implementation adopted',
        'runtime': 'Go1.27.2 darwin/arm64; original declared dependencies downloaded by go test; original source and copied production source unchanged',
        'upstream_passed': upstream, 'counterexamples_passed': controls,
        'findings': [
            'Public FindDuplicates misses identical cross-extension files even in thorough mode; same-extension positive succeeds.',
            'Fast mode reports unequal65536-byte files as duplicates when only byte12000 differs outside sampled intervals; thorough mode separates them.',
            'Missing root returns nil error and empty results, losing completeness information.',
            'Physical hardlink aliases are reported as a redundant copy and reclaimable bytes.',
        ],
        'source_only_observations': [
            'Thorough hashing uses os.ReadFile and retains a whole file per active worker; no total-RSS benchmark performed.',
            'No post-read mutation validation or final byte comparison; no physical race/collision control executed.',
            'Public parallelism0 has no worker progress; no hanging runtime control executed.',
            'Report writers ignore short write counts; no report-writer runtime injection executed.',
        ],
        'scope': 'Five original tests and four authored controls on macOS ARM64. Passing controls document upstream defects, not desirable outcomes. No Windows/Linux, broad corpus, resource, all500 or complete-library qualification.',
        'pins': pins,
    }
    output = D / 'go-find-duplicates-complete-source-review-20261012.json'
    assert not output.exists()
    output.write_text(json.dumps(report, indent=2) + '\n')
    print('Verified25 files,5 original tests,4 counterexample tests; no adoption')

if __name__ == '__main__':
    main()
