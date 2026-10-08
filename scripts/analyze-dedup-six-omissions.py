#!/usr/bin/env python3
"""Locate observed rejection stages in an independently audited fixed prefix."""
import argparse
import collections
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('report', 'audit', 'output'):
    p.add_argument(name, type=Path)
a = p.parse_args()
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
r = json.loads(a.report.read_text())
audit = json.loads(a.audit.read_text())
assert audit['report_sha256'] == digest(a.report)
assert audit['status'] in ('verified_six_region_collection_prefix', 'verified_six_region_collection')
assert audit['verified_queries'] == len(r['results'])
found = set(audit['whole_or_region_supported_queries'])
assert found <= {row['query_id'] for row in r['results']}
omissions = []
for row in r['results']:
    if row['query_id'] in found:
        continue
    e = row['native_collection_evidence']
    spatial = e['spatial_gradient']
    regions = (e.get('spatial_gradient_regions') or {}).get('regions', [])
    assert not spatial['candidate'] and not any(v['accepted_region'] for v in regions)
    stage = ('spatial_model_without_pixel_support' if spatial['geometry'] is not None
             else 'no_spatial_model')
    omissions.append({
        'query_id': row['query_id'], 'observed_stage': stage,
        'legacy_gradient_correspondences': e['correspondence_counts'],
        'spatial_correspondences': spatial['correspondences'],
        'spatial_inliers': spatial['inliers'],
        'spatial_geometry': spatial['geometry'],
        'spatial_whole_pixels': spatial.get('pixels'),
        'spatial_regions_tested': len(regions),
        'spatial_region_failures': dict(collections.Counter(
            v['fit_failure'] if v['counts'] is None else 'pixel_policy_rejected'
            for v in regions)),
    })
a.output.write_text(json.dumps({
    'status': 'analyzed_audited_prefix',
    'pairs': len(r['results']), 'required_pairs': len(r['required_queries']),
    'supported': len(found), 'omitted': len(omissions),
    'observed_stages': dict(collections.Counter(v['observed_stage'] for v in omissions)),
    'omissions': omissions,
    'input_hashes': {str(path): digest(path) for path in (a.report, a.audit, Path(__file__))},
    'scope': 'Observed spatial-lane rejection stages only; no inferred ground-truth geometry, threshold relaxation, or new recall measurement.',
}, indent=2) + '\n')
