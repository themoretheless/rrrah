"""Audit both omitted original-resolution collection crops without assuming recovery."""
import argparse, ast, hashlib, json, math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
report_hash=h(a.report);r=json.loads(a.report.read_text())
assert r['status']=='complete' or (a.checkpoint and r['status']=='running')
assert all(h(k)==v for k,v in r['input_hashes'].items())
expected=['200101.jpg','200301.jpg']
assert 0<len(r['results'])<=2
assert [v['query_id'] for v in r['results']]==expected[:len(r['results'])]
if not a.checkpoint:assert len(r['results'])==2
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');helper_hash=h(helper)
tree=ast.parse(helper.read_text());names={'verify_model','verify_regions'}
nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names];assert {n.name for n in nodes}==names
ns={'math':math};exec(compile(ast.Module(body=nodes,type_ignores=[]),str(helper),'exec'),ns)
baselines=[Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-original-resolution-three-crops.json'];assert len(baselines)==1
baseline=json.loads(baselines[0].read_text());assert baseline['status']=='complete';assert all(h(k)==v for k,v in baseline['input_hashes'].items())
canonical=lambda x:json.dumps(x,sort_keys=True,allow_nan=False)
summary=[]
for row in r['results']:
 assert type(row['returncode']) is int and row['returncode']==0
 e=row['evidence'];assert canonical(json.loads(row['stdout']))==canonical(e)
 assert e['status']=='ok' and type(e['whole_candidate']) is bool
 assert type(e['managed_used']) is int and e['managed_used']==0
 for key in ['managed_peak','retained_before_drop','region_support_count']:
  assert type(e[key]) is int and e[key]>=0
 assert e['retained_before_drop']<=e['managed_peak']<=512*1024*1024
 points=e['correspondences'];assert all(len(p)==2 and all(len(x)==2 and all(type(y) in [int,float] and math.isfinite(y) for y in x) for x in p) for p in points)
 if e['matrix'] is None:assert not e['inliers'] and not e['regions'] and e['region_support_count']==0 and not e['whole_candidate']
 else:ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,row['query_id'])
 refrow=next(v for v in baseline['results'] if v['query_id']==row['query_id']);assert canonical(json.loads(refrow['stdout']))==canonical(refrow['evidence'])
 ref=refrow['evidence']['stats'][1]['distinct'];keys=['correspondences','matrix','inliers','regions','region_support_count']
 assert canonical({k:e[k] for k in keys})==canonical({k:ref[k] for k in keys})
 assert row['exact_diagnostic_parity'] is True
 summary.append({'query':row['query_id'],'matches':len(points),'inliers':len(e['inliers']),'whole_candidate':e['whole_candidate'],'region_support_count':e['region_support_count'],'managed_peak':e['managed_peak']})
assert h(a.report)==report_hash and h(helper)==helper_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_managed_remaining_crops' if r['status']=='complete' else 'verified_managed_remaining_crop_prefix','summary':summary,'input_hashes':{str(a.report):report_hash,str(helper):helper_hash,str(Path(__file__)):h(__file__)},'scope':'Ordered known source-resolution crops, all input pins, typed raw-output/diagnostic parity, model residuals and regional acceptance arithmetic. Reports misses explicitly; no broad recall, negative precision or complete-allocation qualification.'},indent=2)+'\n')
