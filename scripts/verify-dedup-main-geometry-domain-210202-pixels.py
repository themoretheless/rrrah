"""Verify guarded fixed-model confirmation predicates; no independent pixel oracle."""
import ast,hashlib,json,math
from pathlib import Path
b=Path('docs/research');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pin=b/'dedup-main-geometry-domain-210202-pixels-pins.json';pins=json.loads(pin.read_text());assert all(h(k)==v for k,v in pins.items())
p=b/'dedup-main-geometry-domain-210202-pixels.jsonl';lines=p.read_text().splitlines();assert len(lines)==1;r=json.loads(lines[0]);assert r['status']=='ok' and r['managed_used']==0 and r['managed_peak']<=512*1024*1024 and len(r['experiments'])==1
e=r['experiments'][0];assert 'error' not in e
values=list(map(float,(b/'dedup-main-geometry-domain-210202-model.txt').read_text().split()));assert len(values)==13 and e['fit_target_domain']==values[:4] and e['matrix']==[values[4+i*3:7+i*3] for i in range(3)]
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');s=helper.read_text();nodes={v.name:v for v in ast.parse(s).body if isinstance(v,ast.FunctionDef)};ns={'math':math};exec(compile(ast.Module(body=[nodes['verify_regions']],type_ignores=[]),str(helper),'exec'),ns);ns['verify_regions'](e,'210202.jpg')
assert e['local_supported_count']==sum(v['accepted'] for v in e['regions'])==e['region_support_count']
for v in e['regions']:
 for rect,(w,height) in zip(v['domains'],[(2048,1536),(800,600)]):assert rect[0]+rect[2]<=w and rect[1]+rect[3]<=height
supported=[v for v in e['regions'] if v['pixels'] is not None and all(c>=1000 and c/t>=.3 for m,c,t in v['pixels']['counts'])]
best=max((min(m/c for m,c,t in v['pixels']['counts']) for v in supported),default=None)
(b/'dedup-main-geometry-domain-210202-pixels-audit.json').write_text(json.dumps({'status':'verified_fixed_domain_geometry_pixel_predicates','terminal_exit_code':0,'regions':len(e['regions']),'supported_regions':e['region_support_count'],'best_support_eligible_minimum_fraction':best,'pins':{**pins,str(pin):h(pin),str(p):h(p),str(helper):h(helper),str(Path(__file__)):h(__file__)},'scope':'One supplied independently residual/domain-sign checked11-inlier geometry. Native file confirmation, source hashes/managed bound and region predicate math; not independent resampling, broad negatives or copy recognition.'},indent=2)+'\n');print('supported',e['region_support_count'],'best',best)
