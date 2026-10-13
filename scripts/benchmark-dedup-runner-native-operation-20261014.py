#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os,time,statistics,resource
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');P=D/'dedup-runner-native-probe-20261014.json';b=json.loads(P.read_text());assert b['status']=='built_pending_execution' and b['returncode']==0 and b['source_unchanged']
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();new=Path(b['executable']);old=T/'rrrah-consensus-native-v2-20261013/probe';assert sha(new)==b['executable_sha256'];Q=D/'dedup-consensus-native-plan-20261013.json';q=json.loads(Q.read_text());cases=[c for c in q['cases'] if c['label'] in ['self_native','table_true']]
F=T/'rrrah-runner-native-operation-benchmark-20261014';F.mkdir(exist_ok=False);O=D/'dedup-runner-native-operation-benchmark-20261014.json';assert not O.exists();pins={str(p):sha(p) for p in [P,Q,Path(__file__),old,new]}
for c in cases:
 for p in c['arguments'][:3]:pins[p]=sha(p)
d=dict(status='running',pid=os.getpid(),pins=pins,results=[],scope='Whole fresh-process native atomic consensus baseline-source versus latest linear-runner-source comparison including startup, decode and both directions. Two controls, three alternating pairs each. Source versions also include formatting drift; this is version comparison, not isolated linear-runner causal proof. Host shared with test/corpus jobs; child process user+system CPU also recorded to reduce wall contention bias; still a small control sample.')
def save():
 tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(d,indent=2)+'\n');tmp.replace(O)
save();env=os.environ.copy();env['DYLD_LIBRARY_PATH']=str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib')
for c in cases:
 expected=None
 for i in range(3):
  measured={}
  for label,exe in ([('baseline',old),('runner',new)] if i%2==0 else [('runner',new),('baseline',old)]):
   before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic_ns();r=subprocess.run([str(exe)]+c['arguments'],env=env,capture_output=True,text=True);elapsed=time.monotonic_ns()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN);cpu=(after.ru_utime-before.ru_utime)+(after.ru_stime-before.ru_stime)
   row=dict(case=c['label'],pair=i,version=label,elapsed_ns=elapsed,cpu_seconds=cpu,returncode=r.returncode,stdout=r.stdout,stderr=r.stderr);out=F/f"{c['label']}-{i}-{label}.json";out.write_text(json.dumps(row,indent=2)+'\n');d['results'].append(dict(report=str(out),sha256=sha(out),returncode=r.returncode));save()
   if r.returncode:d['status']='execution_failed';save();raise SystemExit(r.returncode)
   v=json.loads(r.stdout);assert v['managed_used']==0;sem={k:v[k] for k in ['support','forward_support','reverse_support','forward_model_attempted','reverse_model_attempted']}
   if expected is None:expected=sem
   assert expected==sem;measured[label]=dict(wall=elapsed,cpu=cpu)
  print(c['label'],i,dict(wall_ratio=measured['runner']['wall']/measured['baseline']['wall'],cpu_ratio=measured['runner']['cpu']/measured['baseline']['cpu']),flush=True)
assert all(sha(p)==h for p,h in pins.items());d['status']='complete_pending_audit';save()
