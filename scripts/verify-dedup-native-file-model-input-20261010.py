"""Provenance, independent proposal selection/residuals, native pixel predicates.

Not an independent decoder/pixel oracle or total inference memory qualification.
"""
import hashlib
import json
import math
from pathlib import Path
import re
import numpy as np

ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
REPORT=D/'dedup-native-file-model-input-20261010.json'
OUT=D/'dedup-native-file-model-input-20261010-audit.json'
assert not OUT.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
r=json.loads(REPORT.read_text())
assert r['status']=='complete_native_file_model_input_diagnostic' and len(r['results'])==2
assert all(sha(p)==h for p,h in r['pins'].items())
m=json.loads((D/'dedup-model-input-snapshot-20261010.json').read_text())
assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
verified=[]
for row in r['results']:
    assert row['side'] in (320,640) and len(row['preparation'])==2
    for p in row['preparation']:
        assert p['returncode']==0
        e=p['evidence'];assert e==json.loads(p['stdout'])
        assert e['memory_used']==0 and 0<e['memory_peak']<=512*1024*1024
        assert all(d%8==0 and 8<=d<=row['side'] for d in e['dimensions'])
    raw=row['inference']['evidence'];g=row['geometry']['evidence']
    assert row['inference']['returncode']==row['geometry']['returncode']==0
    assert g['status']=='geometry' and g['hypotheses']==20000
    assert len(raw['confidence'])<=5000
    arrays=[]
    for i,key in enumerate(('keypoints0','keypoints1')):
        e=row['preparation'][i]['evidence']
        arrays.append((np.array(raw[key])+.5)*np.array(e['original'])/np.array(e['dimensions'])-.5)
    selected=[]
    for j in np.argsort(-np.array(raw['confidence']),kind='stable'):
        p,q=arrays[0][j],arrays[1][j]
        if not all(np.all(v>=0) and np.all(v<row['preparation'][i]['evidence']['original']) for i,v in enumerate((p,q))):continue
        if all(np.sum((p-x[0])**2)>4 and np.sum((q-x[1])**2)>4 for x in selected):selected.append([p.tolist(),q.tolist()])
    assert np.array(selected).shape==np.array(g['points']).shape
    assert np.allclose(selected,g['points'],rtol=0,atol=2e-12)
    matrix=np.array(g['matrix']);inverse=np.linalg.inv(matrix)
    def apply(h,p):
        v=h@np.array([*p,1.]);return v[:2]/v[2]
    for h,prep in zip((matrix,inverse),row['preparation']):
        w,height=prep['evidence']['original']
        denominators=[(h@np.array([x,y,1.]))[2] for x,y in ((0,0),(w-1,0),(0,height-1),(w-1,height-1))]
        assert all(v>0 for v in denominators) or all(v<0 for v in denominators)
    forward=[];bidirectional=[]
    tolerance=2*math.hypot(*row['preparation'][0]['evidence']['original'])/math.hypot(*row['preparation'][1]['evidence']['original'])
    for i,(p,q) in enumerate(g['points']):
        if np.sum((apply(matrix,p)-q)**2)<=4:
            forward.append(i)
            if np.sum((apply(inverse,q)-p)**2)<=tolerance*tolerance:bidirectional.append(i)
    assert forward==g['inliers']
    peak=re.fullmatch(r'proposal_memory_used=0 proposal_memory_peak=(\d+)\n',row['geometry']['stderr'])
    assert peak and int(peak[1])==len(raw['confidence'])*40 and int(peak[1])<=256*1024
    confirmation=row['confirmation'];assert confirmation['returncode']==0
    e=confirmation['evidence'];assert e==json.loads(confirmation['stdout'])
    assert e['status']=='ok' and e['supported'] is True and min(e['anchors'])>=10
    assert e['anchors']==[len(bidirectional)]*2
    assert 'memory_used=0 ' in confirmation['stderr']
    for n,direction in zip(e['anchors'],e['directions']):
        assert direction['sites']==25*n and direction['valid_sites']>=direction['sites']*.3
        assert direction['informative_pairs']>=1000 and direction['agreeing_pairs']>=direction['informative_pairs']*.9
    verified.append(dict(side=row['side'],raw_matches=len(raw['confidence']),selected_points=len(selected),forward_inliers=len(forward),bidirectional_inliers=len(bidirectional),selection_payload_peak=int(peak[1])))
OUT.write_text(json.dumps(dict(status='verified_native_file_model_input_geometry_and_pixel_predicates',pins={str(REPORT):sha(REPORT),str(Path(__file__)):sha(__file__)},snapshot_files_verified=len(m['file_hashes']),variants=verified,scope=__doc__),indent=2)+'\n')
print(verified)
