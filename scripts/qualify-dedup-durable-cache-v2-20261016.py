"""Restore full unchanged current cache integration target into durable owned storage."""
from pathlib import Path
import hashlib,json,os,re,shutil,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research';W=R/'.dedup-work/current-cache-v2-20261016';W.mkdir(exist_ok=False);(W/'.gitignore').write_text('*\n');tmp=W/'tmp';tmp.mkdir();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
old_path=D/'dedup-cache-corruption-all-bits-audit-20261015.json';old=json.loads(old_path.read_text());prior=json.loads((D/'dedup-durable-root-exact-v2-20261015.json').read_text());deps={p:sha for p,sha in prior['dependencies'].items() if Path(p).name.startswith(('libthiserror-','libblake3-','libtempfile-'))};assert len(deps)==3
for p,sha in deps.items():assert h(p)==sha
sources={}
for name,path in [('cache.rs','crates/rrrah-dedup/src/cache.rs'),('pixels.rs','crates/rrrah-dedup/src/pixels.rs'),('integration.rs','crates/rrrah-dedup/tests/cache.rs')]:
 p=R/path;shutil.copyfile(p,W/name);sources[path]=h(p)
for p,sha in old['root_source_pins'].items():assert sources[p]==sha
local=R/'crates/rrrah-dedup/src/local.rs';s=local.read_text();error=s[s.index('#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]'):s.index('\n/// Extract deterministic')];helper=s[s.index('pub(crate) fn reserve_slot'):s.index('\n#[allow(clippy::too_many_lines)]')];sources[str(local.relative_to(R))]=h(local)
harness=W/'harness.rs';harness.write_text('#[allow(dead_code)]mod local {\n'+error+'\n'+helper+'\n}\n#[path="pixels.rs"]pub mod pixels;\n#[path="cache.rs"]pub mod cache;\n')
lib=W/'librrrah_dedup.rlib';exe=W/'tests';search=['-L','dependency='+str(Path(next(iter(deps))).parent)];extern=[]
for p in deps:extern+=['--extern',Path(p).name.split('-',1)[0][3:]+'='+p]
build=['rustc','+stable','--edition=2024','--crate-name','rrrah_dedup','--crate-type','rlib','-Dwarnings','-C','opt-level=1',str(harness),*search,*extern,'-o',str(lib)]
link=['rustc','+stable','--edition=2024','--test','-Dwarnings','-C','opt-level=1',str(W/'integration.rs'),*search,*extern,'--extern','rrrah_dedup='+str(lib),'-o',str(exe)]
env=dict(os.environ,TMPDIR=str(tmp.resolve()));commands={}
for label,cmd in [('library-build',build),('integration-build',link),('inventory',[str(exe),'--list']),('tests',[str(exe),'--test-threads=2'])]:
 commands[label]=cmd
 with (W/(label+'.log')).open('x') as f:subprocess.run(cmd,cwd=R,env=env,stdout=f,stderr=subprocess.STDOUT,check=True,timeout=120)
listed=(W/'inventory.log').read_text();log=(W/'tests.log').read_text();names=re.findall(r'^(.*): test$',listed,re.M);results=re.findall(r'^test (\S+) \.\.\. (ok|FAILED|ignored)$',log,re.M);assert len(names)==len(set(names))==10 and sorted(names)==sorted(old['inventory']);assert sorted(results)==sorted((n,'ok') for n in names) and '10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' in log
assert all(h(R/p)==sha for p,sha in sources.items()) and all(h(p)==sha for p,sha in deps.items())
out=D/'dedup-durable-cache-v2-20261016.json';assert not out.exists();out.write_text(json.dumps(dict(status='fresh_passed_complete_current_cache_integration_scope',passed=10,failed=0,ignored=0,filtered=0,inventory=names,corruption_bit_cases=3264,wire_bit_cases=2048,source_pins=sources,dependency_pins=deps,compiler=subprocess.check_output(['rustc','+stable','-vV'],text=True),commands=commands,binary=str(exe),binary_sha256=h(exe),library=str(lib),library_sha256=h(lib),harness=str(harness),harness_sha256=h(harness),logs={str(W/(x+'.log')):h(W/(x+'.log')) for x in commands},producer_sha256=h(__file__),prior_audit=str(old_path.relative_to(R)),prior_audit_sha256=h(old_path),prior_raw_binary_available=Path(old['binary']).is_file(),scope='Freshsame10unchangedfullcacheintegrationtests, wholecurrentcache/pixels modules, exactselectedLocalError/reserve_slot helpers, realcacheddependencies/directrustcMacARM. Originalroot-leveltestnames preserved forselfspawnedpublication controls. Strict-Dwarnings build;dead_code allowance appliesonlyisolatedselectedlocalhelpermodule. Recipe/content/framekey, ownedvalues,budget,allbitscorruption,atomic/concurrentprocess/cancellation/crashpublication controls; durableownedrawbinary/logs/sources retained. No fullCargo/wholecrate/decode/platform/powerloss/hardRSS/performance or speedup proof.'),indent=2)+'\n');print('Fresh10/10cacheintegrationtests; strictbuild; durablebinaries/logs retained')
