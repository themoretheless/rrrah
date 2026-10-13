#!/usr/bin/env python3
"""Fixed24-pair model-bank candidate evaluation, separate from native-first baseline."""
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-rank2-candidate-20261011.json';single=json.loads(p.read_text());assert single['status']=='complete' and all(sha(k)==v for k,v in single['pins'].items())
 audit=D/'dedup-disc21-rank2-candidate-audit-20261011.json';check=json.loads(audit.read_text());assert all(sha(k)==v for k,v in check['pins'].items())
 prep=D/'dedup-disc21-cohort-prepared-20261011.json';data=json.loads(prep.read_text());assert all(sha(k)==v for k,v in data['pins'].items());images={x['id']:x['path'] for x in data['images']};assert len(data['plan'])==24
 A=T/'rrrah-disc21-rank2-cohort-qualified-20261011';out=D/'dedup-disc21-rank2-cohort-20261011.json';assert not A.exists() and not out.exists();A.mkdir();binary=A/'disc21_rank2_candidate_files';shutil.copy2(single['command'][4],binary)
 pins=dict(single['pins']);pins.update(data['pins']);pins.update({str(x):sha(x) for x in [p,audit,prep,binary,Path(__file__)]});r={'status':'running','producer_pid':os.getpid(),'pins':pins,'plan':data['plan'],'required':24,'completed':0,'results':[],'scope':'Automatic model-bank search with fixed rank2/filter12 candidate caller policy on8 positives16 distractors. Policy selected using first positive, which is a development case; remaining23 cases held out from policy selection. Separate from native-first baseline, not general precision or full DISC21 qualification.'}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 save()
 for i,case in enumerate(data['plan']):
  cmd=single['command'].copy();cmd[4]=str(binary);cmd[5]=images[case['query']];cmd[6]=images[case['reference']];row=dict(case,index=i,command=cmd,status='running');r['results'].append(row)
  if i==0:
   assert cmd[5:]==single['command'][5:] and sha(binary)==sha(single['command'][4]);row.update(status='supported',evidence=json.loads(single['stdout']),reused_from=str(p),elapsed_seconds=single['elapsed_seconds']);r['completed']+=1;save();continue
  save();start=time.monotonic();child=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['pid']=child.pid;save();stdout,stderr=child.communicate();log=A/f'{i:02d}.log';log.write_text(stdout+stderr);row.update(returncode=child.returncode,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log),elapsed_seconds=time.monotonic()-start)
  if child.returncode:row['status']='execution_failed'
  else:
   e=json.loads(stdout);assert e['attempts']==25 and e['managed_used']==0 and type(e['supported']) is bool;row.update(status='supported' if e['supported'] else 'inconclusive',evidence=e)
  assert all(sha(k)==v for k,v in pins.items());r['completed']+=1;save();print(i,case['published_label'],row['status'],flush=True)
 r['status']='complete_fixed_rank2_model_bank_cohort';save()
if __name__=='__main__':main()
