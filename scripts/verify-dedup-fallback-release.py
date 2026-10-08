"""Strict optimized fallback parity against independently audited debug evidence."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)}
r=json.loads(a.report.read_text());assert r['status']=='complete_native_release_parity' or a.checkpoint and r['status']=='running_pairs';assert type(r['required_pairs']) is int and r['required_pairs']==3 and 0<len(r['results'])<=3
if r['status']=='complete_native_release_parity' or not a.checkpoint:assert len(r['results'])==3
assert all(h(p)==v for p,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
positive_path=pinned('dedup-fallback-known-pairs-retry.json');negative_path=pinned('dedup-fallback-original-negatives-201303-checkpoint-1-screen-retry.json');positive=json.loads(positive_path.read_text());negative=json.loads(negative_path.read_text());assert positive['status']=='complete' and len(positive['results'])==2 and len(negative['results'])==1
for doc in [positive,negative]:assert all(h(p)==v for p,v in doc['input_hashes'].items())
for name,status,source in [('dedup-fallback-known-pairs-retry-audit.json','verified_native_fallback_known_pairs',positive_path),('dedup-fallback-original-negatives-201303-checkpoint-1-screen-retry-audit.json','verified_fallback_negative_prefix',negative_path)]:
 audit=json.loads(pinned(name).read_text());assert audit['status']==status and audit['input_hashes'][str(source)]==h(source) and all(h(p)==v for p,v in audit['input_hashes'].items())
expected=[(row['original'],row['query'],row['evidence']) for row in positive['results']]+[(negative['results'][0]['original'],negative['query'],negative['results'][0]['evidence'])]
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');pins[str(helper)]=h(helper);nodes={v.name:v for v in ast.parse(helper.read_text()).body if isinstance(v,ast.FunctionDef)};ns={'math':math}
exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns);region_source=ast.get_source_segment(helper.read_text(),nodes['verify_regions']);assert 'len(regions)<=32' in region_source;exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
canonical=lambda v:json.dumps(v,sort_keys=True,allow_nan=False);keys=['correspondences','matrix','inliers','regions','region_support_count','retained_before_drop','managed_used','smoothing_radii','attempted_recipes'];summary=[]
for row,(left,right,baseline) in zip(r['results'],expected):
 assert row['original']==left and row['query']==right and type(row['returncode']) is int and row['returncode']==0
 argv=row['argv'];assert len(argv)==4 and argv[1]=='original-managed-candidate-union-fallback' and Path(argv[2]).name==left and Path(argv[3]).name==right
 assert all(p in r['input_hashes'] for p in [argv[0],*argv[2:]])
 assert Path(argv[0]).name=='gradient-scales-probe-fallback-release'
 e=row['evidence'];assert e['status']=='ok' and canonical(json.loads(row['stdout']))==canonical(e)
 assert canonical({k:e[k] for k in keys})==canonical({k:baseline[k] for k in keys})
 for k in ['region_support_count','managed_used','managed_peak','retained_before_drop','attempted_recipes']:assert type(e[k]) is int and e[k]>=0
 assert e['managed_used']==0 and e['retained_before_drop']<=e['managed_peak']<=512*1024*1024
 assert 1<=e['attempted_recipes']<=3 and e['smoothing_radii']==[[2,2],[4,0],[0,4]][e['attempted_recipes']-1] and all(type(v) is int for v in e['smoothing_radii'])
 assert type(row['elapsed_seconds']) in [int,float] and math.isfinite(row['elapsed_seconds']) and row['elapsed_seconds']>=0
 if e['matrix'] is None:assert not e['inliers'] and not e['regions'] and e['region_support_count']==0
 else:assert len(e['inliers'])>=10;ns['verify_model'](e['matrix'],e['inliers'],e['correspondences']);ns['verify_regions'](e,right)
 summary.append({'original':left,'query':right,'support_count':e['region_support_count'],'attempted_recipes':e['attempted_recipes']})
assert all(h(p)==v for p,v in pins.items()) and all(h(p)==v for p,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_fallback_release_prefix' if a.checkpoint else 'verified_fallback_release_parity','verified_pairs':len(summary),'required_pairs':3,'summary':summary,'input_hashes':pins,'scope':'Exact independently audited debug/release canonical evidence, typed resources/metadata/argv and geometry/regional arithmetic. Two positives and one negative; no full-corpus, collection or controlled performance proof.'},indent=2)+'\n')
