"""Verify whole pinned source, structured dependency metadata and native boundary evidence."""
from pathlib import Path
import json,hashlib,tomllib,re
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/semantic97-20261016';D=R/'docs/research/dedup-500';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def emit(p,d):assert not p.exists();p.write_text(json.dumps(d,indent=2)+'\n')
def obj(k,b):return hashlib.sha1(k.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
g=json.loads((W/'github-commit.json').read_text());t=json.loads((D/'evidence/gkamradt--SemanticDeduplicator/tree.json').read_text());assert g['tree']['sha']==t['sha'] and not t['truncated'];entries=t['tree'];blobs=[]
for e in entries:
 if e['type']!='blob':continue
 p=W/'source'/e['path'];b=p.read_bytes();assert len(b)==e['size'] and obj('blob',b)==e['sha'];blobs.append({**e,'sha256':h(p),'text_content':b.decode()})
expected={'':t['sha'],**{e['path']:e['sha'] for e in entries if e['type']=='tree'}}
for dirname,want in expected.items():
 children=[e for e in entries if ('' if str(Path(e['path']).parent)=='.' else str(Path(e['path']).parent))==dirname];children.sort(key=lambda e:(Path(e['path']).name+('/' if e['type']=='tree' else '')).encode());b=b''.join(('40000' if e['type']=='tree' else e['mode']).encode()+b' '+Path(e['path']).name.encode()+b'\0'+bytes.fromhex(e['sha']) for e in children);assert obj('tree',b)==want
assert len(blobs)==9 and len(expected)==4
lock=tomllib.loads((W/'source/poetry.lock').read_text());names={x['name'] for x in lock['package']};files=0
for package in lock['package']:
 assert all(n.lower().replace('_','-') in names for n in package.get('dependencies',{}))
 for f in package['files']:assert re.fullmatch('sha256:[0-9a-f]{64}',f['hash']);files+=1
assert len(names)==23 and files==445
p=D/'semantic97-native-controls-20261016.json';d=json.loads(p.read_text());assert h(R/'scripts/qualify-semantic97-native-controls-20261016.py')==d['producer_sha256'];assert len(d['runs'])==2 and len(d['runs'][0])==18 and d['runs'][0]==d['runs'][1]
for key in ['source_pins','loaded_numpy_file_pins']:
 for name,want in d[key].items():assert h(Path(name) if Path(name).is_absolute() else R/name)==want
acq=D/'semantic97-fetched-source-pins-20261016.json';emit(acq,{'repository':'gkamradt/SemanticDeduplicator','commit':g['sha'],'canonical_trees':expected,'blobs':blobs,'structured_lock_review':{'metadata':lock['metadata'],'packages':lock['package'],'distribution_hashes_syntax_checked':445,'dependency_graph_checked':True,'dependency_sources_reviewed':False,'original_environment_installed':False},'readme_image':{'url':'https://i.imgur.com/2RUXX6g.png','path':str((W/'readme-banner.png').relative_to(R)),'sha256':h(W/'readme-banner.png'),'viewed':True}})
emit(D/'semantic97-complete-source-review-20261016.json',{'status':'verified_complete_pinned_source_lock_metadata_banner_and36_boundary_controls','whole_repo_complete':True,'whole_implementation_complete':True,'repository':'gkamradt/SemanticDeduplicator','commit':g['sha'],'blobs':9,'canonical_trees':4,'acquisition':str(acq.relative_to(R)),'acquisition_sha256':h(acq),'auditor_sha256':h(Path(__file__)),'controls':str(p.relative_to(R)),'controls_sha256':h(p),'media_scope':'No tracked binary assets. ExternalREADME algorithmexample banner viewedfull1616x706. Exampleusesadd_items absentfromcurrentAPI; staticdoc mismatch. No videos.','license':'MIT LICENSE fully read; no production code copied.','lock_scope':'23package records/versions/dependencies/extras and445distributionhash syntax structurally reviewed; not dependency-source/artifactdownload/Poetrycontenthash or originalenv verification.','upstream_tests_run':False,'upstream_test_scope':'All7originaltests fullyread; require actualmodel/API semantics andcredentials, so notexecuted or replaced withfake semantic claims.','native_scope':d['scope'],'findings':d['findings']+['Cosine prefilter scores everyexisting item thensequentialremoteLLM perthresholdcandidate, no budget/cancel/batch/index. Static complexity/resource observation, notperformance measurement.'],'decision':'Reference two-stage candidate verification concept, reject textLLM semantic consolidation as image/file identity. Preserve provenance/objectidentity and atomicembedding update lessons; no adoption/speedclaim.','limits':d['limits']});print(json.dumps({'status':'verified','blobs':9,'trees':4,'controls':36,'lock_packages':23,'distribution_hashes':445}))
