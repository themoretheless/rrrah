"""Transfer exactly equal complete evidence from the pinned independent oracle."""
import hashlib, json
from pathlib import Path
D = Path('docs/research')
current = D / 'dedup-anchor-rank-driven-full-checkpoint-41.json'
previous = D / 'dedup-anchor-automatic-pixels-prefix-4.json'
oracle = D / 'dedup-anchor-automatic-pixels-prefix-4-audit.json'
out = D / 'dedup-anchor-rank-driven-pixels-prefix-4-parity-audit.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
n, o, a = [json.loads(p.read_text()) for p in (current, previous, oracle)]
assert a['status'] == 'verified_automatic_anchor_original_point_selection_independent_pixels'
assert a['pairs'] == 4 and len(o['results']) >= 4 and a['directions'] == 8
for path, pin in a['pins'].items():
    assert sha(path) == pin
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
for x, y in zip(n['results'][:4], o['results']):
    assert (x['original'], x['query'], x['source_tolerance']) == (y['original'], y['query'], y['source_tolerance'])
    for path in [T / 'original-resolution-negative-200201' / x['original'],
                 T / 'original-resolution-strong-all' / x['query']]:
        assert n['pins'][str(path)] == o['pins'][str(path)] == sha(path)
    assert x['returncode'] == y['returncode'] == 0
    assert x['evidence']['attempted_recipes'] == 1 and x['evidence']['smoothing_radii'] == [2, 2]
    evidence = dict(x['evidence'])
    evidence.pop('attempted_recipes')
    evidence.pop('smoothing_radii')
    assert evidence == y['evidence']
out.write_text(json.dumps(dict(status='verified_rank_driven_prefix_independent_pixel_evidence_parity',
    pairs=4, directions=8,
    pins={str(p): sha(p) for p in [current, previous, oracle, Path(__file__)]},
    scope='Identical input bytes/tolerance and complete original points/model/anchor/count evidence transfer the pinned NumPy oracle to four current frozen rank-driven results. No new decoder independence, remainder-corpus or current borrowed-gradient claim.'), indent=2) + '\n')
print('verified four complete evidence matches to independent pixel oracle')
