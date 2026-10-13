#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-overlap-exposure-search-snapshot-20261012.json';m=json.loads(parent.read_text());old=Path(m['snapshot']);s=T/'rrrah-disc21-overlap-cached-exposure-snapshot-20261012';h=T/'rrrah-disc21-overlap-cached-exposure-harness-20261012';out=D/'dedup-disc21-overlap-cached-exposure-20261012.json';assert not s.exists() and not h.exists() and not out.exists()
 for f,v in m['file_hashes'].items():
  assert sha(old/f)==v; q=s/f;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(old/f,q)
 shutil.copy2(R/'crates/rrrah-dedup/src/model_loftr.rs',s/'crates/rrrah-dedup/src/model_loftr.rs');h.mkdir();oldh=T/'rrrah-disc21-bank-diagnostic-native-harness-20261011';(h/'Cargo.toml').write_text((oldh/'Cargo.toml').read_text().replace(str(T/'rrrah-disc21-bank-diagnostic-native-snapshot-20261011'),str(s)));shutil.copy2(oldh/'Cargo.lock',h/'Cargo.lock');q=h/'src/bin/disc21_overlap_cached_exposure_files.rs';q.parent.mkdir(parents=True);shutil.copy2(R/'scripts/dedup-loftr-native-probe/src/bin/disc21_overlap_cached_exposure_files.rs',q)
 pins={str(parent):sha(parent),str(Path(__file__)):sha(__file__)};pins.update({str(s/k):sha(s/k)for k in m['file_hashes']});pins.update({str(p):sha(p)for p in [h/'Cargo.toml',h/'Cargo.lock',q]})
 r={'status':'building','producer_pid':os.getpid(),'pins':pins,'results':[],'scope':'Public cached CPU library API for nine automatic overlapping halves versus whole reference; min20 geometry/anchors, explicit guarded exposure calibration, contrast0.005,rank8/2,90%/1000pair rules unchanged. Two development controls, not held-out precision or default adoption. Diagnostic JSON/metadata and model RSS external to managed payload.'}
 def save():tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 save()
 try:
  runtime=T/'rrrah-loftr-native-runtime-20261010';env=os.environ.copy();env.update(LIBTORCH_USE_PYTORCH='1',PATH=str(runtime/'bin')+':'+env['PATH']);log=T/'dedup-disc21-overlap-cached-exposure-build-20261012.log'
  with log.open('w') as stream:
   proc=subprocess.Popen(['cargo','build','--manifest-path',str(h/'Cargo.toml'),'--bin','disc21_overlap_cached_exposure_files','--locked','--offline','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')],cwd='/tmp',env=env,stdout=stream,stderr=subprocess.STDOUT);r['build_pid']=proc.pid;save();code=proc.wait()
  r.update(build_returncode=code,build_log=str(log),build_log_sha256=sha(log));save();assert code==0
  binary=T/'rrrah-dedup-exact-portfolio-20261011/debug/disc21_overlap_cached_exposure_files';pins[str(binary)]=sha(binary)
  original=D/'dedup-disc21-bank-diagnostic-native-20261011.json';data=json.loads(original.read_text());assert all(sha(k)==v for k,v in data['pins'].items());pins[str(original)]=sha(original);prep=D/'dedup-disc21-cohort-prepared-20261011.json';cohort=json.loads(prep.read_text());assert all(sha(k)==v for k,v in cohort['pins'].items());pins.update(cohort['pins']);pins[str(prep)]=sha(prep);images={x['id']:x['path']for x in cohort['images']}
  for query,reference,label in [('Q00906','R014120','copy'),('Q00000','R020965','no_reference_match')]:
   cmd=data['command'].copy();cmd[4]=str(binary);cmd[5]=images[query];cmd[6]=images[reference];row={'query':query,'reference':reference,'label':label,'status':'running','command':cmd};r['results'].append(row);r['status']='running';save();start=time.monotonic();proc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['child_pid']=proc.pid;save();stdout,stderr=proc.communicate();row.update(status='complete' if proc.returncode==0 else 'execution_failed',returncode=proc.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start);assert all(sha(k)==v for k,v in pins.items());save();assert proc.returncode==0;print(query,reference,json.loads(stdout)['evidence']['supported'],flush=True)
  r['status']='complete_two_development_controls';save()
 except Exception as e:r.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
