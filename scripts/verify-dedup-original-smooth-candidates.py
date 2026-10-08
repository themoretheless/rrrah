"""Audit the candidate-only smoothing experiment without assuming recovery."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();report_hash=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='complete_native'
assert type(r['returncode']) is int and r['returncode']==0 and r['query']=='200301.jpg'
assert r['evidence']['candidate_smoothing_radius']==2 and len(r['evidence']['features'])==2 and all(type(v) is int and 0<=v<=14000 for v in r['evidence']['features'])
assert all(h(k)==v for k,v in r['input_hashes'].items())
manifest=next(Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-original-resolution-hard-crops.json');m=json.loads(manifest.read_text());assert m['status']=='verified_selected_archive_bytes';assert all(h(k)==v for k,v in m['input_hashes'].items())
for n in ['200300.jpg','200301.jpg']:
 k=next(k for k in r['input_hashes'] if Path(k).name==n);assert r['input_hashes'][k]==m['input_hashes'][k]
e=r['evidence'];canonical=lambda v:json.dumps(v,sort_keys=True,allow_nan=False);assert canonical(json.loads(r['stdout']))==canonical(e)
assert e['status']=='ok'
for k in ['managed_used','managed_peak','region_support_count']:assert type(e[k]) is int and e[k]>=0
assert e['managed_used']==0 and e['managed_peak']<=512*1024*1024 and e['region_support_count']<=128
points=e['correspondences'];assert all(len(v)==2 and all(len(x)==2 and all(type(y) in [int,float] and math.isfinite(y) for y in x) for x in v) for v in points)
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');helper_hash=h(helper);source=helper.read_text();nodes={v.name:v for v in ast.parse(source).body if isinstance(v,ast.FunctionDef)};ns={'math':math}
exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns)
region_source=ast.get_source_segment(source,nodes['verify_regions']);assert 'len(regions)<=32' in region_source;exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
if e['matrix'] is None:assert not e['inliers'] and not e['regions'] and e['region_support_count']==0
else:ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,'200301.jpg')
dim={v['name']:v['dimensions'] for v in m['records']}
for region in e['regions']:
 for (x,y,w,height),(width,limit_height) in zip(region['domains'],[dim['200300.jpg'],dim['200301.jpg']]):assert x+w<=width and y+height<=limit_height
baseline_path=next(Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-original-area-regions.json');baseline=json.loads(baseline_path.read_text());assert baseline['status']=='complete' and all(h(k)==v for k,v in baseline['input_hashes'].items());prior=next(v for v in baseline['results'] if v['query_id']=='200301.jpg');assert prior['returncode']==0 and canonical(json.loads(prior['stdout']))==canonical(prior['evidence'])
assert h(a.report)==report_hash and h(helper)==helper_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_candidate_only_smoothing_experiment','matches':len(points),'inliers':len(e['inliers']),'features':e['features'],'region_support_count':e['region_support_count'],'prior_area_result':prior,'input_hashes':{str(a.report):report_hash,str(helper):helper_hash,str(Path(__file__)):h(__file__)},'scope':'Single known publisher-origin noisy crop; candidate-only smoothing; plain matcher/geometry allocations excluded; no whole decision; pins, native/raw parity, typed resource fields, model residuals, regional arithmetic and domain bounds. No independent pixel resampling oracle, negative precision or default promotion.'},indent=2)+'\n')
