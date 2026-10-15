"""Audit partial upstream source and actual original selected tests/numeric controls."""
from pathlib import Path
import json,hashlib,xml.etree.ElementTree as ET
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/deduplipy99-20261016';D=R/'docs/research/dedup-500';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def emit(p,d):assert not p.exists();p.write_text(json.dumps(d,indent=2)+'\n')
def obj(k,b):return hashlib.sha1(k.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
g=json.loads((W/'github-commit.json').read_text());t=json.loads((D/'evidence/fritshermans--deduplipy/tree.json').read_text());assert g['tree']['sha']==t['sha'] and not t['truncated'];entries=t['tree'];blobs=[]
read={'setup.py','deduplipy/__init__.py','deduplipy/config.py','deduplipy/clustering/__init__.py','deduplipy/clustering/clustering.py','deduplipy/clustering/fill_missing_edges.py','tests/test_clustering/test_clustering.py','tests/test_clustering/test_fill_missing_edges.py','tests/test_clustering/clustering_fixture.csv'}
for e in entries:
 if e['type']!='blob':continue
 p=W/'source'/e['path'];b=p.read_bytes();assert len(b)==e['size'] and obj('blob',b)==e['sha'];row={**e,'sha256':h(p),'fully_read_this_turn':e['path'] in read}
 if e['path'] in read:row['text_content']=b.decode()
 blobs.append(row)
expected={'':t['sha'],**{e['path']:e['sha'] for e in entries if e['type']=='tree'}}
for dirname,want in expected.items():
 children=[e for e in entries if ('' if str(Path(e['path']).parent)=='.' else str(Path(e['path']).parent))==dirname];children.sort(key=lambda e:(Path(e['path']).name+('/' if e['type']=='tree' else '')).encode());b=b''.join(('40000' if e['type']=='tree' else e['mode']).encode()+b' '+Path(e['path']).name.encode()+b'\0'+bytes.fromhex(e['sha']) for e in children);assert obj('tree',b)==want
assert len(blobs)==62 and sum(x['fully_read_this_turn'] for x in blobs)==9
p=D/'deduplipy99-clustering-upstream-tests-20261016.json';tests=json.loads(p.read_text());assert h(R/'scripts/qualify-deduplipy99-clustering-tests-20261016.py')==tests['producer_sha256']
for run in tests['runs']:
 assert len(run['cases'])==5 and run['exit_code']==0 and all(not x['failed'] and not x['error'] for x in run['cases']);assert h(R/run['log'])==run['log_sha256'] and h(R/run['junit'])==run['junit_sha256'];xml=ET.parse(R/run['junit']).getroot();assert len(list(xml.iter('testcase')))==5 and not list(xml.iter('failure')) and not list(xml.iter('error'));assert '5 passed' in (R/run['log']).read_text()
for name,want in tests['source_pins'].items():assert h(R/name)==want
q=D/'deduplipy99-clustering-controls-20261016.json';controls=json.loads(q.read_text());assert h(R/'scripts/qualify-deduplipy99-clustering-controls-20261016.py')==controls['producer_sha256'] and len(controls['runs'])==2 and len(controls['runs'][0])==9 and controls['runs'][0]==controls['runs'][1]
for key in ['source_pins','loaded_dependency_file_pins']:
 for name,want in controls[key].items():assert h(Path(name) if Path(name).is_absolute() else R/name)==want
acq=D/'deduplipy99-fetched-source-pins-20261016.json';emit(acq,{'repository':'fritshermans/deduplipy','commit':g['sha'],'canonical_trees':expected,'blobs':blobs,'scope':'62blobs acquired andbyteverified;9selectedsource/test/config/setup blobs fullyread. Othermodules/docs/LICENSE/notebooks/CSV/XLSX/logo notyetreviewed. Acquisition isnotfull-source ormedia review.'})
emit(D/'deduplipy99-clustering-audit-20261016.json',{'status':'verified_partial_source_original_clustering_tests_and_native_boundaries','whole_repo_complete':False,'whole_implementation_complete':False,'repository':'fritshermans/deduplipy','commit':g['sha'],'acquisition':str(acq.relative_to(R)),'acquisition_sha256':h(acq),'auditor_sha256':h(Path(__file__)),'upstream_tests':str(p.relative_to(R)),'upstream_tests_sha256':h(p),'controls':str(q.relative_to(R)),'controls_sha256':h(q),'source_blobs_fully_read':9,'source_blobs_total':62,'selected_original_tests_passed':10,'additional_native_controls':18,'dependency_versions':controls['dependency_versions'],'scope':controls['scope'],'findings':controls['findings'],'limits':controls['limits']+['Fiveoriginalclusteringtests twice passed inclseed1234original500x500case; notallupstreamtests orwholelibrary/nativeformat/platform/quality campaign.','Other53blobs/media/notebooks/data/LICENSE notyetreviewed; no fullsource countincrease or codeadoption.']});print(json.dumps({'status':'verified','source_read':9,'source_total':62,'upstream_test_passes':10,'native_controls':18,'full_source':False}))
