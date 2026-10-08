"""Reconstruct candidate union and audit native original-pixel regional evidence."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();report_hash=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='complete_native' and type(r['returncode']) is int and r['returncode']==0
assert all(h(k)==v for k,v in r['input_hashes'].items())
reports=[next(Path(k) for k in r['input_hashes'] if Path(k).name==n) for n in ['dedup-original-lowcontrast-200301.json','dedup-original-smooth-candidates-200301.json']]
values=[]
for path in reports:
 value=json.loads(path.read_text());assert value['status']=='complete_native' and value['returncode']==0 and all(h(k)==v for k,v in value['input_hashes'].items());assert json.loads(value['stdout'])==value['evidence'];values.append(value)
for n in ['dedup-original-lowcontrast-200301-audit.json','dedup-original-smooth-candidates-200301-audit.json']:
 audit=json.loads(next(Path(k) for k in r['input_hashes'] if Path(k).name==n).read_text());assert all(h(k)==v for k,v in audit['input_hashes'].items())
# Independent nested coordinate flattening, deterministic lexicographic ordering,
# and rejection of nearby source OR target positions. No score-weighted voting.
unique=sorted({(v[0][0],v[0][1],v[1][0],v[1][1]) for value in values for v in value['evidence']['correspondences']})
selected=[]
for row in unique:
 if all(math.hypot(row[0]-v[0],row[1]-v[1])>2 and math.hypot(row[2]-v[2],row[3]-v[3])>2 for v in selected):selected.append(row)
assert type(r['unique_union_matches']) is int and r['unique_union_matches']==len(unique)
assert type(r['selected_matches']) is int and r['selected_matches']==len(selected)
assert r['source_recipe_matches']==[len(v['evidence']['correspondences']) for v in values]
points_file=next(Path(k) for k in r['input_hashes'] if Path(k).suffix=='.tsv');assert [tuple(map(float,line.split())) for line in points_file.read_text().splitlines()]==selected
canonical=lambda v:json.dumps(v,sort_keys=True,allow_nan=False);e=r['evidence'];assert canonical(json.loads(r['stdout']))==canonical(e)
assert e['status']=='ok'
for k in ['managed_used','managed_peak','region_support_count']:assert type(e[k]) is int and e[k]>=0
assert e['managed_used']==0 and e['managed_peak']<=512*1024*1024 and e['region_support_count']<=128
assert e['correspondences']==[[list(v[:2]),list(v[2:])] for v in selected]
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');helper_hash=h(helper);source=helper.read_text();nodes={v.name:v for v in ast.parse(source).body if isinstance(v,ast.FunctionDef)};ns={'math':math}
exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns)
region_source=ast.get_source_segment(source,nodes['verify_regions']);assert 'len(regions)<=32' in region_source;exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
if e['matrix'] is None:assert not e['inliers'] and not e['regions'] and e['region_support_count']==0
else:
 assert len(e['inliers'])>=10
 ns['verify_model'](e['matrix'],e['inliers'],e['correspondences']);ns['verify_regions'](e,'200301.jpg')
for region in e['regions']:
 for (x,y,w,height),(width,limit_height) in zip(region['domains'],[(2048,1536),(692,349)]):assert x+w<=width and y+height<=limit_height
assert h(a.report)==report_hash and h(helper)==helper_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_candidate_union_diagnostic','matches':len(selected),'inliers':len(e['inliers']),'region_support_count':e['region_support_count'],'input_hashes':{str(a.report):report_hash,str(helper):helper_hash,str(Path(__file__)):h(__file__)},'scope':'Pinned prior native candidate evidence; independently reconstructed exact union and distinct locations, TSV/native parity, geometry residuals and original-domain regional arithmetic. Single known positive diagnostic; no whole decision, collection integration, negative precision or complete managed allocation bound.'},indent=2)+'\n')
