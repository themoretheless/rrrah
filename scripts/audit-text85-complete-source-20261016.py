"""Full pinned source audit and independent retained native whole-CLI/finisher evidence."""
from pathlib import Path
import json,hashlib,tomllib
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/text85-20261016';D=R/'docs/research/dedup-500';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def emit(p,d):assert not p.exists();p.write_text(json.dumps(d,indent=2)+'\n')
def obj(k,b):return hashlib.sha1(k.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
g=json.loads((W/'github-commit.json').read_text());t=json.loads((D/'evidence/google-research--deduplicate-text-datasets/tree.json').read_text());assert g['tree']['sha']==t['sha'] and not t['truncated'];entries=t['tree'];blobs=[]
for e in entries:
 if e['type']!='blob':continue
 p=W/'source'/e['path'];b=p.read_bytes();assert len(b)==e['size'] and obj('blob',b)==e['sha'];blobs.append({**e,'sha256':h(p),'text_content':b.decode()})
expected={'':t['sha'],**{e['path']:e['sha'] for e in entries if e['type']=='tree'}}
for dirname,want in expected.items():
 children=[e for e in entries if ('' if str(Path(e['path']).parent)=='.' else str(Path(e['path']).parent))==dirname];children.sort(key=lambda e:(Path(e['path']).name+('/' if e['type']=='tree' else '')).encode());b=b''.join(('40000' if e['type']=='tree' else e['mode']).encode()+b' '+Path(e['path']).name.encode()+b'\0'+bytes.fromhex(e['sha']) for e in children);assert obj('tree',b)==want
assert len(blobs)==15 and len(expected)==3
reports=[]
for label,producer in [('whole-cli','qualify-text85-whole-cli-controls-20261016.py'),('finish','qualify-text85-finish-controls-20261016.py')]:
 p=D/f'text85-{label}-controls-20261016.json';d=json.loads(p.read_text());assert h(R/'scripts'/producer)==d['producer_sha256'];assert len(d['runs'])==2 and len(d['runs'][0])==(14 if label=='whole-cli' else 8)
 for name,want in d['artifact_pins'].items():assert h(R/name)==want
 if label=='whole-cli':
  assert h(R/d['binary'])==d['binary_sha256'];assert h(Path(d['resolved_cargo_lock']['path']))==d['resolved_cargo_lock']['sha256'];assert h(R/d['build_log']['path'])==d['build_log']['sha256'];assert 'Finished `dev` profile' in (R/d['build_log']['path']).read_text()
  for n,rows in enumerate(d['runs']):
   a=W/f'whole-cli-controls/run{n}'
   for i,row in enumerate(rows):assert h(a/f'{i}.stdout')==row['stdout_sha256'] and h(a/f'{i}.stderr')==row['stderr_sha256']
   assert 'Duplicates found: 4' in (a/'8.stdout').read_text();assert 'Duplicates found: 0' in (a/'9.stdout').read_text();assert 'Reached EOF badly' in (a/'12.stderr').read_text();assert (a/'banana.table.bin').read_bytes()==bytes([5,3,1,0,4,2]);assert (a/'merged').read_bytes()==b'banana'
 else:
  assert d['runs'][0]==d['runs'][1];assert d['runs'][0][4]['exit_code']!=0 and d['runs'][0][4]['result_hex']==b'012345'.hex();assert d['runs'][0][-1]['result_hex']==''
 reports.append({'path':str(p.relative_to(R)),'sha256':h(p),'scope':d['scope'],'findings':d['findings'],'limits':d['limits']})
p=D/'text85-suffix-controls-audit-20261016.json';old=json.loads(p.read_text());assert h(R/old['controls'])==old['controls_sha256'];reports.append({'path':str(p.relative_to(R)),'sha256':h(p),'scope':old['read_scope'],'total_queries':old['total_queries']})
a=D/'text85-complete-source-pins-20261016.json';emit(a,{'repository':'google-research/deduplicate-text-datasets','commit':g['sha'],'canonical_trees':expected,'blobs':blobs,'media_scope':'All15trackedblobsUTF8text; READMEhasnoembeddedimage/video links. Externalpaper/algorithmreferences and13GB/1.4GB/26MBdatasetdownloads notpartpinnedsource andnotdownloaded.'})
findings=[f for r in reports for f in r.get('findings',[])]+['Static main: is_present_memory rejectsfinalsuffixindex andfind_index_memory computeslow-1 atfirstindex; notseparatelyruntimequalified here.','Static merge: HACKSIZE100000 overlap assumption andone sharedwidth fromfirstpart; noalllength-longrepeats/exactorderingguarantee. Threadpartition andshort-part bounds needstrongerqualification.','Static wrappers: shared/tmp/fin/cache/drop paths, shellinterpolatedunquotedpaths, fixedtmp/out cleanup/retrywithoutbound. Notexecuted as shellpipeline, nocommandinjection proof.','Static TF/HF exporters cast tokenIDs touint16; Wiki40B finisher metadata rewritten/byte offsets adjusted. No actualTensorFlow/tokenizer/originalpaperdataset qualification.']
emit(D/'text85-complete-source-review-20261016.json',{'status':'verified_complete_pinned_source_and_native_suffix_cli_finisher_controls','whole_repo_complete':True,'whole_implementation_complete':True,'repository':'google-research/deduplicate-text-datasets','commit':g['sha'],'blobs':15,'canonical_trees':3,'acquisition':str(a.relative_to(R)),'acquisition_sha256':h(a),'auditor_sha256':h(Path(__file__)),'selected_native_reports':reports,'original_cargo_build_passed':True,'upstream_tests_run':False,'upstream_test_scope':'No upstreamtest files in pinnedtree. Wholeunchangedsuffixmodule oracleharness andwholeCLI/Pythonfinisher ownedfixtures, notpaperdataset/upstreamsuite.','media_scope':'No trackedbinarymedia orREADMEembeddedimages/videos. Externalpapers/datasets notdownloaded orclaimedreviewed.','license':'Apache2 LICENSEfullyread. table.rs notes originalMITcode but leavesTODOattribution; READMEidentifiesBurntSushi/suffix origin. No productioncodecopied.','findings':findings,'decision':'Reference byte-oriented suffixarray indexing andstreaming candidate scan, requireboundedresources/completepartitions/validatedwidths/sourceidentity/atomicoutput before anyadoption. Exactsubstrdatasetdedup differsfromfile/pixel identity. No adoption/speedclaim.','limits':['WholeoriginalCargo build withresolvedunlockeddeps,MacARMsmallfixtures; notdependency-source/historicalenv/allplatform/currentroot/fullformat/hugefile/hardRSS/performance qualification.','Bounded3280text403440query oracles4runs remainqualifiedsuffixmodule only; fullCLIhasnativefalse-negative/crash findings anddoesnotinherit modulecorrectness.','No TensorFlow/HF/tokenizer/livecorpus/networkdatasets/userfiles orproductioncodecopy.']});print(json.dumps({'status':'verified','blobs':15,'trees':3,'whole_CLI_processes':28,'whole_finisher_processes':16,'prior_suffix_queries':1613760}))
