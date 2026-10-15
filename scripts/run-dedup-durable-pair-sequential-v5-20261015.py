"""Oneoff sequential full baseline+audit then full candidate+audit, without performance dispatch."""
from pathlib import Path
import os,sys,json,hashlib,subprocess,traceback
R=Path(__file__).resolve().parents[1];D=R/'docs/research';out=D/'dedup-durable-pair-coordinator-v5-20261015.json';assert not out.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();producer=R/'scripts/qualify-dedup-durable-pair-v5-20261015.py';auditor=R/'scripts/audit-dedup-durable-pair-v5-20261015.py';data=dict(status='running',coordinator_pid=os.getpid(),coordinator_sha256=h(__file__),producer_sha256=h(producer),auditor_sha256=h(auditor),phase='preparing',scope='Freshcompleteunfilteredbaseline+candidate sequentialcorrectness qualification, never launchesbenchmark oradoptsproductioncode. Durableworkspace holds sources/logs/binaries/cache/TMP; currentstate mustrevalidate.')
def save():
 temp=out.with_suffix('.tmp');temp.write_text(json.dumps(data,indent=2)+'\n');temp.replace(out)
save()
try:
 for variant in ['baseline','candidate']:
  for action,script in [('gate',producer),('audit',auditor)]:
   data['phase']=variant+'_'+action;save();child=subprocess.Popen([sys.executable,'-u',str(script),variant],cwd=R);data['child_pid']=child.pid;save();code=child.wait();assert code==0,(variant,action,code)
  audit=D/f'dedup-durable-{variant}-audit-v5-20261015.json';a=json.loads(audit.read_text());assert a['gate_passed'] and a['failed']==0 and len(a['targets'])==162;data[variant+'_audit_sha256']=h(audit);save()
 data.update(status='paired_correctness_complete_not_performance',phase='complete');save()
except BaseException as error:data.update(status='stopped_or_failed_unqualified',controller_error=type(error).__name__,controller_traceback=traceback.format_exc());save();raise
