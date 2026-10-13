#!/usr/bin/env python3
import json,hashlib,subprocess,os,re,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-loftr-bank-partition-plan-20261013.json';plan=json.loads(P.read_text());H=Path(plan['harness']);A=Path(plan['source_audit']);audit=json.loads(A.read_text());S=Path(audit['source'])
assert all(sha(S/p)==h for p,h in audit['source_sha256'].items())
assert all(sha(H/p)==h for p,h in plan['harness_sha256'].items())
assert all(sha(p)==h for p,h in plan['pins'].items())
assert all(sha(c['plan'])==c['plan_sha256'] for c in plan['cases'])
O=D/'dedup-loftr-bank-partition-20261013.json';F=T/'rrrah-loftr-bank-partition-qualified-20261013';L=T/'rrrah-loftr-bank-partition-build-20261013.log';assert not O.exists() and not F.exists() and not L.exists();F.mkdir()
runtime=T/'rrrah-loftr-native-runtime-20261010';env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH']
data=dict(status='building',producer_pid=os.getpid(),plan=str(P),plan_sha256=sha(P),source_audit_sha256=sha(A),harness=str(H),build_log=str(L),required=len(plan['cases']),completed=0,results=[],scope=plan['scope'])
def save():
 q=O.with_suffix('.tmp');q.write_text(json.dumps(data,indent=2)+'\n');q.replace(O)
def main():
 save()
 with L.open('x') as log:
  cmd=['cargo','generate-lockfile','--offline'];data['lock_command']=cmd;child=subprocess.Popen(cmd,cwd=H,env=env,stdout=log,stderr=subprocess.STDOUT);data['child_pid']=child.pid;save();code=child.wait()
  if code:data.update(status='terminal_lock_failure',returncode=code,build_log_sha256=sha(L));save();return
  data['lock_sha256']=sha(H/'Cargo.lock')
  cmd=['cargo','build','--offline','--locked','--jobs','2','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')];data['build_command']=cmd;child=subprocess.Popen(cmd,cwd=H,env=env,stdout=log,stderr=subprocess.STDOUT);data['child_pid']=child.pid;save();code=child.wait()
 data.update(build_returncode=code,build_log_sha256=sha(L))
 if code:data['status']='terminal_build_failure';save();return
 exe=F/'loftr_bank_partition_diagnostic';subprocess.run(['cp','-cp',str(T/'rrrah-dedup-exact-portfolio-20261011/debug/loftr_bank_partition_diagnostic'),str(exe)],check=True);data.update(executable=str(exe),executable_sha256=sha(exe),status='running');save()
 for c in plan['cases']:
  assert sha(c['plan'])==c['plan_sha256'] and sha(exe)==data['executable_sha256']
  for k in ['left','right','weights']:assert sha(c[k])==plan['pins'][c[k]]
  cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(runtime/'lib/python3.12/site-packages/torch/lib'),str(exe),c['plan']];data.update(active_label=c['label'],active_command=cmd);save();started=time.monotonic();child=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);data['child_pid']=child.pid;save();stdout,stderr=child.communicate();elapsed=time.monotonic()-started
  for k in ['left','right','weights']:assert sha(c[k])==plan['pins'][c[k]]
  parsed=json.loads(stdout) if child.returncode==0 else None
  if parsed:assert parsed['managed_used']==0
  rss=re.search(r'(\d+)\s+maximum resident set size',stderr);row=dict(label=c['label'],command=cmd,returncode=child.returncode,elapsed_seconds=elapsed,rss_bytes=int(rss[1]) if rss else None,stdout=stdout,stderr=stderr)
  result=F/(c['label']+'.json');result.write_text(json.dumps(row,indent=2)+'\n');data['results'].append(dict(label=c['label'],report=str(result),report_sha256=sha(result),terminal='partitioned' if parsed else 'execution_failed'));data['completed']+=1;save();print(c['label'],data['results'][-1]['terminal'],flush=True)
 data.update(status='complete_fixed12_bank_partition',source_unchanged=all(sha(S/p)==h for p,h in audit['source_sha256'].items()),harness_unchanged=all(sha(H/p)==h for p,h in plan['harness_sha256'].items()));save()
main()
