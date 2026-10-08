"""Independent finite debug/release evidence parity, not speed qualification."""
import argparse,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();report_hash=h(a.report);r=json.loads(a.report.read_text())
assert r['status']=='verified_native_release_parity' or a.checkpoint and r['status']=='running'
assert type(r['required_pairs']) is int and r['required_pairs']==4 and len(r['results'])<=4
if r['status']=='verified_native_release_parity' or not a.checkpoint:assert len(r['results'])==4
assert all(h(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
third=json.loads(pinned('dedup-fresh-candidate-union-200301.json').read_text());first=json.loads(pinned('dedup-fresh-union-original-full-strict-checkpoint-1.json').read_text());negative=json.loads(pinned('dedup-fresh-union-negative-controls.json').read_text())
assert third['status']=='verified_fresh_native_diagnostic_parity' and first['status']=='running_pairs' and len(first['results'])==1 and negative['status']=='complete'
for doc in [third,first,negative]:assert all(h(k)==v for k,v in doc['input_hashes'].items())
cases=[('200300.jpg','200301.jpg',third['evidence'],third),('200000.jpg','200001.jpg',first['results'][0]['evidence'],first),*[(v['original'],'200301.jpg',v['evidence'],negative) for v in negative['results']]];assert len(cases)==4
keys=['status','correspondences','matrix','inliers','regions','region_support_count','retained_before_drop','managed_used'];canonical=lambda e:json.dumps(e,sort_keys=True,allow_nan=False)
for row,(left,right,reference,doc) in zip(r['results'],cases):
 assert row['original']==left and row['query']==right and type(row['returncode']) is int and row['returncode']==0
 for name in [left,right]:assert h(pinned(name)) in [v for k,v in doc['input_hashes'].items() if Path(k).name==name]
 e=row['evidence'];assert canonical(json.loads(row['stdout']))==canonical(e) and canonical({k:e[k] for k in keys})==canonical({k:reference[k] for k in keys})
 for key in ['managed_used','managed_peak','retained_before_drop','region_support_count']:assert type(e[key]) is int and e[key]>=0
 assert e['managed_used']==0 and e['retained_before_drop']<=e['managed_peak']<=512*1024*1024
 assert type(row['elapsed_seconds']) in [int,float] and math.isfinite(row['elapsed_seconds']) and row['elapsed_seconds']>0
assert h(a.report)==report_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_native_release_prefix' if a.checkpoint and r['status']=='running' else 'verified_native_release_parity','verified_pairs':len(r['results']),'required_pairs':4,'input_hashes':{str(a.report):report_hash,str(Path(__file__)):h(__file__)},'scope':'Source/reference pins, typed resources, exact canonical debug/release evidence and retained credit for two real positives and two negatives. Finite parity; no full release recall, independent pixel resampling or controlled speed claim.'},indent=2)+'\n')
