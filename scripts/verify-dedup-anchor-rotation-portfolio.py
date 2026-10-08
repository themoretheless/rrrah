"""Audit every fixture outcome, original-coordinate geometry and native counts."""
import hashlib, json, math, struct
from pathlib import Path
import numpy as np
D = Path('docs/research')
p = D / 'dedup-anchor-rotation-current.json'
out = D / 'dedup-anchor-rotation-current-audit.json'
assert not out.exists()
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
r = json.loads(p.read_text())
assert r['status'] == 'complete_rotation_fixture_diagnostic'
for path, digest in r['pins'].items():
    assert sha(path) == digest
for path, digest in r['source_hashes'].items():
    assert sha(Path(r['source_snapshot']) / path) == digest
names = ['base', 'angle-17', 'angle--37', 'angle-63', 'scale-117-angle-0',
         'scale-216-angle-0', 'scale-216-angle-17', 'mirrored', 'unrelated']
assert [row['query'] for row in r['results']] == [name + '.png' for name in names]
root = Path('crates/rrrah-dedup/tests/fixtures/rotation')
size = lambda path: struct.unpack('>II', path.read_bytes()[16:24])
source_size = size(root / 'base.png')
supported, misses = [], []
for row in r['results']:
    target_size = size(root / row['query'])
    assert row['source_tolerance'] == 2 * math.hypot(*source_size) / math.hypot(*target_size)
    assert row['returncode'] == 0
    e = row['evidence']
    assert e == json.loads(row['stdout'])
    assert e['smoothing_radii'] == [[2, 2], [4, 0], [0, 4]][e['attempted_recipes'] - 1]
    if e['status'] == 'no_geometry':
        assert e['attempted_recipes'] == 3 and not e['supported']
    else:
        assert e['status'] == 'ok'
        H = np.array(e['matrix'])
        inverse = np.linalg.inv(H)
        assert np.isfinite(H).all() and np.isfinite(inverse).all()
        for matrix, dimensions in [(H, source_size), (inverse, target_size)]:
            values = [matrix[2] @ np.array([x, y, 1.]) for x in [0, dimensions[0] - 1] for y in [0, dimensions[1] - 1]]
            assert all(v > 0 for v in values) or all(v < 0 for v in values)
        points = e['points']
        for i, (a, b) in enumerate(points):
            assert all(math.isfinite(v) for v in a + b)
            assert all(0 <= v < bound for v, bound in zip(a, source_size))
            assert all(0 <= v < bound for v, bound in zip(b, target_size))
            assert all(math.dist(a, old[0]) > 2 and math.dist(b, old[1]) > 2 for old in points[:i])
        predicates = []
        for anchors, counts in zip(e['anchors'], e['directions']):
            assert anchors >= 10 and counts['sites'] == 25 * anchors
            assert 0 <= counts['agreeing_pairs'] <= counts['informative_pairs'] <= 8 * counts['valid_sites'] <= 8 * counts['sites']
            predicates.append(counts['informative_pairs'] >= 1000 and counts['valid_sites'] / counts['sites'] >= .3 and counts['agreeing_pairs'] / counts['informative_pairs'] >= .9)
        assert e['supported'] == all(predicates)
    if row['query'] == 'unrelated.png':
        assert not e['supported']
    elif e['supported']:
        supported.append(row['query'])
    else:
        misses.append(row['query'])
out.write_text(json.dumps(dict(status='verified_rotation_fixture_outcomes',
    supported_positives=supported, missed_positives=misses, unrelated_rejected=True,
    pins={str(p): sha(p), str(Path(__file__)): sha(__file__)},
    scope='Every fixture, source/binary/input provenance, original-point bounds/aliases, whole projective denominator domains and native count predicates checked. Not independent pixel recomputation, real angular recall or whole-library reflection proof.'), indent=2) + '\n')
print('verified positives', len(supported), 'misses', misses)
