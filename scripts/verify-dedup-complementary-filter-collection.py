#!/usr/bin/env python3
"""Check indexed Copydays proposals against all fixed constituent decisions."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
p=argparse.ArgumentParser()
for name in ['manifest','probe','report','base','filters']:
    p.add_argument(name,type=Path)
p.add_argument('--checkpoint',action='store_true')
a=p.parse_args()
m=json.loads(a.manifest.read_text());r=json.loads(a.report.read_text())
base=json.loads(a.base.read_text());filters=json.loads(a.filters.read_text())
assert r['mode']=='complementary_filter_collection'
assert r['manifest_sha256']==base['manifest_sha256']==filters['manifest_sha256']==hashlib.sha256(a.manifest.read_bytes()).hexdigest()
assert r['probe_sha256']==hashlib.sha256(a.probe.read_bytes()).hexdigest()
assert base['mode']=='complementary_file' and filters['mode']=='pyramid_filter_portfolio'
for ref in [base,filters]:
    assert ref['completed_pairs']==len(ref['results'])==229
    assert all(x['status']=='ok' for x in ref['results'])
assert r['required_pairs']==229 and r['completed_pairs']==len(r['results'])
assert 0<len(r['results'])<=229
if not a.checkpoint:
    assert len(r['results'])==229 and r['status']=='complete_measurement_not_full_library_qualification'
assert r['status_counts']==dict(collections.Counter(x['status'] for x in r['results']))
b={x['query_id']:x['evidence'] for x in base['results']}
f={x['query_id']:x['evidence'] for x in filters['results']}
keys=['accepted_searches','registration_accepted_lanes','registration_counts','pyramid_counts','registration_geometry','pyramid_geometry','correspondence_counts','inlier_counts','fit_failure']
images=set();retrieved=0
for row,pair in zip(r['results'],m['positive_pairs']):
    assert row['query_id']==pair['query_id'] and row['label']==pair['label']
    assert row['returncode']==0 and row['status']=='ok'
    e=row['evidence'];old=b[row['query_id']];fp=f[row['query_id']]
    assert isinstance(e['candidate'],bool)
    assert e['candidate']==(old['candidate'] or fp['candidate'])
    assert type(e['retrieved_pairs']) is int and e['retrieved_pairs'] in [0,1]
    assert e['managed_used']==0 and 0<=e['managed_peak']<=64*1024*1024
    if e['retrieved_pairs']:
        retrieved+=1
        for key in keys:assert e[key]==old[key],(row['query_id'],key)
        assert e['base_candidate']==old['candidate']
        assert e['pyramid_geometry']==fp['geometry']
        assert e['secondary_counts']==fp['secondary_counts']
        assert e['secondary_fit_failure']==fp['secondary_fit_failure']
        assert e['joined_accepted_searches']==old['accepted_searches']+[fp['accepted_filters'][1]]
        assert e['candidate']==any(e['joined_accepted_searches'])
    else:
        assert e['candidate'] is False
    for side in ['left','right']:
        v=pair[side]
        if v['normalized_path'] not in images:
            assert hashlib.sha256(Path(v['normalized_path']).read_bytes()).hexdigest()==v['normalized_sha256']
            images.add(v['normalized_path'])
assert r['candidates']==sum(x['evidence']['candidate'] for x in r['results'])
print(json.dumps({'status':'verified_checkpoint' if a.checkpoint else 'verified_full_finite_measurement','completed_pairs':len(r['results']),'accepted':r['candidates'],'retrieved_pairs':retrieved,'normalized_images_verified':len(images),'scope':'Candidate decisions equal the full fixed constituent union; all retrieved pairs retain native geometry/counts/refusals. Nonretrieved negatives contain no geometry certificate. Finite229 collection coverage only; not semantic/burst precision or full library.'},indent=2))
