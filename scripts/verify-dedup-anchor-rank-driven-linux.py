"""Audit declared tests, completed native Linux results, frozen bytes and drift."""
import hashlib
import json
import re
from pathlib import Path

D = Path('docs/research')
manifest = D / 'dedup-anchor-rank-driven-linux-snapshot.json'
m = json.loads(manifest.read_text())
snapshot = Path(m['snapshot'])
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
for name, digest in m['file_hashes'].items():
    assert sha(snapshot / name) == digest, name
suites = ['candidate_geometry_only', 'anchor_geometry_search', 'anchor_fallback_search', 'anchor_fallback_collection', 'anchor_fallback_collection_lifecycle', 'anchor_collection', 'anchor_collection_preflight', 'anchor_collection_scaled', 'anchor_candidate_search', 'anchor_rank_file', 'anchor_rank', 'local_rank', 'local_rank_cached', 'piecewise_local_rank', 'piecewise_warp', 'mesh_rank_regions', 'projective_grid', 'mesh_grid', 'mesh_rank', 'rank_region', 'projective_exhaustive_domains', 'projective_domains', 'projective_geometry', 'anchor_geometry_rank_fallback', 'anchor_rank_driven_collection', 'anchor_rank_driven_collection_preflight', 'anchor_rank_driven_collection_lifecycle', 'anchor_file_search_preflight', 'anchor_geometry_smoothing', 'anchor_geometry_collection', 'anchor_geometry_collection_preflight', 'anchor_geometry_collection_lifecycle', 'anchor_fallback_collection_preflight']
declared = set()
counts = {}
for suite in suites:
    text = (snapshot / 'crates/rrrah-dedup/tests' / (suite + '.rs')).read_text()
    names = set(re.findall(r'#\[test\]\s*(?:#\[[^\n]*\]\s*)*fn\s+(\w+)', text))
    assert names and not names & declared
    declared |= names
    counts[suite] = len(names)
log = D / 'dedup-anchor-rank-driven-linux-native.log'
text = log.read_text()
actual = re.findall(r'^test (\w+) \.\.\. ok$', text, re.M)
assert len(actual) == len(set(actual)) == len(declared) and set(actual) == declared
assert set(re.findall(r'Running tests/(\w+)\.rs', text)) == set(suites)
results = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
assert len(results) == len(suites)
assert sorted(int(n) for n, _, _ in results) == sorted(counts.values())
assert all(int(f) == int(i) == 0 for _, f, i in results)
assert 'FAILED' not in text and 'error:' not in text
drift = [name for name, digest in m['file_hashes'].items()
         if name.startswith('crates/rrrah-dedup/') and
         (not Path(name).is_file() or sha(name) != digest)]
gate = {'status': 'verified_native_linux_rank_driven_anchor_suites',
        'tests': len(declared), 'suites': counts, 'snapshot_files': len(m['file_hashes']),
        'current_dedup_drift': drift,
        'pins': {str(p): sha(p) for p in [manifest, log, Path(__file__)]},
        'scope': 'Native aarch64 Linux tests on frozen dedup bytes and prior frozen dependencies. '
                 'Includes automatic rank-driven fallback and indexed collection parity/lifecycle/cumulative admission, geometry-only parity/cancellation/tokens/mutation/restore, fallback file '
                 'and indexed collection direct parity/lifecycle/admission, existing rank/geometry regressions. '
                 'No Windows, real corpus precision, all contract cases, full-package or current unrelated dependency claim.'}
(D / 'dedup-anchor-rank-driven-linux-state.json').write_text(json.dumps(gate, indent=2) + '\n')
print(json.dumps({'tests': len(declared), 'current_dedup_drift': drift}))
