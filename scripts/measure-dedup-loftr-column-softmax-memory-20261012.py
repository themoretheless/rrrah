#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-loftr-column-softmax-memory-20261012.json';assert not out.exists();build=T/'rrrah-loftr-column-softmax-memory-build-attempt2-20261012.log';source=T/'rrrah-loftr-column-softmax-memory-probe-20261012';binary=Path(re.search(r'Executable unittests src/lib.rs \(([^\n]+)\)',build.read_text())[1]);saved=T/'rrrah-loftr-column-softmax-memory-executable-20261012';assert not saved.exists();shutil.copy2(binary,saved)
 pins={str(p):sha(p)for p in [build,saved,Path(__file__)]};pins.update({str(p):sha(p)for p in source.rglob('*')if p.is_file()});r={'status':'running','producer_pid':os.getpid(),'pins':pins,'results':[],'scope':'Separate-process CPU synthetic6400x6400 softmax stage, not whole LoFTR inference or total-memory admission. Baseline reproduces current original column-full/row128 implementation; aligned candidate chunks column128 with unchanged normalization and output.'}
 def save():p=out.with_suffix('.tmp');p.write_text(json.dumps(r,indent=2)+'\n');p.replace(out)
 save()
 for mode in ['original','aligned']:
  cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),str(saved),'--exact','coarse_matching::tests::rss_probe_'+mode,'--ignored','--nocapture','--test-threads=1'];start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);r['child_pid']=p.pid;save();stdout,stderr=p.communicate();row={'mode':mode,'command':cmd,'returncode':p.returncode,'stdout':stdout,'stderr':stderr,'elapsed_seconds':time.monotonic()-start};r['results'].append(row);save();assert p.returncode==0;row['rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1]);row['first'],row['sum']=re.search(r'first=(\S+) sum=(\S+)',stdout).groups();save()
 assert all(sha(p)==v for p,v in pins.items());a,b=r['results'];assert (a['first'],a['sum'])==(b['first'],b['sum']);r.update(status='complete_two_process_softmax_memory_comparison',rss_delta_bytes=b['rss_bytes']-a['rss_bytes']);save();print([(x['mode'],x['rss_bytes'])for x in r['results']])
if __name__=='__main__':main()
