"""Audit native managed union / fresh compound file parity to qualified diagnostic."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();report_hash=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='verified_fresh_collection_file_parity'
assert type(r['returncode']) is int and r['returncode']==0 and all(h(k)==v for k,v in r['input_hashes'].items())
baseline_path=next(Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-fresh-candidate-union-200301.json');baseline=json.loads(baseline_path.read_text());assert baseline['status']=='verified_fresh_native_diagnostic_parity' and baseline['returncode']==0 and all(h(k)==v for k,v in baseline['input_hashes'].items())
baseline_audit_path=Path('docs/research/dedup-fresh-candidate-union-200301-audit.json');audit=json.loads(baseline_audit_path.read_text());assert audit['status']=='verified_fresh_union_file_parity' and all(h(k)==v for k,v in audit['input_hashes'].items())
canonical=lambda v:json.dumps(v,sort_keys=True,allow_nan=False);e=r['evidence'];prior=baseline['evidence'];assert canonical(json.loads(r['stdout']))==canonical(e) and canonical(json.loads(baseline['stdout']))==canonical(prior)
keys=['status','correspondences','matrix','inliers','regions','region_support_count','managed_used','retained_before_drop'];assert canonical({k:e[k] for k in keys})==canonical({k:prior[k] for k in keys})
assert e['status']=='ok'
for k in ['managed_used','managed_peak','region_support_count']:assert type(e[k]) is int and e[k]>=0
assert e['managed_used']==0 and e['managed_peak']<=512*1024*1024 and e['region_support_count']==47
assert type(e['retained_before_drop']) is int and 0<e['retained_before_drop']<=e['managed_peak']
points=e['correspondences'];assert len(points)==17 and len(e['inliers'])==10
assert all(len(v)==2 and all(len(x)==2 and all(type(y) in [int,float] and math.isfinite(y) for y in x) for x in v) for v in points)
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');helper_hash=h(helper);source=helper.read_text();nodes={v.name:v for v in ast.parse(source).body if isinstance(v,ast.FunctionDef)};ns={'math':math}
exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns)
region_source=ast.get_source_segment(source,nodes['verify_regions']);assert 'len(regions)<=32' in region_source;exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,'200301.jpg')
for region in e['regions']:
 for (x,y,w,height),(width,limit_height) in zip(region['domains'],[(2048,1536),(692,349)]):assert x+w<=width and y+height<=limit_height
assert h(a.report)==report_hash and h(helper)==helper_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_fresh_union_collection_parity','matches':17,'inliers':10,'region_support_count':47,'managed_peak':e['managed_peak'],'retained_before_drop':e.get('retained_before_drop'),'input_hashes':{str(a.report):report_hash,str(helper):helper_hash,str(baseline_audit_path):h(baseline_audit_path),str(Path(__file__)):h(__file__)},'scope':'Actual native source/report pins, typed memory lifecycle, exact qualified diagnostic correspondence/model/region parity, residuals and domain/count arithmetic. Two-source known positive collection; no whole decision, negative precision, full corpus collection or complete decoder/geometry allocation/RSS bound.'},indent=2)+'\n')
