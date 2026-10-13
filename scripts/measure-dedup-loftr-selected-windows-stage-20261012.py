#!/usr/bin/env python3
import hashlib,json,os,re,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 tests=D/'dedup-loftr-selected-windows-v2-direct-tests-20261012.json';m=json.loads(tests.read_text());assert m['returncode']==0 and all(sha(p)==h for p,h in m['pins'].items())
 exe=T/'rrrah-loftr-selected-windows-v2-test-executable-20261012';out=D/'dedup-loftr-selected-windows-stage-20261012.json';assert not out.exists();pins=dict(m['pins']);pins.update({str(p):sha(p) for p in [tests,Path(__file__)]});r={'status':'running','producer_pid':os.getpid(),'pins':pins,'results':[],'scope':'One fine-preprocessing stage, CPU single batch Float,128channels,320x320maps,256of6400windows. Fresh processes, same executable, exact original dense-forward test oracle. Not whole model/collection/RSS hard cap or speed qualification.'}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 save()
 for i,mode in enumerate(['dense','selected','selected','dense']):
  cmd=['/usr/bin/time','-l','/usr/bin/env',m['command'][1],'RRRAH_SELECTED_WINDOW_CONTROL='+mode,str(exe),'--exact','fine_preprocess::tests::selected_windows_memory_control','--ignored','--nocapture','--test-threads=1'];start=time.monotonic();run=subprocess.run(cmd,capture_output=True,text=True,cwd='/tmp');log=T/f'rrrah-loftr-selected-windows-stage-{i:02d}-{mode}-20261012.log';log.write_text(run.stdout+run.stderr);pins[str(log)]=sha(log)
  row={'mode':mode,'command':cmd,'returncode':run.returncode,'elapsed_seconds':time.monotonic()-start,'stdout':run.stdout,'stderr':run.stderr,'log':str(log),'log_sha256':sha(log)};r['results'].append(row);save();assert run.returncode==0 and '1 passed; 0 failed' in run.stdout
  row['hash']=re.search(r'hash=([0-9a-f]{16})',run.stdout)[1];row['rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',run.stderr)[1]);save();print(mode,row['rss_bytes'],row['hash'],flush=True)
 assert len({x['hash'] for x in r['results']})==1
 r['status']='complete_stage_only_not_adopted';save()
if __name__=='__main__':main()
