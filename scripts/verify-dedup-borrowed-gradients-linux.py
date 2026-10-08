"""Audit frozen current gradient suites on native Linux."""
import hashlib, json, re
from pathlib import Path
D = Path('docs/research')
manifest = D / 'dedup-borrowed-gradients-linux-snapshot.json'
log = D / 'dedup-borrowed-gradients-linux.log'
out = D / 'dedup-borrowed-gradients-linux-state.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
m = json.loads(manifest.read_text())
snapshot = Path(m['snapshot'])
for path, pin in m['file_hashes'].items():
    assert sha(snapshot / path) == pin
suites = ['gradient', 'gradient_interpolated', 'gradient_distinct_locations', 'gradient_empty', 'gradient_index']
declared = set()
counts = []
for name in suites:
    text = (snapshot / 'crates/rrrah-dedup/tests' / (name + '.rs')).read_text()
    names = set(re.findall(r'#\[test\]\s*(?:#\[[^\n]*\]\s*)*fn\s+(\w+)', text))
    assert names and not declared & names
    declared |= names
    counts.append(len(names))
text = log.read_text()
actual = re.findall(r'^test (\w+) \.\.\. ok$', text, re.M)
assert len(actual) == len(set(actual)) == len(declared) == 25
assert set(actual) == declared
assert set(re.findall(r'Running tests/(\w+)\.rs', text)) == set(suites)
results = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
assert len(results) == 5 and sorted(int(n) for n, _, _ in results) == sorted(counts)
assert all(int(f) == int(i) == 0 for _, f, i in results)
assert 'error:' not in text and 'FAILED' not in text
drift = [p for p, pin in m['file_hashes'].items() if p.startswith('crates/rrrah-dedup/')
         and (not Path(p).is_file() or sha(p) != pin)]
out.write_text(json.dumps(dict(status='verified_native_linux_borrowed_gradients', tests=25,
    suites=suites, current_dedup_drift=drift,
    pins={str(p): sha(p) for p in [manifest, log, Path(__file__)]},
    scope='Native aarch64 Linux five frozen current gradient suites over prior frozen dependencies. Exact index/boundary/collision/cancellation/budget and ordinary/interpolated/distinct matching regressions; no full package, Windows or real corpus claim.'), indent=2) + '\n')
print('verified native Linux25tests', drift)
