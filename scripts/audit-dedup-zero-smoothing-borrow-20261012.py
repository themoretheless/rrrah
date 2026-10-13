#!/usr/bin/env python3
import hashlib, json, re, shutil, subprocess
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def main():
    manifest = D / 'dedup-zero-smoothing-borrow-source-v2-20261012.json'
    m = json.loads(manifest.read_text())
    source = Path(m['snapshot'])
    assert sha(m['parent_manifest']) == m['parent_sha256']
    assert all(sha(source / k) == v for k, v in m['file_hashes'].items())
    pins = {str(manifest): sha(manifest), str(Path(__file__)): sha(__file__)}
    suites = []
    for label, logname, expected, filtered in [
        ('unit', 'rrrah-zero-smoothing-borrow-v2-raster-tests-20261012.log', 2, 43),
        ('pair', 'rrrah-zero-smoothing-borrow-v2-pair-tests-20261012.log', 15, 0),
    ]:
        log = T / logname
        text = log.read_text()
        names = re.findall(r'^test (\S+) \.\.\. ok$', text, re.M)
        results = re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; (\d+) filtered out', text)
        assert len(names) == len(set(names)) == expected
        assert sum(int(x[0]) for x in results) == expected
        assert sum(int(x[1]) for x in results) == filtered
        binaries = re.findall(r'Running .*? \((/[^)]+)\)', text)
        listed_names = set()
        for i, binary in enumerate(binaries):
            original = Path(binary)
            dest = T / f'rrrah-zero-smoothing-borrow-v2-{label}-{i}-executable-20261012'
            assert not dest.exists()
            shutil.copy2(original, dest)
            assert sha(dest) == sha(original)
            listed = subprocess.check_output([str(dest), '--list'], text=True)
            listed_names.update(re.findall(r'^(.*): test$', listed, re.M))
            pins[str(dest)] = sha(dest)
        assert set(names) <= listed_names
        if not filtered:
            assert set(names) == listed_names
        pins[str(log)] = sha(log)
        suites.append({'suite': label, 'passed': expected, 'filtered': filtered, 'tests': names})
    report = D / 'dedup-zero-smoothing-borrow-memory-20261012.json'
    memory = json.loads(report.read_text())
    assert all(sha(k) == v for k, v in memory['pins'].items())
    mm = json.loads(Path(next(k for k in memory['pins'] if k.endswith('memory-source-20261012.json'))).read_text())
    assert all(sha(Path(mm['snapshot']) / k) == v for k, v in mm['file_hashes'].items())
    key = 'crates/rrrah-dedup/src/gradient_scale.rs'
    assert (Path(mm['snapshot']) / key).read_bytes().startswith((source / key).read_bytes())
    baseline, borrow = memory['rows']
    for row in memory['rows']:
        assert row['returncode'] == 0 and json.loads(row['stdout']) == row['result']
        assert row['result']['after_drop'] == 0
        assert int(re.search(r'(\d+)\s+maximum resident set size', row['stderr'])[1]) == row['rss_bytes']
    assert baseline['command'][:-1] == borrow['command'][:-1]
    assert baseline['result']['digest'] == borrow['result']['digest']
    assert baseline['result']['managed_peak'] == baseline['result']['retained'] == 50331648
    assert borrow['result']['managed_peak'] == borrow['result']['retained'] == 0
    pins[str(report)] = sha(report)
    out = D / 'dedup-zero-smoothing-borrow-verification-20261012.json'
    assert not out.exists()
    out.write_text(json.dumps({'status': 'verified_isolated_candidate_17_tests_and_identity_stage_memory', 'passed': 17, 'ignored': 0, 'suites': suites, 'pins': pins, 'rss_bytes': [baseline['rss_bytes'], borrow['rss_bytes']], 'scope': 'Isolated v2 candidate, root not adopted. Two helper tests, nine existing file/collection lifecycle tests and six smoothing tests pass. 43 filtered unit tests are not counted. Full pixel digest identical for one identity-stage memory control. No whole-pair memory, collection extractor optimization, Linux candidate, full corpus or full500 qualification.'}, indent=2) + '\n')
    print('Verified isolated candidate: 17 tests and identity-stage memory evidence; root not adopted')

if __name__ == '__main__':
    main()
