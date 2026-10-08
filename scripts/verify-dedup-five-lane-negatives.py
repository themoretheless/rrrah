#!/usr/bin/env python3
"""Independently audit finished queries in a running five-search negative suite."""
import argparse
import hashlib
import json
from pathlib import Path

p=argparse.ArgumentParser()
for name in ('manifest','probe','suite','output'):
    p.add_argument(name,type=Path)
a=p.parse_args()
digest=lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
payload=a.suite.read_bytes()
s=json.loads(payload)
m=json.loads(a.manifest.read_bytes())
assert s['manifest_sha256']==digest(a.manifest)
assert s['probe_sha256']==digest(a.probe)
queries=('200001.jpg','201501.jpg','202701.jpg','206801.jpg','207201.jpg',
         '209901.jpg','207302.jpg','208102.jpg','214501.jpg')
assert s['required_pairs']==1404
assert 0<len(s['queries'])<=len(queries)
assert s['completed_pairs']==156*len(s['queries'])
if len(s['queries'])==len(queries):
    assert s['status']=='complete_nine_query_measurement_not_full_library_qualification'
verified=[]
for entry,query in zip(s['queries'],queries):
    assert entry['query_id']==query and entry['pairs']==156 and entry['candidates']==0
    report=a.suite.parent/('dedup-five-lane-negative-'+query.removesuffix('.jpg')+'.json')
    raw=report.read_bytes()
    assert hashlib.sha256(raw).hexdigest()==entry['report_sha256']
    r=json.loads(raw)
    assert r['mode']=='complementary_gradient_portfolio'
    assert r['manifest_sha256']==s['manifest_sha256'] and r['probe_sha256']==s['probe_sha256']
    assert r['completed_pairs']==r['required_pairs']==len(r['results'])==156
    assert r['status_counts']=={'ok':156} and r['candidates']==0
    assert r['status']=='complete_measurement_not_full_library_qualification'
    image=next(v['right'] for v in m['positive_pairs'] if v['query_id']==query)
    originals=[v for v in m['images']['original'] if v['group_id']!=image['group_id']]
    assert len(originals)==156
    old=a.suite.parent/('dedup-four-lane-negative-'+query.removesuffix('.jpg')+'.json')
    previous=json.loads(old.read_bytes()) if old.exists() else None
    if previous:
        assert previous['completed_pairs']==len(previous['results'])==156
        assert previous['mode']=='complementary_gradient'
        assert previous['manifest_sha256']==s['manifest_sha256']
        assert previous['probe_sha256']==digest(a.probe.parent/'photo-probe-complementary-gradient')
    interp_path=a.suite.parent/('dedup-gradient-interpolated-negative-'+query+'.json')
    interp=json.loads(interp_path.read_bytes()) if interp_path.exists() else None
    if interp:
        assert interp['mode']=='gradient_pyramid_public_files_interpolated'
        assert interp['manifest_sha256']==s['manifest_sha256']
        assert interp['probe_sha256']==digest(a.probe.parent/'photo-probe-gradient-interpolated-file')
        assert interp['completed_pairs']==len(interp['results'])==156
    for i,(row,original) in enumerate(zip(r['results'],originals)):
        assert row['query_id']==query+'/'+original['filename']
        assert row['label']=='different_publisher_origin'
        assert row['status']=='ok' and row['returncode']==0
        e=row['evidence']
        assert e['status']=='ok' and e['managed_used']==0
        assert type(e['managed_peak']) is int and 0<e['managed_peak']<=64_000_000
        assert e['candidate'] is False and e['four_candidate'] is False
        for field,count in [('five_accepted_searches',5),('four_accepted_searches',4),('joined_accepted_searches',3)]:
            assert len(e[field])==count and all(value is False for value in e[field])
        for prefix in ('gradient','interpolated'):
            assert e[prefix+'_candidate'] is False
            counts=e[prefix+'_counts']
            if counts is not None:
                assert len(counts)>0
                accepted=True
                for matched,compared,source in counts:
                    assert all(type(value) is int for value in (matched,compared,source))
                    assert 0<=matched<=compared<=source and source>0
                    accepted &= compared>=1000 and compared>=source*.3 and matched>=compared*.9
                assert not accepted
        if previous:
            before=previous['results'][i]
            assert before['query_id']==row['query_id'] and before['status']=='ok'
            for key,value in before['evidence'].items():
                if key!='managed_peak':assert e[key]==value,(query,i,key)
        if interp:
            ir=interp['results'][i]
            assert ir['query_id']==row['query_id'] and ir['status']=='ok' and ir['returncode']==0
            ge=ir['evidence']
            for key in ('candidate','correspondences','inliers','geometry'):
                assert e['interpolated_'+key]==ge[key],(query,i,key)
            pixels=ge['pixels']
            assert e['interpolated_counts']==(pixels['counts'] if pixels and pixels['status']=='ok' else None)
        for value in (image,original):
            assert digest(Path(value['normalized_path']))==value['normalized_sha256']
    assert report.read_bytes()==raw
    verified.append({'query_id':query,'pairs':156,'four_reference_parity':bool(previous),'interpolated_reference_parity':bool(interp),
                     'report_sha256':entry['report_sha256']})
assert a.suite.read_bytes()==payload,'Suite changed during audit; retry snapshot.'
out={'verified_pairs':156*len(verified),'required_pairs':1404,'queries':verified,
     'complete':len(verified)==len(queries),
     'suite_sha256':hashlib.sha256(payload).hexdigest(),
     'scope':'Finished-query native decisions, numeric rejections, source integrity and available four-search parity. Not all-query semantic/burst qualification.'}
a.output.write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({'verified_pairs':out['verified_pairs'],'required_pairs':1404},indent=2))
