"""Audit frozen current gradient suites on native Linux."""
import hashlib, json, re
from pathlib import Path
D = Path('docs/research')
manifest = D / 'dedup-result-growth-snapshot.json'
log = D / 'dedup-result-growth-linux.log'
out = D / 'dedup-result-growth-linux-state.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
m = json.loads(manifest.read_text())
snapshot = Path(m['snapshot'])
for path, pin in m['file_hashes'].items():
    assert sha(snapshot / path) == pin
suites = ['gradient_index','gradient_index_reflection','anchor_reflection_collection','anchor_reflection_collection_cancel','anchor_rank_driven_collection']
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
assert len(actual) == len(set(actual)) == len(declared)
assert set(actual) == declared
assert set(re.findall(r'Running tests/(\w+)\.rs', text)) == set(suites)
results = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
assert len(results) == len(suites) and sorted(int(n) for n, _, _ in results) == sorted(counts)
assert all(int(f) == int(i) == 0 for _, f, i in results)
assert 'error:' not in text and 'FAILED' not in text
drift = [p for p, pin in m['file_hashes'].items() if p.startswith('crates/rrrah-dedup/')
         and (not Path(p).is_file() or sha(p) != pin)]
out.write_text(json.dumps(dict(status='verified_native_linux_result_growth_collection', tests=len(declared),
    suites=suites, current_dedup_drift=drift,
    pins={str(p): sha(p) for p in [manifest, log, Path(__file__)]},
    scope='Native aarch64 Linux current reflected index/pair/collection, fixture positive/foreign all-direct parity, ten-cap admission, early cancellation and ordinary/exhaustive index regressions. No late mutation lifecycle, broad real precision/recall, Windows or full package claim.'), indent=2) + '\n')
print('verified native Linux reflection/memory tests', len(declared), drift)
