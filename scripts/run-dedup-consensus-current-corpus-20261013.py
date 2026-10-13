#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os,sys,re,signal
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');role=sys.argv[1];assert role in ['positive','negative'];P=D/f'dedup-consensus-current-{role}-plan-20261013.json';p=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert p['stratum']==role and p['required']==len(p['cases']);assert all(sha(k)==h for k,h in p['pins'].items());M=Path(p['source_manifest']);assert sha(M)==p['source_manifest_sha256'];m=json.loads(M.read_text());S=Path(p['source']);assert all(sha(S/k)==h for k,h in m['source_sha256'].items());F=T/f'rrrah-consensus-current-{role}-20261013';assert not F.exists();F.mkdir();exe=F/'probe';subprocess.run(['cp','-cp',p['executable'],str(exe)],check=True);assert sha(exe)==p['pins'][p['executable']];O=D/f'dedup-consensus-current-{role}-20261013.json';assert not O.exists();d=dict(status='running',producer_pid=os.getpid(),stratum=role,plan=str(P),plan_sha256=sha(P),producer_sha256=sha(__file__),executable=str(exe),executable_sha256=sha(exe),required=p['required'],completed=0,results=[],counts=dict(supported_positive=0,inconclusive_positive=0,false_acceptance=0,inconclusive_negative=0,execution_failed=0),scope=p['scope'])
def save():
 q=O.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(O)
def interrupted(signum,frame):raise KeyboardInterrupt(f'signal{signum}')
signal.signal(signal.SIGTERM,interrupted);signal.signal(signal.SIGINT,interrupted);save();child=None
try:
 for i,c in enumerate(p['cases']):
  relevant=[c['left'],c['right'],c['arguments'][2]];assert all(sha(k)==p['pins'][k] for k in relevant);d.update(active_index=i);save();reused=c.get('reused_control');cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH=/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib',str(exe)]+c['arguments']
  if reused:
   assert sha(reused['report'])==reused['sha256'];r=json.loads(Path(reused['report']).read_text());assert r['command'][-8:]==c['arguments'] and sha(r['command'][-9])==sha(exe);r=dict(r,index=i,upstream_control=reused,reused=True)
  else:
   child=subprocess.Popen(cmd,cwd=R,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True);d['child_pid']=child.pid;save();out,err=child.communicate();rss=re.search(r'(\d+)\s+maximum resident set size',err);r=dict(index=i,command=cmd,returncode=child.returncode,stdout=out,stderr=err,rss_bytes=int(rss[1]) if rss else None,reused=False);child=None
  r.update(label=c['label'],expected=c['expected'],left=c['left'],right=c['right']);report=F/f'{i:04}.json';assert not report.exists();report.write_text(json.dumps(r,indent=2)+'\n');row=dict(index=i,label=c['label'],expected=c['expected'],report=str(report),sha256=sha(report),returncode=r['returncode'],reused=r['reused'])
  if r['returncode']:
   row['classification']='execution_failed';d['counts']['execution_failed']+=1;d['results'].append(row);d['completed']=len(d['results']);d['status']='terminal_runtime_failure';save();raise SystemExit(r['returncode'])
  out=json.loads(r['stdout']);assert out['managed_used']==0 and (out['support'] is True or out['support'] is None);assert out['support']==(True if out['forward_support'] is True and out['reverse_support'] is True else None);key=('supported_positive' if out['support'] is True else 'inconclusive_positive') if c['expected']=='duplicate' else ('false_acceptance' if out['support'] is True else 'inconclusive_negative');row.update(classification=key,support=out['support'],forward_support=out['forward_support'],reverse_support=out['reverse_support'],model_attempted=[out['forward_model_attempted'],out['reverse_model_attempted']]);assert all(sha(k)==p['pins'][k] for k in relevant);d['counts'][key]+=1;d['results'].append(row);d['completed']=len(d['results']);save()
 assert all(sha(k)==h for k,h in p['pins'].items()) and sha(exe)==d['executable_sha256'] and all(sha(S/k)==h for k,h in m['source_sha256'].items());d.update(status='complete_fixed_current_atomic_corpus_pending_audit',source_unchanged=True);save();print(role,d['status'],d['counts'])
except BaseException as e:
 if d['status']=='running':d.update(status='terminal_controller_failure',error=repr(e));save()
 raise
finally:
 if child is not None and child.poll() is None:
  os.killpg(child.pid,signal.SIGKILL);child.wait()
