"""Independently verify all pinned tree blobs and complete-module controls."""
from pathlib import Path
import json,hashlib
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/trace72-20261016';D=R/'docs/research/dedup-500';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def emit(p,d):assert not p.exists();p.write_text(json.dumps(d,indent=2)+'\n')
g=json.loads((W/'github-commit.json').read_text());t=json.loads((D/'evidence/ultralytics--source-trace/tree.json').read_text());assert g['tree']['sha']==t['sha'] and not t['truncated'];entries=t['tree'];blobs=[]
def obj(k,b):return hashlib.sha1(k.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
for e in entries:
 if e['type']!='blob':continue
 p=W/'source'/e['path'];b=p.read_bytes();assert len(b)==e['size'] and obj('blob',b)==e['sha'];blobs.append({**e,'sha256':h(p),'text_content':b.decode(),'symlink_stored_inert':e['mode']=='120000'})
expected={'':t['sha'],**{e['path']:e['sha'] for e in entries if e['type']=='tree'}}
for dirname,want in expected.items():
 children=[e for e in entries if ('' if str(Path(e['path']).parent)=='.' else str(Path(e['path']).parent))==dirname];children.sort(key=lambda e:(Path(e['path']).name+('/' if e['type']=='tree' else '')).encode())
 b=b''.join(('40000' if e['type']=='tree' else e['mode']).encode()+b' '+Path(e['path']).name.encode()+b'\0'+bytes.fromhex(e['sha']) for e in children);assert obj('tree',b)==want
assert len(blobs)==15 and len(expected)==6
p=D/'trace72-native-controls-20261016.json';d=json.loads(p.read_text());assert h(R/'scripts/qualify-trace72-native-controls-20261016.py')==d['producer_sha256']
for key in ['source_pins','loaded_dependency_file_pins']:
 for name,want in d[key].items():assert h(R/name)==want,name
assert len(d['runs'])==2 and len(d['runs'][0])==19 and d['runs'][0]==d['runs'][1]
for n in range(2):
 a=W/f'native-controls-v2/run{n}';assert (a/'clone/x.py').read_text()=='old' and (a/'origin/x.py').read_text()=='new';assert (a/'poison').is_dir() and not (a/'poison/x.py').exists();assert (a/'files/alias.py').resolve()==(a/'owned-outside.txt').resolve()
a=D/'trace72-fetched-source-pins-20261016.json';emit(a,{'repository':'ultralytics/source-trace','commit':g['sha'],'canonical_trees':expected,'blobs':blobs,'limits':['All15 pinned blobs UTF8 text including inert symlinktarget. No tracked binary media. External README logos, social icons, contributor banner and dynamic badges are links outside pinned source tree, not reviewed here.']})
r={'status':'verified_complete_pinned_source_and_38_native_controls','whole_repo_complete':True,'whole_implementation_complete':True,'repository':'ultralytics/source-trace','commit':g['sha'],'blobs':15,'canonical_trees':6,'acquisition':str(a.relative_to(R)),'acquisition_sha256':h(a),'auditor_sha256':h(Path(__file__)),'controls':str(p.relative_to(R)),'controls_sha256':h(p),'media_scope':'No tracked binary assets/videos. External marketing logos/social icons/contributor banner and dynamic badges not fetched; no claim of reviewing linked external sites or repositories.','license':'Complete AGPL3 LICENSE read; no production code copied.','upstream_tests_run':False,'native_scope':d['scope'],'findings':d['findings'],'decision':'Reject line-copy statistics as exact/image identity. Reference explicit pair-vs-line count contracts and snapshot freshness; no production adoption or speed claim.','limits':d['limits']+['Complete pinned source review excludes thirdpartyGitPython/dependency-source and linked external marketing assets.']}
emit(D/'trace72-complete-source-review-20261016.json',r);print(json.dumps({'status':'verified','blobs':15,'trees':6,'native_controls':38}))
