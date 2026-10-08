"""Audit frozen current gradient suites on native Linux."""
import hashlib, json, re
from pathlib import Path
D = Path('docs/research')
manifest = D / 'dedup-pixel-admission-macos-snapshot.json'
log = D / 'dedup-pixel-admission-linux.log'
out = D / 'dedup-pixel-admission-linux-state.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
m = json.loads(manifest.read_text())
snapshot = Path(m['snapshot'])
for path, pin in m['file_hashes'].items():
    assert sha(snapshot / path) == pin
suites = ['gradient_scale','gradient_scale_files','gradient_distinct_locations','projective_domains','projective_geometry','gradient','gradient_interpolated']
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
out.write_text(json.dumps(dict(status='verified_native_linux_pixel_admission', tests=len(declared),
    suites=suites, current_dedup_drift=drift,
    pins={str(p): sha(p) for p in [manifest, log, Path(__file__)]},
    scope='Native aarch64 Linux per-level pixel admission, managed/unmanaged feature parity, area scale/file extraction, distinct locations, projective domain/geometry and gradient regression suites. Frozen complete workspace. No real full collection, broad recall/precision, Windows or full package claim.'), indent=2) + '\n')
print('verified native Linux reflection/memory tests', len(declared), drift)
