#!/usr/bin/env python3
"""Fixed24-pair evaluation of the public composed CPU recovery API."""
import hashlib,json,os,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-exposure-recovery-cohort-20261012.json';folder=T/'rrrah-disc21-exposure-recovery-cohort-20261012';assert not out.exists() and not folder.exists();folder.mkdir()
 prep=D/'dedup-disc21-cohort-prepared-20261011.json';cohort=json.loads(prep.read_text());base=D/'dedup-disc21-exposure-recovery-api-20261012.json';b=json.loads(base.read_text());audit=D/'dedup-disc21-exposure-recovery-api-audit-20261012.json';a=json.loads(audit.read_text());assert a['status']=='verified_actual_cpu_guarded_primary_then_exposure_recovery_controls';assert all(sha(p)==h for p,h in a['pins'].items())
 pins=dict(b['pins']);pins.update(cohort['pins']);pins.update({str(p):sha(p)for p in [prep,base,audit,Path(__file__)]});assert all(sha(p)==h for p,h in pins.items());images={x['id']:x['path']for x in cohort['images']};r={'status':'running','producer_pid':os.getpid(),'pins':pins,'required':24,'plan':cohort['plan'],'results':[],'scope':'Actual composed primary25-bank rank2/min10 plus nine-region calibrated/min20 recovery, fixed24 pairs. Three matching development controls reused after pinned binary/input/argument checks. This measures composition directly; previously observed cohort and easy negatives are not held-out hard-negative precision. No default adoption or all-case completion.'}
 def save():tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 save()
 try:
  for i,case in enumerate(cohort['plan']):
   cmd=b['results'][0]['command'].copy();cmd[5]=images[case['query']];cmd[6]=images[case['reference']];row=dict(case,index=i,command=cmd,status='running');r['results'].append(row);save();old=next((x for x in b['results']if x['query']==case['query'] and x['reference']==case['reference']),None)
   if old is not None:
    assert old['command']==cmd and old['returncode']==0;stdout,stderr,code,elapsed=old['stdout'],old['stderr'],0,old['elapsed_seconds'];row.update(reused_from=str(base),original_control_index=b['results'].index(old))
   else:
    start=time.monotonic();proc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['child_pid']=proc.pid;save();stdout,stderr=proc.communicate();code=proc.returncode;elapsed=time.monotonic()-start
   log=folder/f'{i:04d}.log';log.write_text(stdout+stderr);row.update(returncode=code,stdout=stdout,stderr=stderr,elapsed_seconds=elapsed,log=str(log),log_sha256=sha(log));assert all(sha(p)==h for p,h in pins.items())
   if code:row['status']='execution_failed'
   else:
    value=json.loads(stdout);assert value['managed_used']==0;row['evidence']=value;row['status']='supported' if value['evidence']['supported']else'inconclusive'
   save();print(i,case['query'],row['status'],flush=True)
  r['status']='complete_fixed_exposure_recovery_cohort';save()
 except Exception as e:r.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
