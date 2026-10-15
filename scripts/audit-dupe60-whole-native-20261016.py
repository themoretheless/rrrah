"""Audit complete original Cargo test inventory and actual CLI controls; full repository review remains partial."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/dupe60-20261016';S=W/'source';B=W/'cargo-build';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
f=json.loads((D/'dupe60-fetched-source-pins-20261016.json').read_text());rows=f['tree']['tree'];assert len(f['files'])==97
for x in f['files']:
 p=S/x['path'];b=p.read_bytes();assert len(b)==x['bytes'] and h(p)==x['sha256'] and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==x['git_blob']
 if x['path']=='Cargo.toml':assert (B/x['path']).read_text()==p.read_text()+'\n[workspace]\n'
 else:assert (B/x['path']).read_bytes()==b
trees={'':f['commit_object']['tree']['sha'],**{x['path']:x['sha'] for x in rows if x['type']=='tree'}}
for parent,sha in trees.items():
 es=[dict(x,path=Path(x['path']).name) for x in rows if ('' if str(Path(x['path']).parent)=='.' else str(Path(x['path']).parent))==parent];es.sort(key=lambda x:x['path']+('/' if x['type']=='tree' else ''));b=b''.join(x['mode'].lstrip('0').encode()+b' '+x['path'].encode()+b'\0'+bytes.fromhex(x['sha']) for x in es);assert hashlib.sha1(b'tree '+str(len(b)).encode()+b'\0'+b).hexdigest()==sha
cli_path=D/'dupe60-whole-cli-controls-20261016.json';cli=json.loads(cli_path.read_text());assert len(cli['controls'])==8 and cli['control_runs']==2 and cli['producer_sha256']==h(R/'scripts/qualify-dupe60-cli-controls-20261016.py')
for p,sha in cli['evidence_pins'].items():assert h(R/p)==sha
logs=[W/f'cargo-upstream-run{i}.log' for i in [1,2]];targets=[]
for run,path in enumerate(logs,1):
 log=path.read_text();headers=list(re.finditer(r'^\s*Running ([^\n]+) \(([^\n]+)\)$',log,re.M));assert len(headers)==4
 current=[]
 for i,match in enumerate(headers):
  body=log[match.end():headers[i+1].start() if i+1<len(headers) else len(log)];binary=(B/match.group(2)).resolve();inventory=subprocess.check_output([str(binary),'--list'],cwd=B,text=True);names=re.findall(r'^(.+): test$',inventory,re.M);result=re.findall(r'^test (.+) \.\.\. ok$',body,re.M);assert sorted(names)==sorted(result)
  summary=re.search(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out',body);assert summary and int(summary[1])==len(names)
  ip=W/f'whole-native-inventory-{i}.log'
  if run==1:ip.write_text(inventory)
  else:assert ip.read_text()==inventory
  current.append(dict(target=match.group(1),binary=str(binary.relative_to(R)),binary_sha256=h(binary),inventory=str(ip.relative_to(R)),inventory_sha256=h(ip),tests=names,passed=len(names)))
 if run==1:targets=current
 else:assert current==targets
assert [x['passed'] for x in targets]==[72,7,2,17] and sum(x['passed'] for x in targets)==98
read_files=sorted([str(p.relative_to(S)) for p in S.glob('src/**/*.rs')]+[str(p.relative_to(S)) for p in S.glob('src/lang/queries/*.scm')]+['tests/check.rs','tests/history.rs','tests/languages.rs','Cargo.toml','LICENSE']);assert len(read_files)==42
remaining=[x['path'] for x in f['files'] if 'content'in x and x['path'] not in read_files];binary_files=[x['path'] for x in f['files'] if x.get('binary')];assert len(remaining)==48 and len(binary_files)==7
pins={str(p.relative_to(R)):h(p) for p in logs+[W/'cargo-build.log',cli_path,R/'scripts/qualify-dupe60-cli-controls-20261016.py',D/'dupe60-fetched-source-pins-20261016.json']}
for x in targets:pins[x['binary']]=x['binary_sha256'];pins[x['inventory']]=x['inventory_sha256']
out=D/'dupe60-whole-native-partial-source-review-20261016.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_complete_original_cargo_suite_and_whole_cli_controls_with_partial_repository_review',whole_repo_complete=False,whole_implementation_complete=False,whole_production_rust_implementation_read=True,repository='Rafaelpta/dupehound',commit=f['commit_object']['sha'],tree=f['commit_object']['tree']['sha'],source_blobs=97,verified_trees=len(trees),fully_read_files=read_files,remaining_plain_text_files=remaining,remaining_binary_files=binary_files,original_tests=98,original_suite_runs=2,original_targets=targets,actual_whole_cli_controls=cli['controls'],actual_whole_cli_runs=2,evidence_pins=pins,auditor_sha256=h(__file__),build_scope='Complete unchanged21Rustfiles/16queries/all3originalintegrationtestfiles; onlyCargo.toml workspaceboundary appended inisolatedbuildcopy. OriginalCargo.lock unchanged/defaultcardfeature. ActualoriginalCLIbinary andcomplete4testtargets, ownedGitfixtureswithceiling andTMPDIR. Originalofflineattempt missingstreaming-iterator; subsequentlockeddownload/build passed.',limits=['48remainingplainfiles and7binaryassets stillrequire review. Includesdocumentation/benchmarkcode/results/SVG andPNG/fonts. Do notincrementfull500count.','Passing98upstreamtests doesnotcover8additionalwholeCLIcounterexamples; no productionadoption.','No timing/speedup/externalbenchmarkreproduction/platform/currentRoot/completeformats/corpus/hardRSS claim.','Entirethirdpartydependency source notreviewed; originallockedpackagebuild only. Earlierselecteddataadapter evidence retainedseparately, noadapter used inthiswholeCLI/suite.']),indent=2)+'\n');print('98originaltests twice/4targets/8wholeCLIcontrolstwice/42of97filesread; fullrepository incomplete')
