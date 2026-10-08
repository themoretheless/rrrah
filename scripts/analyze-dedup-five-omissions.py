#!/usr/bin/env python3
"""Describe observed missing-copy stages without inferring the root cause."""
import argparse
import collections
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('report', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
raw = a.report.read_bytes()
r = json.loads(raw)
assert r['mode'] == 'complementary_gradient_portfolio'
assert r['completed_pairs'] == len(r['results']) == 229
assert all(row['status'] == 'ok' for row in r['results'])
partitions = collections.defaultdict(list)
rows = []
for row in r['results']:
    e = row['evidence']
    if e['candidate']:
        continue
    geometry = [e[key] for key in ('registration_geometry', 'pyramid_geometry', 'gradient_geometry', 'interpolated_geometry')]
    pixels = [e[key] for key in ('registration_counts', 'pyramid_counts', 'secondary_counts', 'gradient_counts', 'interpolated_counts')]
    stage = 'pixel_evidence_present_but_all_searches_reject' if any(v is not None for v in pixels) else 'geometry_present_without_pixel_evidence' if any(v is not None for v in geometry) else 'no_reported_geometry'
    partitions[stage].append(row['query_id'])
    rows.append({'query_id': row['query_id'], 'stage': stage,
                 'binary_correspondences': e['correspondence_counts'],
                 'fixed_gradient_correspondences': e['gradient_correspondences'],
                 'interpolated_gradient_correspondences': e['interpolated_correspondences']})
assert len(rows) == 229 - r['candidates'] == 192
assert a.report.read_bytes() == raw
out = {'report_sha256': hashlib.sha256(raw).hexdigest(), 'probe_sha256': r['probe_sha256'],
       'required_pairs': 229, 'omitted_pairs': len(rows),
       'counts': {k: len(v) for k, v in sorted(partitions.items())},
       'partitions': dict(sorted(partitions.items())), 'rows': rows,
       'scope': 'Observed stages in pinned native five-search report. Absence of geometry does not distinguish extraction, matching or model-estimation cause. Publisher-origin copy labels, not exact pixels. No threshold relaxation justified.'}
a.output.write_text(json.dumps(out, indent=2) + '\n')
print(json.dumps(out['counts'], indent=2))
