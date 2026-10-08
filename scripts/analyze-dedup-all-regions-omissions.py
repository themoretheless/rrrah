#!/usr/bin/env python3
"""Classify observed regional refusals; do not infer model correctness."""
import argparse
import collections
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('report', 'audit', 'output'):
    p.add_argument(name, type=Path)
a = p.parse_args()
raw = {path: path.read_bytes() for path in (a.report, a.audit)}
report, audit = [json.loads(raw[path]) for path in (a.report, a.audit)]
digest = lambda data: hashlib.sha256(data).hexdigest()
assert digest(raw[a.report]) == audit['report_sha256']
assert audit['verified_pairs'] == audit['required_pairs'] == 229
assert report['status'] == 'complete' and len(report['results']) == 229
rows = []
reasons = collections.Counter()
seen = set()
for row in report['results']:
    qid, e = row['query_id'], row['evidence']
    assert qid not in seen
    seen.add(qid)
    lanes = e['gradient_regions']
    assert len(lanes) == 2
    if e['candidate'] or e['region_support_count'] or any(
            lane is not None and lane['region_support_count'] for lane in lanes):
        continue
    details = []
    for lane_index, lane in enumerate(lanes):
        if lane is None:
            continue
        assert lane['region_support_count'] == 0
        for region_index, region in enumerate(lane['regions']):
            assert region['accepted_region'] is False
            counts = region['counts']
            failures = set()
            if counts is None:
                failures.add(region['fit_failure'].split(' {')[0])
            else:
                assert len(counts) == 2
                for matched, compared, source in counts:
                    assert all(type(v) is int for v in (matched, compared, source))
                    assert 0 <= matched <= compared <= source
                    if compared < 1000:
                        failures.add('samples')
                    if compared < source * 0.3:
                        failures.add('coverage')
                    if matched < compared * 0.9:
                        failures.add('pixel_residual')
                assert failures, (qid, lane_index, region_index)
            reason = ','.join(sorted(failures))
            reasons[reason] += 1
            details.append({'lane': lane_index, 'region': region_index,
                            'reasons': sorted(failures), 'domains': region['domains'],
                            'counts': counts, 'fit_failure': region['fit_failure']})
    rows.append({'query_id': qid,
                 'gradient_model_present': any(lane is not None for lane in lanes),
                 'gradient_correspondences': [e['gradient_correspondences'], e['interpolated_correspondences']],
                 'regions': details})
assert len(rows) == 229 - audit['whole_or_region_supported_pairs'] == 83
assert sum(row['gradient_model_present'] for row in rows) == 32
assert all(path.read_bytes() == data for path, data in raw.items())
result = {'status': 'observed_native_refusal_partition',
          'input_hashes': {str(path): digest(data) for path, data in raw.items()},
          'unconfirmed_pairs': len(rows), 'no_gradient_model_pairs': 51,
          'gradient_model_without_support_pairs': 32,
          'region_refusal_counts': dict(sorted(reasons.items())), 'rows': rows,
          'scope': 'Pinned prior native measurement. Region counts overlap and are not area unions. Model presence does not prove geometry correctness. No threshold change or current-version accuracy claim.'}
a.output.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({k: v for k, v in result.items() if k != 'rows'}, indent=2))
