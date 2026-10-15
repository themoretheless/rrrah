"""Independently verify pinned source tree and owned native filesystem outcomes."""
from pathlib import Path
import json,hashlib,struct
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/sidebar55-20261016';D=R/'docs/research/dedup-500';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def emit(p,d):assert not p.exists();p.write_text(json.dumps(d,indent=2)+'\n')
def obj(k,b):return hashlib.sha1(k.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
g=json.loads((W/'github-commit.json').read_text());t=json.loads((D/'evidence/braver--SideBarTools/tree.json').read_text());assert g['tree']['sha']==t['sha'] and not t['truncated'];entries=t['tree'];blobs=[]
for e in entries:
 if e['type']!='blob':continue
 p=W/'source'/e['path'];b=p.read_bytes();assert len(b)==e['size'] and obj('blob',b)==e['sha'];row={**e,'sha256':h(p)}
 if p.suffix=='.png':assert b[:8]==b'\x89PNG\r\n\x1a\n';row['dimensions']=list(struct.unpack('>II',b[16:24]))
 else:row['text_content']=b.decode()
 blobs.append(row)
expected={'':t['sha'],**{e['path']:e['sha'] for e in entries if e['type']=='tree'}}
for dirname,want in expected.items():
 children=[e for e in entries if ('' if str(Path(e['path']).parent)=='.' else str(Path(e['path']).parent))==dirname];children.sort(key=lambda e:(Path(e['path']).name+('/' if e['type']=='tree' else '')).encode());b=b''.join(('40000' if e['type']=='tree' else e['mode']).encode()+b' '+Path(e['path']).name.encode()+b'\0'+bytes.fromhex(e['sha']) for e in children);assert obj('tree',b)==want
assert len(blobs)==13 and len(expected)==3
p=D/'sidebar55-native-controls-20261016.json';d=json.loads(p.read_text());assert h(R/'scripts/qualify-sidebar55-native-controls-20261016.py')==d['producer_sha256'];assert len(d['runs'])==2 and len(d['runs'][0])==18 and d['runs'][0]==d['runs'][1]
for name,want in d['source_pins'].items():assert h(R/name)==want
for n in range(2):
 a=W/f'native-controls/run{n}';assert (a/'destination.txt').read_text()=='SOURCE';assert (a/'move-target.txt').read_text()=='MOVED' and not (a/'move-file.txt').exists();assert (a/'container/move-dir/child.txt').read_text()=='DIR' and not (a/'container/child.txt').exists();assert (a/'existing/retain.txt').read_text()=='RETAIN';assert (a/'new.txt').read_text()=='KEEP';assert (a/'link-copy.txt').read_text()=='SOURCE' and not (a/'link-copy.txt').is_symlink()
a=D/'sidebar55-fetched-source-pins-20261016.json';emit(a,{'repository':'braver/SideBarTools','commit':g['sha'],'canonical_trees':expected,'blobs':blobs})
emit(D/'sidebar55-complete-source-review-20261016.json',{'status':'verified_complete_pinned_source_screenshot_and36_selected_native_controls','whole_repo_complete':True,'whole_implementation_complete':True,'repository':'braver/SideBarTools','commit':g['sha'],'blobs':13,'canonical_trees':3,'acquisition':str(a.relative_to(R)),'acquisition_sha256':h(a),'auditor_sha256':h(Path(__file__)),'controls':str(p.relative_to(R)),'controls_sha256':h(p),'media_scope':'Whole tracked screenshot.png viewed1158x810; README links same screenshot. External donation badge not fetched; no other tracked binary assets/videos.','license':'MIT LICENSE fully read; no production code copied.','upstream_tests_run':False,'native_scope':d['scope'],'findings':d['findings'],'decision':'Reject as duplicate detector: file operations wrapper and external compare launcher. Retain destination/overwrite and directory-retarget boundary lessons only; no adoption or speed claim.','limits':d['limits']});print(json.dumps({'blobs':13,'trees':3,'native_controls':36,'status':'verified'}))
