"""Compare audited regional evidence; color-space tolerances are not equivalent."""
import hashlib
import json
from pathlib import Path

digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
baseline = Path('docs/research/dedup-fresh-union-original-full-strict-checkpoint-9-next.json')
experiment = Path('docs/research/dedup-screen-copy-encoded.json')
audits = [baseline.with_name(baseline.stem + '-audit.json'), experiment.with_name(experiment.stem + '-audit.json')]
for report, audit in zip([baseline, experiment], audits):
    assert json.loads(audit.read_text())['input_hashes'][str(report)] == digest(report)
left = next(row['evidence'] for row in json.loads(baseline.read_text())['results'] if row['query'] == '200501.jpg')
right = json.loads(experiment.read_text())['evidence']
assert all(left[key] == right[key] for key in ['correspondences', 'matrix', 'inliers'])
assert len(left['regions']) == len(right['regions']) == 78
rows = []
for index, (a, b) in enumerate(zip(left['regions'], right['regions'])):
    assert a['domains'] == b['domains']
    measurements = []
    for region in [a, b]:
        pixels = region['pixels']
        measurements.append(None if pixels is None else {
            'minimum_matched_fraction': min(matched / compared if compared else 0 for matched, compared, total in pixels['counts']),
            'minimum_coverage': min(compared / total if total else 0 for matched, compared, total in pixels['counts']),
            'offset_components_at_bound': sum(abs(value) >= .1 - 1e-12 for direction in pixels['offsets'] for value in direction),
            'accepted': region['accepted'],
        })
    rows.append({'region': index, 'linear': measurements[0], 'encoded': measurements[1]})
paired = [row for row in rows if row['linear'] is not None and row['encoded'] is not None]
result = {
    'status': 'verified_paired_region_diagnostic',
    'identical_geometry_and_domains': True,
    'regions': rows,
    'paired_pixel_regions': len(paired),
    'encoded_higher_fraction': sum(row['encoded']['minimum_matched_fraction'] > row['linear']['minimum_matched_fraction'] for row in paired),
    'best_linear': max(paired, key=lambda row: row['linear']['minimum_matched_fraction']),
    'best_encoded': max(paired, key=lambda row: row['encoded']['minimum_matched_fraction']),
    'accepted_regions': [sum(row[key] is not None and row[key]['accepted'] for row in rows) for key in ['linear', 'encoded']],
    'scope': 'Same geometry and regional domains isolate downstream comparison behavior. Numeric tolerance .03 in different color spaces is not physically equivalent; this is not a causal color-space benchmark or justification to relax acceptance. Both measured recipes miss this capture.',
    'input_hashes': {str(path): digest(path) for path in [baseline, experiment, *audits, Path(__file__)]},
}
output = Path('docs/research/dedup-screen-color-pair.json')
assert not output.exists()
output.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({key: value for key, value in result.items() if key not in ['regions', 'input_hashes']}, indent=2))
