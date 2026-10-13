"""Verify all forty-six recorded tile attempts and their complete candidate union.

Independent bounds, residuals, partitions, full-baseline parity and native
original-pixel controls. No learned inference rerun or broad precision claim.
"""
import hashlib
import json
import math
from pathlib import Path

import numpy as np

D=Path(__file__).resolve().parents[1]/'docs/research'
SOURCE=D/'dedup-loftr-both-tiles-20261010.json'
OUT=D/'dedup-loftr-both-tiles-audit-20261010.json'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert not OUT.exists()
r=json.loads(SOURCE.read_text())
assert r['status']=='complete_46_attempt_proposal_diagnostic' and len(r['rows'])==46
assert all(sha(p)==h for p,h in r['pins'].items())


def geometry(points,record,tolerance):
    indices=[]
    if record['matrix'] is not None:
        matrix=np.asarray(record['matrix']);inverse=np.linalg.inv(matrix)
        def project(m,p):
            with np.errstate(all='ignore'):
                v=m@np.array([*p,1.]);return v[:2]/v[2]
        for i,p in enumerate(points):
            if np.linalg.norm(project(matrix,p[:2])-p[2:])<=2. and np.linalg.norm(project(inverse,p[2:])-p[:2])<=tolerance:
                indices.append(i)
    assert indices==record['bidirectional_inlier_indices']


assert len({(tuple(row['source_rectangle']),tuple(row['target_rectangle']),row['clahe']) for row in r['rows'] if row['case']=='both-tiles'})==40
old=json.loads((D/'dedup-low-texture-crumpled-loftr-diagnostic.json').read_text())
counts=[]
for i,row in enumerate(r['rows']):
    assert row['status']=='complete_proposals_only'
    points=row['distinct_points'];scores=row['confidence']
    assert len(points)==len(scores)<=row['raw_matches']<=5000
    assert scores==sorted(scores,reverse=True)
    for index,(point,score) in enumerate(zip(points,scores)):
        assert all(math.isfinite(value) for value in point) and math.isfinite(score) and 0<=score<=1
        for p,rectangle,full in [(point[:2],row['source_rectangle'],row['source_dimensions']),(point[2:],row['target_rectangle'],row['target_dimensions'])]:
            assert 0<=p[0]<full[0] and 0<=p[1]<full[1]
            assert rectangle[0]-.5<=p[0]<=rectangle[0]+rectangle[2]-.5 and rectangle[1]-.5<=p[1]<=rectangle[1]+rectangle[3]-.5
        assert all(math.dist(point[:2],previous[:2])>2 and math.dist(point[2:],previous[2:])>2 for previous in points[:index])
    tolerance=2*math.hypot(*row['source_dimensions'])/math.hypot(*row['target_dimensions'])
    assert row['source_tolerance']==tolerance
    geometry(points,row['geometry'],tolerance)
    if row['case']=='baseline':
        previous=next(x for x in old['rows'] if x['max_side']==640 and x['clahe']==row['clahe'])
        assert points==[a+b for a,b in previous['distinct_points']]
        assert scores==previous['confidence']
    counts.append(len(row['geometry']['bidirectional_inlier_indices']))
raw=[(score,point) for row in r['rows'] if row['case'] in ('baseline','both-tiles') for score,point in zip(row['confidence'],row['distinct_points'])]
raw.sort(key=lambda value:-value[0])
points=[];scores=[]
for score,point in raw:
    if all(math.dist(point[:2],previous[:2])>2 and math.dist(point[2:],previous[2:])>2 for previous in points):points.append(point);scores.append(score)
union=r['crumpled_union']
assert points==union['distinct_points'] and scores==union['confidence']
geometry(points,union['geometry'],5.12)
for region in union['regions']:
    grid=region['grid'];x,y=region['cell']
    indices=[i for i,p in enumerate(points) if x*800/grid<=p[2]<(x+1)*800/grid and y*600/grid<=p[3]<(y+1)*600/grid]
    assert indices==region['proposal_indices']
    geometry([points[i] for i in indices],region,5.12)
native_path=D/'dedup-loftr-both-tiles-native-controls-20261010.json'
native=json.loads(native_path.read_text())
assert all(sha(p)==h for p,h in native['pins'].items())
selected=[i for i,row in enumerate(r['rows']) if row['case']=='both-tiles' and counts[i]>=10]
assert selected==[row['row'] for row in native['results']]==[16,20]
for row in native['results']:
    assert row['returncode']==0 and all(sha(p)==h for p,h in row['files'].items())
    e=json.loads(row['stdout']);assert e['status']=='ok' and e['supported'] and min(e['anchors'])>=10
    assert 'memory_used=0' in row['stderr']
OUT.write_text(json.dumps({'status':'verified_two_crumpled_tile_original_pixel_controls','scope':__doc__,
    'attempts':46,'tiled_attempts':40,'maximum_tile_inliers':max(counts[6:]),'native_supported_rows':selected,'union_points':len(points),'union_inliers':len(union['geometry']['bidirectional_inlier_indices']),
    'automatic_tiled_runtime_qualified':False,'tiled_foreign_precision_qualified':False,
    'pins':{str(p):sha(p) for p in [SOURCE,Path(__file__),native_path,D/'dedup-low-texture-crumpled-loftr-diagnostic.json']}},indent=2)+'\n')
print('verified',selected,'maximum',max(counts[6:]),'union',len(points))
