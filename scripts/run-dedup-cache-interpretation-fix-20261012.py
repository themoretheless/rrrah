import pathlib,subprocess,shutil,json,hashlib,time,os
R=pathlib.Path('/Users/themoretheless/Documents/Sources/rrrah'); T=pathlib.Path('/Users/themoretheless/.codex/tmp'); W=T/'rrrah-cache-interpretation-fix-20261012'; S=W/'source'; B=W/'target'; P=R/'docs/research/dedup-cache-interpretation-fix-20261012.json'
assert not W.exists() and not P.exists()
W.mkdir(); data={'status':'preparing','producer_pid':os.getpid(),'source':str(S),'target':str(B)}
def save(): P.write_text(json.dumps(data,indent=2)+'\n')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
save()
subprocess.run(['cp','-cpR',str(T/'rrrah-dedup-current-full-fixtures-source-20261012'),str(S)],check=True)
files=['src/lib.rs','src/decode.rs','src/decode_recipe.rs','tests/cache_interpretation.rs','tests/scan_retry.rs']
for f in files:shutil.copy2(R/'crates/rrrah-dedup'/f,S/'crates/rrrah-dedup'/f)
data['changed_inputs']={f:sha(S/'crates/rrrah-dedup'/f) for f in files}; save()
(B/'debug').mkdir(parents=True)
for part in ['deps','build','.fingerprint']:
 subprocess.run(['cp','-cpR',str(T/'rrrah-dedup-exact-portfolio-20261011/debug'/part),str(B/'debug'/part)],check=True)
cmd=['cargo','test','-p','rrrah-dedup','--features','decode','--locked','--offline','--test','cache_interpretation','--test','scan_retry','--jobs','2','--target-dir',str(B)]
data.update(status='running',command=cmd,log=str(W/'test.log'));save()
with (W/'test.log').open('w') as log:
 child=subprocess.Popen(cmd,cwd=S,stdout=log,stderr=subprocess.STDOUT);data['child_pid']=child.pid;save(); code=child.wait()
data.update(status='passed' if code==0 else 'failed',returncode=code,log_sha256=sha(W/'test.log'),inputs_unchanged=all(sha(S/'crates/rrrah-dedup'/f)==h for f,h in data['changed_inputs'].items()));save()
print(data['status'])
