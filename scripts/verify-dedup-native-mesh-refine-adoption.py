"""Check native refinement/admission/full-field rank against frozen qualified data.

Shared normalized decoder input; selected proposals are not copy decisions.
"""
import hashlib
import json
from pathlib import Path

pins_path = Path('docs/research/dedup-native-mesh-refine-adoption-input-pins.json')
pins = json.loads(pins_path.read_text())
def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert all(digest(p) == h for p, h in pins.items())
report = Path('docs/research/dedup-native-mesh-refine-adoption.jsonl')
report_hash = digest(report)
rows = [json.loads(line) for line in report.read_text().splitlines()]
assert rows[0] == dict(kind='admission', original_points=156, eligible=40, duplicates=2, points=194)
points = rows[1:195]
assert all(p['kind'] == 'point' for p in points)
def landmarks(path):
    return [list(map(float, line.split())) for line in Path(path).read_text().splitlines()]
actual = [p['source'] + p['target'] for p in points]
assert actual[:156] == landmarks('docs/research/dedup-folded-qualified-landmarks.txt')
expected = landmarks('docs/research/dedup-folded-dense-small-window-distinct-landmarks.txt')
assert len(expected) == len(actual) == 194
# Native atlas barycentric arithmetic may differ by a few ulps from the independent grid.
for a, b in zip(actual, expected):
    assert all(abs(x-y) <= 1e-8 for x, y in zip(a, b))
for i, p in enumerate(actual):
    for q in actual[:i]:
        assert all(sum((p[j+k]-q[j+k])**2 for k in range(2)) > 4 for j in (0, 2))
reference = [json.loads(line) for line in Path('docs/research/dedup-folded-dense-distinct-native-mesh-rank.jsonl').read_text().splitlines()]
assert len(rows[195:]) == len(reference) == 6
assert rows[195:] == reference
assert digest(report) == report_hash
assert all(digest(p) == h for p, h in pins.items())
output = Path('docs/research/dedup-native-mesh-refine-adoption-audit.json')
assert not output.exists()
output.write_text(json.dumps(dict(
    status='verified_native_refinement_admission_whole_field_parity',
    original_points=156, eligible_proposals=40, duplicate_proposals=2,
    retained_new_points=38, final_points=194, whole_field_comparisons=6,
    pins={**pins, str(pins_path): digest(pins_path), str(report): report_hash,
          'scripts/verify-dedup-native-mesh-refine-adoption.py': digest(__file__)},
    scope='Native proposal selection, stable original-preserving spatial admission, whole-mesh triangulation/diagonal/conformity and forward/reverse rank. Six counts exactly equal independently pixel-qualified frozen results. Coordinates within 1e-8. Shared decoder-normalized pixels, original correspondence retrieval supplied; no independent spatial holdout, broad negatives, Linux, automatic file API or copy admission qualification.'
), indent=2)+'\n')
print('Verified 194 landmarks and six whole-field comparisons')
