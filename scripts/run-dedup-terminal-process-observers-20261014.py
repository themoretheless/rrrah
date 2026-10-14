#!/usr/bin/env python3
"""Exact retained test binaries; permissioned process-state observation only."""
from pathlib import Path
import hashlib,json,subprocess,os,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();plans={}
for label in ['runner','collection_first']:
 p=D/f'dedup-{label}-terminal-binaries-20261014.json';m=json.loads(p.read_text());assert all(sha(r['retained'])==r['sha256']for r in m['rows']);plans[label]=(p,m)
F=Path('/private/tmp/rrrah-terminal-process-observers-20261014');F.mkdir(exist_ok=False);O=D/'dedup-terminal-process-observers-20261014.json';assert not O.exists()
state=dict(status='running',producer_pid=os.getpid(),producer_sha256=sha(__file__),pins={str(p):sha(p)for p,_ in plans.values()},results=[],scope='Exact retained frozen source6runner and initial collection unit/integration binaries. Same tests, no weakening/skips; elevated launch solely to allow ps observation of their own synthetic descendants. Original sandbox failures retained. Not current root16/native/fullworkspace/platform qualification.')
def save():
 tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
cases=[]
for label,target,filter_name,expected in [('runner','unittests src/lib.rs','model_worker::tests::successful_worker_response_does_not_leave_background_process',1),('runner','tests/worker_group.rs',None,8),('collection_first','unittests src/lib.rs','model_worker::tests::successful_worker_response_does_not_leave_background_process',1)]:
 p,m=plans[label];row=next(r for r in m['rows']if r['target']==target);original=json.loads((R/m['report']).read_text());cmd=[row['retained']]+([filter_name,'--exact']if filter_name else[])+['--test-threads','2'];cases.append(dict(label=label,target=target,command=cmd,cwd=original['source'],expected=expected,executable_sha256=row['sha256']))
save()
for index,case in enumerate(cases):
 log=F/f'{index}.log'
 with log.open('x')as out:
  child=subprocess.Popen(case['command'],cwd=case['cwd'],stdout=out,stderr=subprocess.STDOUT);state.update(active_index=index,child_pid=child.pid);save();code=child.wait()
 text=log.read_text();summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)
 state['results'].append(dict(**case,returncode=code,log=str(log),log_sha256=sha(log)));save();assert code==0 and summary and (int(summary[1]),int(summary[2]),int(summary[3]))==(case['expected'],0,0)
assert all(sha(p)==h for p,h in state['pins'].items());state['status']='passed_pending_audit';save();print(O)
