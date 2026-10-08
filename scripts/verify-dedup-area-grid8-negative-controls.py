"""Verify two same-recipe original-resolution different-origin controls."""
import argparse,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
r=json.loads(a.report.read_text());assert r['status']=='complete' and len(r['results'])==2
assert all(h(k)==v for k,v in r['input_hashes'].items())
manifest=next(Path(k) for k in r['input_hashes'] if Path(k).name=='prepared.json');m=json.loads(manifest.read_text())
q=next(v for v in m['images']['strong'] if v['filename']=='200101.jpg');assert r['query_group']==q['group_id']
qpath=next(Path(k) for k in r['input_hashes'] if Path(k).name=='200101.jpg');assert h(qpath)==q['source_sha256']
assert [v['original'] for v in r['results']]==['200200.jpg','200300.jpg']
for row in r['results']:
 origin=next(v for v in m['images']['original'] if v['filename']==row['original']);assert row['original_group']==origin['group_id']!=q['group_id']
 source=next(Path(k) for k in r['input_hashes'] if Path(k).name==row['original']);assert h(source)==origin['source_sha256']
 assert type(row['returncode']) is int and row['returncode']==0
 e=row['evidence'];assert json.dumps(json.loads(row['stdout']),sort_keys=True,allow_nan=False)==json.dumps(e,sort_keys=True,allow_nan=False)
 assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
 assert type(e['whole_candidate']) is bool and not e['whole_candidate']
 assert type(e['region_support_count']) is int and e['region_support_count']==0
 # These two controls both refuse before a geometric model; no pixel-negative claim.
 assert e['matrix'] is None and not e['inliers'] and not e['regions']
 assert all(len(point)==2 and all(len(x)==2 and all(type(y) in [int,float] and math.isfinite(y) for y in x) for x in point) for point in e['correspondences'])
 assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=512*1024*1024
 assert type(e['retained_before_drop']) is int and 0<=e['retained_before_drop']<=e['managed_peak']
assert all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_two_different_origin_controls','pairs':2,'input_hashes':{str(a.report):h(a.report),str(Path(__file__)):h(__file__)},'scope':'Same frozen area/grid8/radius7 recipe, original bytes and different publisher-origin labels, typed output and memory credits. Both reject before geometry; no broad pixel, semantic or burst precision qualification.'},indent=2)+'\n')
