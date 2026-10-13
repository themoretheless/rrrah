#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-bidirectional-exposure-collection-snapshot-20261012.json';m=json.loads(parent.read_text());s=T/'rrrah-disc21-bidirectional-exposure-collection-source-20261012';h=T/'rrrah-disc21-bidirectional-exposure-collection-harness-20261012';out=D/'dedup-disc21-bidirectional-exposure-collection-20261012.json';assert not s.exists()and not h.exists()and not out.exists()
 for k,v in m['file_hashes'].items():
  old=Path(m['snapshot'])/k;assert sha(old)==v;p=s/k;p.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(old,p)
 h.mkdir();oldh=T/'rrrah-disc21-bank-diagnostic-native-harness-20261011';(h/'Cargo.toml').write_text((oldh/'Cargo.toml').read_text().replace(str(T/'rrrah-disc21-bank-diagnostic-native-snapshot-20261011'),str(s)));shutil.copy2(oldh/'Cargo.lock',h/'Cargo.lock');q=h/'src/bin/disc21_bidirectional_exposure_collection.rs';q.parent.mkdir(parents=True);shutil.copy2(R/'scripts/dedup-loftr-native-probe/src/bin/disc21_bidirectional_exposure_collection.rs',q)
 base=D/'dedup-disc21-exposure-recovery-api-20261012.json';b=json.loads(base.read_text());assert all(sha(p)==v for p,v in b['pins'].items());pins=dict(b['pins']);pins.update({str(p):sha(p)for p in [parent,base,Path(__file__),h/'Cargo.toml',h/'Cargo.lock',q]});pins.update({str(s/k):sha(s/k)for k in m['file_hashes']});prep=D/'dedup-disc21-cohort-prepared-20261011.json';c=json.loads(prep.read_text());assert all(sha(p)==v for p,v in c['pins'].items());images={x['id']:x['path']for x in c['images']};pins[str(prep)]=sha(prep)
 r={'status':'building','producer_pid':os.getpid(),'pins':pins,'files':[(1,'R014120'),(2,'Q00906')],'scope':'Actual public two-source bidirectional collection with reference ID1 and dark query ID2; expected support via reverse recovery. No held-out precision/default portfolio/full458/formats/platforms/RSS/full500 qualification.'}
 def save():p=out.with_suffix('.tmp');p.write_text(json.dumps(r,indent=2)+'\n');p.replace(out)
 save()
 try:
  runtime=T/'rrrah-loftr-native-runtime-20261010';env=os.environ.copy();env.update(LIBTORCH_USE_PYTORCH='1',PATH=str(runtime/'bin')+':'+env['PATH']);log=T/'dedup-disc21-bidirectional-exposure-collection-build-20261012.log'
  with log.open('w')as stream:
   proc=subprocess.Popen(['cargo','build','--manifest-path',str(h/'Cargo.toml'),'--bin','disc21_bidirectional_exposure_collection','--locked','--offline','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')],cwd='/tmp',env=env,stdout=stream,stderr=subprocess.STDOUT);r['build_pid']=proc.pid;save();code=proc.wait()
  r.update(build_returncode=code,build_log=str(log),build_log_sha256=sha(log));save();assert code==0
  binary=T/'rrrah-dedup-exact-portfolio-20261011/debug/disc21_bidirectional_exposure_collection';pins[str(binary)]=sha(binary);original=b['results'][0]['command'];cmd=original[:4]+[str(binary),images['Q00906'],images['R014120'],images['Q00000'],original[7],'640','2.0'];r.update(status='running',command=cmd);start=time.monotonic();proc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);r['child_pid']=proc.pid;save();stdout,stderr=proc.communicate();r.update(returncode=proc.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start);assert all(sha(p)==v for p,v in pins.items());assert proc.returncode==0;v=json.loads(stdout);assert v['managed_used']==0;r['status']='complete_bidirectional_collection_control';save();print([(p['left'],p['right'],p['supported'])for p in v['evidence']['pairs']],flush=True)
 except Exception as e:r.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
