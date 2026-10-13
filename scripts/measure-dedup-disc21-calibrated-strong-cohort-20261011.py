#!/usr/bin/env python3
"""Separate fixed24-pair evaluation of the calibrated strong-overlap recipe."""
import hashlib,json,os,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-calibrated-strong-cohort-20261011.json';folder=T/'rrrah-disc21-calibrated-strong-cohort-20261011';assert not out.exists() and not folder.exists();folder.mkdir()
 prep=D/'dedup-disc21-cohort-prepared-20261011.json';cohort=json.loads(prep.read_text());base=D/'dedup-disc21-overlap-calibrated-strong-20261011.json';b=json.loads(base.read_text());audit=D/'dedup-disc21-overlap-calibrated-strong-audit-20261011.json';a=json.loads(audit.read_text());assert a['status']=='verified_automatic_cpu_calibrated_strong_two_development_controls';assert all(sha(p)==h for p,h in a['pins'].items())
 pins=dict(b['pins']);pins.update(cohort['pins']);pins.update({str(p):sha(p)for p in [prep,base,audit,Path(__file__)]});assert all(sha(p)==h for p,h in pins.items());images={x['id']:x['path']for x in cohort['images']};r={'status':'running','producer_pid':os.getpid(),'pins':pins,'required':24,'plan':cohort['plan'],'results':[],'scope':'Separate fixed24 model-overlap/calibrated/min20 candidate evaluation. Q00906 was used during policy selection. Two exact matching controls reused with pinned binary/args/source checks. No aggregation with rank2/native-first results or default adoption; easy distractors do not qualify hard semantic precision.'}
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
  r['status']='complete_fixed_calibrated_strong_cohort';save()
 except Exception as e:r.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
