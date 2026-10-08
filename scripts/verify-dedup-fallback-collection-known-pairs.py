"""Strict two-case fallback parity; neither count nor process success is sufficient."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)}
r=json.loads(a.report.read_text());assert (r['status']=='complete' or a.checkpoint and r['status']=='running_pairs') and type(r['required_pairs']) is int and r['required_pairs']==2
assert 0<len(r['results'])<=2
if r['status']=='complete' or not a.checkpoint:assert len(r['results'])==2
assert all(h(p)==digest for p,digest in r['input_hashes'].items())
reference_paths=[Path('docs/research/dedup-combined-domain-release.json'),Path('docs/research/dedup-201303-asymmetric4.json')]
references=[json.loads(p.read_text()) for p in reference_paths]
for path,ref in zip(reference_paths,references):
 pins[str(path)]=h(path);assert all(h(p)==digest for p,digest in ref['input_hashes'].items())
refs=[references[0]['results'][0]['evidence'],references[1]['evidence']]
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');pins[str(helper)]=h(helper);nodes={n.name:n for n in ast.parse(helper.read_text()).body if isinstance(n,ast.FunctionDef)};ns={'math':math}
exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns)
region_source=ast.get_source_segment(helper.read_text(),nodes['verify_regions']);assert 'len(regions)<=32' in region_source
exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
keys=['correspondences','matrix','inliers','regions','region_support_count','retained_before_drop','managed_used'];canonical=lambda x:json.dumps(x,sort_keys=True,allow_nan=False)
summary=[]
for row,reference,(left,right,attempts,radii,support) in zip(r['results'],refs,[('200300.jpg','200301.jpg',1,[2,2],47),('201300.jpg','201303.jpg',2,[4,0],46)]):
 assert row['original']==left and row['query']==right and type(row['returncode']) is int and row['returncode']==0
 e=row['evidence'];assert canonical(json.loads(row['stdout']))==canonical(e) and e['status']=='ok'
 assert type(e['attempted_recipes']) is int and e['attempted_recipes']==attempts and e['smoothing_radii']==radii and all(type(v) is int for v in e['smoothing_radii'])
 assert canonical({k:e[k] for k in keys})==canonical({k:reference[k] for k in keys})
 for k in ['managed_used','managed_peak','retained_before_drop','region_support_count']:assert type(e[k]) is int and e[k]>=0
 assert e['managed_used']==0 and e['retained_before_drop']<=e['managed_peak']<=512*1024*1024 and e['region_support_count']==support
 assert len(e['inliers'])>=10;ns['verify_model'](e['matrix'],e['inliers'],e['correspondences']);ns['verify_regions'](e,right)
 argv=row['argv'];assert len(argv)==4 and argv[1]=='original-managed-candidate-union-fallback-collection' and Path(argv[2]).name==left and Path(argv[3]).name==right
 assert all(v in r['input_hashes'] for v in [argv[0],argv[2],argv[3]])
 refdoc=references[0] if attempts==1 else references[1]
 assert all(r['input_hashes'][v]==refdoc['input_hashes'][v] for v in argv[2:])
 summary.append({'query':right,'attempted_recipes':attempts,'smoothing_radii':radii,'support_count':support})
assert all(h(p)==digest for p,digest in pins.items()) and all(h(p)==digest for p,digest in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_native_fallback_collection_prefix' if a.checkpoint else 'verified_native_fallback_collection_known_pairs','required_pairs':2,'verified_pairs':len(summary),'summary':summary,'input_hashes':pins,'scope':'Exact canonical geometry/pixel evidence parity with pinned symmetric200301 and asymmetric201303; typed resources and stdout. Two indexed positives only: retrieval necessarily retained each accepted pair in the native diagnostic; no all-corpus or negative precision, broad collection or independent pixel oracle.'},indent=2)+'\n')
