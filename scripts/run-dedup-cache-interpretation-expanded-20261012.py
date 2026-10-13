"""Qualify the expanded caller-interpretation regression after the earlier gate."""
import pathlib,subprocess,shutil,json,hashlib,os,time
R=pathlib.Path(__file__).resolve().parents[1]; W=pathlib.Path('/Users/themoretheless/.codex/tmp/rrrah-cache-interpretation-fix-20261012'); S=W/'source'; P=R/'docs/research/dedup-cache-interpretation-expanded-20261012.json'
assert not P.exists()
data={'status':'waiting_for_prior_decode_gate','producer_pid':os.getpid()}
def save(): P.write_text(json.dumps(data,indent=2)+'\n')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
save()
while True:
 try:os.kill(56866,0)
 except ProcessLookupError:break
 time.sleep(2)
old=S/'crates/rrrah-dedup/tests/cache_interpretation.rs';shutil.copy2(old,W/'initial-cache-interpretation.rs');shutil.copy2(R/'crates/rrrah-dedup/tests/cache_interpretation.rs',old)
files=['src/lib.rs','src/decode.rs','src/decode_recipe.rs','tests/cache_interpretation.rs','tests/scan_retry.rs','tests/decode.rs']
data.update(status='running',source=str(S),source_hashes={f:sha(S/'crates/rrrah-dedup'/f) for f in files},initial_test_copy=str(W/'initial-cache-interpretation.rs'))
cmd=['cargo','test','-p','rrrah-dedup','--features','decode','--locked','--offline','--test','cache_interpretation','--test','decode','--test','scan_retry','--jobs','2','--target-dir',str(W/'target')]
data.update(command=cmd,log=str(W/'final-test.log'));save()
with (W/'final-test.log').open('x') as log:
 child=subprocess.Popen(cmd,cwd=S,stdout=log,stderr=subprocess.STDOUT);data['child_pid']=child.pid;save();code=child.wait()
data.update(status='passed' if code==0 else 'failed',returncode=code,log_sha256=sha(W/'final-test.log'),inputs_unchanged=all(sha(S/'crates/rrrah-dedup'/f)==h for f,h in data['source_hashes'].items()));save();print(data['status'])
