#!/usr/bin/env python3
"""Capture a completed prefix of either fixed cohort; preserve all observed gaps."""
import hashlib,json,re,sys
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 kind=sys.argv[1];assert kind in ['native','rank2'];name='dedup-disc21-cohort-native' if kind=='native' else 'dedup-disc21-rank2-cohort';p=Path(sys.argv[2]) if len(sys.argv)>2 else D/(name+'-20261011.json');raw=p.read_bytes();r=json.loads(raw);completed=[x for x in r['results'] if x['status']!='running'];n=len(completed);assert n and completed==r['results'][:n]
 out=D/f'{name}-prefix-{n:04d}-audit-20261011.json';capture=T/f'{name}-prefix-{n:04d}-20261011.json';assert not out.exists()
 if capture.exists():assert p==capture and capture.read_bytes()==raw
 else:capture.write_bytes(raw)
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items())
 prep=D/'dedup-disc21-cohort-prepared-20261011.json';data=json.loads(prep.read_text());assert r['plan']==data['plan'] and r['required']==len(data['plan'])==24;images={x['id']:x['path'] for x in data['images']}
 pa=D/'dedup-disc21-cohort-prepared-audit-20261011.json';a=json.loads(pa.read_text());assert a['status']=='verified_fixed_disc21_8_positive_16_distractor_cohort' and all(sha(k)==v for k,v in a['pins'].items());pins[str(pa)]=sha(pa)
 rows=[]
 for i,row in enumerate(completed):
  case=data['plan'][i];assert row['index']==i and all(row[k]==case[k] for k in ['query','reference','published_label']);cmd=row['command'];assert cmd[5:7]==[images[case['query']],images[case['reference']]];assert cmd[8:]==(['640','2.0','1000000'] if kind=='native' else ['640','2.0'])
  if 'reused_from' in row:
   original=Path(row['reused_from']);assert str(original) in pins and sha(original)==pins[str(original)];prior=json.loads(original.read_text());assert all(sha(k)==v for k,v in prior['pins'].items());pins.update(prior['pins']);assert sha(cmd[4])==pins[cmd[4]]
   if kind=='native':
    old=prior['results'][row['original_pilot_index']];assert row['evidence']==old['evidence'] and row['command']==old['command'];value=json.loads(old['stdout'])
   else:
    assert i==0 and row['evidence']==json.loads(prior['stdout']) and cmd[5:]==prior['command'][5:] and sha(cmd[4])==sha(prior['command'][4]);value=json.loads(prior['stdout'])
  else:
   assert sha(cmd[4])==pins[cmd[4]];assert sha(row['log'])==row['log_sha256'] and Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256']
   if row['returncode']:
    assert row['status']=='execution_failed';rows.append(dict(case,index=i,status='execution_failed'));continue
   value=json.loads(row['stdout'])
  assert value['managed_used']==0
  if kind=='native':
   assert value['files']==[2,9] and len(value['pairs'])==1;pair=value['pairs'][0];assert pair==row['evidence'] and (pair['left'],pair['right'])==(2,9);support=pair['support'];assert support is None or type(support) is bool;status='supported' if support is True else ('rejected' if support is False else 'inconclusive')
  else:
   assert value==row['evidence'] and value['attempts']==25 and type(value['supported']) is bool;status='supported' if value['supported'] else 'inconclusive'
  assert row['status']==status;rows.append(dict(case,index=i,status=status))
 counts={label:{s:sum(x['published_label']==label and x['status']==s for x in rows) for s in ['supported','inconclusive','rejected','execution_failed']} for label in ['copy','no_reference_match']}
 gaps=[x for x in rows if x['status']=='execution_failed' or (x['published_label']=='copy' and x['status']!='supported') or (x['published_label']=='no_reference_match' and x['status']=='supported')];pins.update({str(x):sha(x) for x in [capture,Path(__file__)]});out.write_text(json.dumps({'status':'verified_fixed_cohort_completed_prefix_only','kind':kind,'completed':n,'required':24,'counts':counts,'rows':rows,'observed_acceptance_gaps':gaps,'pins':pins,'scope':'Immutable prefix, not full cohort qualification. Rank2 first positive is development case used for policy selection. Native baseline and model-only candidate are distinct pipelines; unknown is not definite negative, distractors not hard semantic/burst precision.'},indent=2)+'\n');print(kind,n,counts,'gaps',gaps)
if __name__=='__main__':main()
