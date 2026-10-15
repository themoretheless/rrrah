"""Verify full upstream tree and original test/native owned-filesystem evidence."""
from pathlib import Path
import json,hashlib,struct
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/vscode80-20261016';D=R/'docs/research/dedup-500';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def emit(p,d):assert not p.exists();p.write_text(json.dumps(d,indent=2)+'\n')
def obj(k,b):return hashlib.sha1(k.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
g=json.loads((W/'github-commit.json').read_text());t=json.loads((D/'evidence/mrmlnc--vscode-duplicate/tree.json').read_text());assert g['tree']['sha']==t['sha'] and not t['truncated'];entries=t['tree'];blobs=[]
for e in entries:
 if e['type']!='blob':continue
 p=W/'source'/e['path'];b=p.read_bytes();assert len(b)==e['size'] and obj('blob',b)==e['sha'];row={**e,'sha256':h(p)}
 if p.suffix=='.png':assert b[:8]==b'\x89PNG\r\n\x1a\n';row['dimensions']=list(struct.unpack('>II',b[16:24]))
 else:row['text_content']=b.decode()
 blobs.append(row)
expected={'':t['sha'],**{e['path']:e['sha'] for e in entries if e['type']=='tree'}}
for dirname,want in expected.items():
 children=[e for e in entries if ('' if str(Path(e['path']).parent)=='.' else str(Path(e['path']).parent))==dirname];children.sort(key=lambda e:(Path(e['path']).name+('/' if e['type']=='tree' else '')).encode());b=b''.join(('40000' if e['type']=='tree' else e['mode']).encode()+b' '+Path(e['path']).name.encode()+b'\0'+bytes.fromhex(e['sha']) for e in children);assert obj('tree',b)==want
assert len(blobs)==21 and len(expected)==7
p=D/'vscode80-native-controls-20261016.json';d=json.loads(p.read_text());assert h(R/'scripts/qualify-vscode80-native-controls-20261016.cjs')==d['producer_sha256']
for key in ['source_pins','loaded_dependency_file_pins']:
 for name,want in d[key].items():assert h(R/name)==want
assert len(d['runs'])==2 and d['runs'][0]==d['runs'][1]
for n,run in enumerate(d['runs']):
 assert len(run['controls'])==14 and run['upstream']['result']=={'failures':0,'tests':8,'passes':8};raw=json.loads((W/f'native-controls/upstream-run{n}.json').read_text());assert raw==run['upstream']
 a=W/f'native-controls/run{n}';assert (a/'existing.txt').read_text()=='SOURCE';assert (a/'dest-dir/new').read_text()=='NEW' and (a/'dest-dir/old').read_text()=='OLD';assert (a/'open-fail.txt').read_text()=='SOURCE';assert (a/'block').read_text()=='BLOCK';assert (a/'absolute-looking.txt').read_text()=='SOURCE'
acq=D/'vscode80-fetched-source-pins-20261016.json';emit(acq,{'repository':'mrmlnc/vscode-duplicate','commit':g['sha'],'canonical_trees':expected,'blobs':blobs,'native_log':{'path':str((W/'native-run.log').relative_to(R)),'sha256':h(W/'native-run.log')}})
emit(D/'vscode80-complete-source-review-20261016.json',{'status':'verified_complete_pinned_source_icon_and44_native_tests_controls','whole_repo_complete':True,'whole_implementation_complete':True,'repository':'mrmlnc/vscode-duplicate','commit':g['sha'],'blobs':21,'canonical_trees':7,'acquisition':str(acq.relative_to(R)),'acquisition_sha256':h(acq),'auditor_sha256':h(Path(__file__)),'controls':str(p.relative_to(R)),'controls_sha256':h(p),'media_scope':'Only trackedbinary icon.png128x129 viewedfull. No tracked screenshots/videos; READMEexternaldonationbadge notfetched.','license':'MIT LICENSE fully read; no production code copied.','upstream_tests_run':True,'upstream_test_scope':'All8originalMocha pathspec tests twice usingwholeTStranspileModule/actualMocha5.0.5 andnativepath, Statsadapter asoriginaltest. No npmtest fullstrictbuild/Node7/VSCodehost qualification.','native_scope':d['scope'],'findings':d['findings'],'decision':'Reject file-copy command as duplicate detector. Retain confirmation, partial-success/error-boundary and directorymerge semantics lessons; no adoption orspeedclaim.','limits':d['limits']});print(json.dumps({'blobs':21,'trees':7,'original_tests':16,'additional_controls':28,'status':'verified'}))
