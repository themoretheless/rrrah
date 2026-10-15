"""Restore fresh complete current exact-scope evidence into durable owned storage."""
from pathlib import Path
import hashlib,json,os,re,shutil,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research';W=R/'.dedup-work/root-exact-v2-20261015';W.mkdir(exist_ok=False);(W/'.gitignore').write_text('*\n')
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
prior=D/'dedup-exact-read-offsets-audit-20261015.json';old=json.loads(prior.read_text());deps=old['dependencies'];assert len(deps)==4
for p,sha in deps.items():assert h(Path(p))==sha,p
sources={}
for name,path in [('exact.rs','crates/rrrah-dedup/src/exact.rs'),('integration.rs','crates/rrrah-dedup/tests/exact_files.rs'),('large.rs','crates/rrrah-dedup/tests/exact_large_files.rs'),('repeated.rs','crates/rrrah-dedup/tests/exact_repeated_metadata.rs')]:
 p=R/path;shutil.copyfile(p,W/name);sources[path]=h(p)
harness=W/'harness.rs';harness.write_text('extern crate self as rrrah_dedup;\n#[path="exact.rs"]pub mod exact;\n#[path="integration.rs"]mod integration_controls;\n#[path="large.rs"]mod large_controls;\n#[path="repeated.rs"]mod repeated_metadata_controls;\n')
exe=W/'tests';cmd=['rustc','+stable','--edition=2024','--test',str(harness),'-C','opt-level=1','-L','dependency='+str(Path(next(iter(deps))).parent),'-o',str(exe)]
for p in deps:cmd+=['--extern',Path(p).name.split('-',1)[0][3:]+'='+p]
plan={'status':'running','producer_pid':os.getpid(),'sources':sources,'dependencies':deps,'command':cmd,'workspace':str(W)}
(W/'plan.json').write_text(json.dumps(plan,indent=2)+'\n')
for label,command in [('build',cmd),('inventory',[str(exe),'--list']),('tests',[str(exe),'--test-threads=2'])]:
 with (W/(label+'.log')).open('x') as f:subprocess.run(command,cwd=R,stdout=f,stderr=subprocess.STDOUT,check=True,timeout=120)
listed=(W/'inventory.log').read_text();log=(W/'tests.log').read_text();names=re.findall(r'^(.*): test$',listed,re.M);results=re.findall(r'^test (\S+) \.\.\. (ok|FAILED|ignored)$',log,re.M)
assert len(names)==len(set(names))==54 and set(names)==set(old['inventory'])|{'repeated_metadata_controls::repeated_comparisons_reread_content_when_size_and_modified_time_are_restored'}
assert sorted(results)==sorted((n,'ok') for n in names);assert '54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' in log
assert all(h(R/p)==sha for p,sha in sources.items());assert all(h(Path(p))==sha for p,sha in deps.items())
out=D/'dedup-durable-root-exact-v2-20261015.json';assert not out.exists();out.write_text(json.dumps({'status':'fresh_passed_complete_current_exact_scope','sources':sources,'dependencies':deps,'compiler':subprocess.check_output(['rustc','+stable','-vV'],text=True),'passed':54,'failed':0,'ignored':0,'filtered':0,'inventory':names,'binary':str(exe),'binary_sha256':h(exe),'logs':{str(W/(x+'.log')):h(W/(x+'.log')) for x in ['build','inventory','tests']},'producer':str(Path(__file__).relative_to(R)),'producer_sha256':h(Path(__file__)),'prior_scope_reference':str(prior.relative_to(R)),'prior_sha256':h(prior),'scope':'Full unchanged current exact module, complete exact_files and three sparse largefiles controls with real cached deps; directrustc54tests, all inventory matched. Reordered blocks/shortread192failurecases/hardlinks/mutation/cancellation and new repeatedsameinode/samemtime/samesize comparison/scan/verifiedscan controls across4sizes included. Fresh durable raw evidence replaces unavailable TMP artifacts for this scope only. No full Cargo/library/decode/Windows/Linux/corpus/physicalIO/hardRSS or speedup proof.'},indent=2)+'\n');print('Fresh54/54 exact-scope tests; durable raw artifacts retained')
