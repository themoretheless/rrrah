#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 path=D/'dedup-disc21-exposure-recovery-cohort-20261012.json';raw=path.read_bytes();r=json.loads(raw);rows=[x for x in r['results']if x['status']!='running'];n=len(rows);assert n and rows==r['results'][:n]
 out=D/f'dedup-disc21-exposure-recovery-cohort-prefix-{n:04d}-audit-20261012.json';capture=T/f'rrrah-disc21-exposure-recovery-prefix-{n:04d}-20261012.json';assert not out.exists() and not capture.exists()
 assert all(sha(p)==h for p,h in r['pins'].items());prep=json.loads((D/'dedup-disc21-cohort-prepared-20261011.json').read_text());assert r['plan']==prep['plan'] and r['required']==24;images={x['id']:x['path']for x in prep['images']}
 totals={label:{'supported':0,'inconclusive':0,'execution_failed':0}for label in ['copy','no_reference_match']};summary=[]
 def stage(e,attempts,minimum):
  assert e['attempts']==attempts
  if e['supported']:
   assert e['geometry_inliers']>=minimum and e['transform'] is not None and e['selected_bank'] is not None
   counts=[tuple(map(int,x))for x in re.findall(r'informative_pairs: (\d+), agreeing_pairs: (\d+)',e['pixels'])];assert len(counts)==2 and all(c>=1000 and a*10>=c*9 for c,a in counts)
   assert e['pixel_rank_radius'] in [2,8] and e['pixel_window_radius'] in [2,6]
  else:assert e['transform'] is None and e['selected_bank'] is None and e['pixels']=='None'
 for i,row in enumerate(rows):
  case=prep['plan'][i];assert row['index']==i and all(row[k]==case[k]for k in ['query','reference','published_label']);assert row['command'][5:7]==[images[case['query']],images[case['reference']]] and row['command'][8:]==['640','2.0'];assert sha(row['command'][4])==r['pins'][row['command'][4]];assert sha(row['log'])==row['log_sha256'] and Path(row['log']).read_text()==row['stdout']+row['stderr']
  if 'reused_from'in row:
   old=json.loads(Path(row['reused_from']).read_text())['results'][row['original_control_index']];assert old['command']==row['command'] and old['stdout']==row['stdout'] and old['stderr']==row['stderr']
  detail={}
  if row['returncode']:assert row['status']=='execution_failed'
  else:
   v=json.loads(row['stdout']);assert v==row['evidence'] and v['managed_used']==0;e=v['evidence'];p=e['primary'];stage(p,25,10);secondary=e['recovery']
   if p['supported']:assert secondary is None and e['supported']
   else:
    assert secondary is not None;stage(secondary['search'],9,20);assert e['supported']==secondary['search']['supported']
    assert (secondary['exposures']!='None')==secondary['search']['supported']
   assert row['status']==('supported'if e['supported']else'inconclusive');detail={'primary_supported':p['supported'],'recovery_attempted':secondary is not None,'recovery_supported':secondary['search']['supported'] if secondary else None}
  totals[case['published_label']][row['status']]+=1;summary.append(dict(case,index=i,status=row['status'],**detail))
 capture.write_bytes(raw);pins={str(p):sha(p)for p in [capture,Path(__file__),D/'dedup-disc21-exposure-recovery-api-audit-20261012.json']};report={'status':'verified_actual_composed_recovery_completed_prefix_only','completed':n,'required':24,'totals':totals,'rows':summary,'pins':pins,'source_pins_verified':len(r['pins']),'scope':'Direct public composed CPU API results, primary25/min10 then recovery9/min20 only on primary miss. Completed prefix only. Previously observed development cohort; no held-out hard-negative precision, localization/full-format/platform/RSS/all500 qualification or default adoption.'};out.write_text(json.dumps(report,indent=2)+'\n');print(n,totals)
if __name__=='__main__':main()
