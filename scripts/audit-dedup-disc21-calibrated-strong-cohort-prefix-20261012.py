#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 path=D/'dedup-disc21-calibrated-strong-cohort-20261011.json';raw=path.read_bytes();r=json.loads(raw);rows=[x for x in r['results']if x['status']!='running'];n=len(rows);assert n and rows==r['results'][:n];out=D/f'dedup-disc21-calibrated-strong-cohort-prefix-{n:04d}-audit-20261012.json';capture=T/f'rrrah-disc21-calibrated-strong-prefix-{n:04d}-20261012.json';assert not out.exists() and not capture.exists();assert all(sha(p)==h for p,h in r['pins'].items());prep=json.loads((D/'dedup-disc21-cohort-prepared-20261011.json').read_text());assert r['plan']==prep['plan'] and r['required']==24;images={x['id']:x['path']for x in prep['images']};summary=[];totals={label:{'supported':0,'inconclusive':0,'execution_failed':0}for label in ['copy','no_reference_match']}
 for i,row in enumerate(rows):
  case=prep['plan'][i];assert row['index']==i and all(row[k]==case[k]for k in ['query','reference','published_label']);assert row['command'][5:7]==[images[case['query']],images[case['reference']]] and row['command'][8:]==['640','2.0'];assert sha(row['command'][4])==r['pins'][row['command'][4]];assert sha(row['log'])==row['log_sha256'] and Path(row['log']).read_text()==row['stdout']+row['stderr']
  if 'reused_from'in row:
   old=json.loads(Path(row['reused_from']).read_text())['results'][row['original_control_index']];assert old['command']==row['command'] and old['stdout']==row['stdout'] and old['stderr']==row['stderr']
  if row['returncode']:assert row['status']=='execution_failed'
  else:
   v=json.loads(row['stdout']);assert v==row['evidence'] and v['managed_used']==0;e=v['evidence'];assert e['bank_count']==9;assert row['status']==('supported'if e['supported']else'inconclusive');accepted=[]
   for attempt in e['pixel_attempts']:
    assert len(attempt['witnesses'])>=20
    if attempt['pixels']=='insufficient_geometry':assert not attempt['supported'];continue
    counts=[tuple(map(int,x))for x in re.findall(r'informative_pairs: (\d+), agreeing_pairs: (\d+)',attempt['pixels'])];assert len(counts)==2;assert attempt['supported']==all(n>=1000 and a/n>=0.9 for n,a in counts)
    if attempt['supported']:accepted.append(attempt)
   assert bool(accepted)==e['supported']
  totals[case['published_label']][row['status']]+=1;summary.append(dict(case,index=i,status=row['status']))
 capture.write_bytes(raw);pins={str(p):sha(p)for p in [capture,Path(__file__),D/'dedup-disc21-overlap-calibrated-strong-audit-20261011.json']};report={'status':'verified_fixed_calibrated_strong_completed_prefix_only','completed':n,'required':24,'totals':totals,'rows':summary,'pins':pins,'source_pins_verified':len(r['pins']),'scope':'Separate min20 calibrated-overlap candidate; no merge with native/rank2 recall. Previously observed development cohort, not independent hard-negative qualification. First Q00061 remains inconclusive, so this recipe cannot replace earlier rank2 search. Unfinished suffix not qualified; image-level supports need separate localization evidence.'};out.write_text(json.dumps(report,indent=2)+'\n');print(n,totals)
if __name__=='__main__':main()
