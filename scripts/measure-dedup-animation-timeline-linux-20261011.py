#!/usr/bin/env python3
import hashlib,json,os,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');M=D/'dedup-animation-timeline-linux-snapshot-20261011.json';OUT=D/'dedup-animation-timeline-linux-execution-20261011.json';LOG=D/'dedup-animation-timeline-linux-20261011.log';HELPER=T/'rrrah-animation-timeline-linux-20261011.sh'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not LOG.exists();m=json.loads(M.read_text());source=Path(m['snapshot'])
 def verify():assert all(sha(source/p)==h for p,h in m['file_hashes'].items())
 verify();r={'status':'running','producer_pid':os.getpid(),'manifest':str(M),'manifest_sha256':sha(M),'helper_sha256':sha(HELPER),'results':[]}
 def save():
  p=OUT.with_suffix('.tmp');p.write_text(json.dumps(r,indent=2)+'\n');p.replace(OUT)
 save()
 try:
  with LOG.open('w') as log:
   for mode in ['animation-unit','animation-integration']:
    cmd=['sh',str(HELPER),str(T/'rrrah-dedup-linux-native'),'/opt/rrrah-dedup-validation/sysroot',mode,str(source)];row={'mode':mode,'command':cmd};r['results'].append(row);p=subprocess.Popen(cmd,stdout=log,stderr=subprocess.STDOUT);row['pid']=p.pid;save();row['returncode']=p.wait();save();assert row['returncode']==0
  verify();assert sha(HELPER)==r['helper_sha256'];r.update(status='complete_bounded_frozen_animation_linux_execution',log_sha256=sha(LOG));save();print(r['status'])
 except Exception as e:r.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
