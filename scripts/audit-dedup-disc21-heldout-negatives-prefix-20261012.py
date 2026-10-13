#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 report=D/'dedup-disc21-heldout-negatives-runtime-20261012.json';raw=report.read_bytes();r=json.loads(raw);assert len(r['plan'])==16
 bp=D/'dedup-disc21-heldout-negative-build-audit-20261012.json';b=json.loads(bp.read_text());assert b['policy_changes']==[]
 for m in [r,b]:assert all(sha(p)==h for p,h in m['pins'].items())
 complete=[]
 for row in r['results']:
  if row['status']=='running':break
  complete.append(row)
 n=len(complete);assert n>0
 out=D/f'dedup-disc21-heldout-negatives-prefix-{n:02d}-audit-20261012.json';capture=T/f'rrrah-disc21-heldout-negatives-prefix-{n:02d}-20261012.json';assert not out.exists() and not capture.exists();capture.write_bytes(raw)
 pins={str(p):sha(p) for p in [report] if False};pins.update({str(p):sha(p) for p in [capture,bp,Path(__file__)]});pins.update(r['pins']);counts={'false_acceptances':0,'inconclusive_non_acceptances':0,'execution_errors':0};rows=[]
 for i,row in enumerate(complete):
  assert all(row[k]==v for k,v in r['plan'][i].items());assert row['published_label']=='no_reference_match';assert row['command'][-2:]==['640','2.0']
  for key in ['stdout','stderr']:
   p=Path(row[key+'_path']);assert sha(p)==row[key+'_sha256'];pins[str(p)]=sha(p)
  if row['status']=='error':counts['execution_errors']+=1;continue
  assert row['returncode']==0
  value=json.loads(Path(row['stdout_path']).read_text());assert value==row['evidence'];assert value['managed_used']==0 and value['managed_report_used']>0
  ev=value['evidence'];assert ev['file_ids']==[1,2] and len(ev['pairs'])==1;pair=ev['pairs'][0];assert (pair['left'],pair['right'])==(1,2)
  for direction in ['forward','reverse']:
   result=pair[direction];assert result is not None and isinstance(result['supported'],bool)
   if not result['supported']:assert result['primary']['attempts']==25 and result['recovery']['search']['attempts']==9
  assert pair['supported']==(pair['forward']['supported'] or pair['reverse']['supported'])==row['false_acceptance']
  rss=int(re.search(r'(\d+)\s+maximum resident set size',Path(row['stderr_path']).read_text())[1]);kind='false_acceptances' if pair['supported'] else 'inconclusive_non_acceptances';counts[kind]+=1
  rows.append({'query':row['query'],'reference':row['reference'],'false_acceptance':pair['supported'],'rss_bytes':rss,'elapsed_seconds':row['elapsed_seconds'],'managed_peak':value['managed_peak']})
 out.write_text(json.dumps({'status':'verified_completed_prefix_only','completed':n,'required':16,'counts':counts,'rows':rows,'pins':pins,'scope':'Actual fixed-policy current-source bidirectional CPU collection on unused local publisher distractors. Unsupported is inconclusive, not proof of non-copy. Hash-near selection, not semantic/burst hard negatives. Not pretraining holdout, full precision, full458/full500 or hard total-memory bound.'},indent=2)+'\n');print(n,counts)
if __name__=='__main__':main()
