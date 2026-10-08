"""Verify frozen automatic selected-recipe/direct parity and native predicates."""
import argparse
import hashlib
import json
import math
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('report', type=Path)
p.add_argument('output', type=Path)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
assert not a.output.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
digest = sha(a.report)
r = json.loads(a.report.read_text())
assert r['status'] == 'complete' or a.checkpoint and r['status'] == 'running'
D = Path('docs/research')
prior = json.loads((D / 'dedup-anchor-fallback-recovered-real.json').read_text())
assert r['required_pairs'] == 3 and 0 < len(r['results']) <= 3
if not a.checkpoint:
    assert len(r['results']) == 3
for name, pin in r['pins'].items():
    assert sha(name) == pin, name
summary = []
for row, old in zip(r['results'], prior['results']):
    for key in ('original', 'query', 'source_tolerance'):
        assert row[key] == old[key]
    assert row['returncode'] == row['direct_returncode'] == 0 and row['direct_parity_verified']
    e = row['evidence']
    assert e == json.loads(row['stdout']) and row['direct_evidence'] == json.loads(row['direct_stdout'])
    assert row['direct_evidence'] == {k: v for k, v in e.items() if k not in ('attempted_recipes', 'smoothing_radii')}
    assert e['status'] == 'ok' and e['supported'] is True
    assert 1 <= e['attempted_recipes'] <= 3
    assert e['smoothing_radii'] == [[2, 2], [4, 0], [0, 4]][e['attempted_recipes'] - 1]
    assert len(e['matrix']) == 3 and all(len(v) == 3 and all(math.isfinite(x) for x in v) for v in e['matrix'])
    assert len(e['points']) <= 28000 and len(e['anchors']) == len(e['directions']) == 2
    for anchors, counts in zip(e['anchors'], e['directions']):
        assert anchors >= 10 and counts['sites'] == 25 * anchors
        assert 0 <= counts['agreeing_pairs'] <= counts['informative_pairs'] <= 8 * counts['valid_sites'] <= 8 * counts['sites']
        assert counts['informative_pairs'] >= 1000 and counts['valid_sites'] / counts['sites'] >= .3
        assert counts['agreeing_pairs'] / counts['informative_pairs'] >= .9
    summary.append({'query': row['query'], 'attempted_recipes': e['attempted_recipes'],
                    'smoothing_radii': e['smoothing_radii'], 'seconds': row['seconds']})
manifest = json.loads((D / 'dedup-anchor-rank-driven-fallback-real-source.json').read_text())
assert sha(manifest['binary']) == manifest['binary_sha256']
for name, pin in manifest['file_hashes'].items():
    assert sha(Path(manifest['snapshot']) / name) == pin
assert sha(a.report) == digest
audit = {'status': 'verified_rank_driven_fallback_real_checkpoint' if a.checkpoint else 'verified_rank_driven_fallback_real_terminal',
         'pairs': len(summary), 'required_pairs': 3, 'summary': summary, 'report_sha256': digest,
         'scope': 'Frozen inputs/source/binary, exact automatic selected recipe versus fresh direct native evidence, support predicates. Earlier unsuccessful attempts are covered by native implementation/authored tests, not independently replayed here. No full-corpus recall, broad precision or collection qualification.'}
a.output.write_text(json.dumps(audit, indent=2) + '\n')
print(json.dumps(audit))
