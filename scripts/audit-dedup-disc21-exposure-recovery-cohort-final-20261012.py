#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-exposure-recovery-cohort-final-audit-20261012.json';assert not out.exists();rp=D/'dedup-disc21-exposure-recovery-cohort-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_fixed_exposure_recovery_cohort' and len(r['results'])==r['required']==24;assert all(sha(p)==h for p,h in r['pins'].items())
 prefix=D/'dedup-disc21-exposure-recovery-cohort-prefix-0024-audit-20261012.json';a=json.loads(prefix.read_text());assert a['completed']==24 and all(sha(p)==h for p,h in a['pins'].items());assert a['totals']=={'copy':{'supported':8,'inconclusive':0,'execution_failed':0},'no_reference_match':{'supported':0,'inconclusive':16,'execution_failed':0}}
 trace=[];reused=[]
 for row in r['results']:
  v=json.loads(row['stdout']);e=v['evidence'];p=e['primary'];secondary=e['recovery'];assert v['managed_used']==0
  if p['supported']:assert secondary is None
  else:assert secondary is not None
  trace.append({'query':row['query'],'reference':row['reference'],'label':row['published_label'],'supported':e['supported'],'primary_supported':p['supported'],'recovery_attempted':secondary is not None,'recovery_supported':secondary['search']['supported']if secondary else None})
  if 'reused_from'in row:reused.append(row['query'])
 assert sum(x['primary_supported']for x in trace)==7;assert sum(x['recovery_attempted']for x in trace)==17;assert [x['query']for x in trace if x['recovery_supported']]==['Q00906'];assert sorted(reused)==['Q00000','Q00061','Q00906']
 report={'status':'verified_complete_fixed24_public_composed_cpu_cohort','completed':24,'positives_supported':8,'positives_inconclusive':0,'negatives_supported':0,'negatives_inconclusive':16,'execution_failures':0,'primary_supported':7,'recovery_attempted':17,'recovery_supported':1,'exact_audited_controls_reused':reused,'trace':trace,'pins':{str(p):sha(p)for p in [rp,prefix,Path(__file__)]},'scope':'Direct public composed CPU API on complete precommitted fixed24 development plan, with3identical audited controls reused after input/binary/argument pins. Primary retains7positives, recovery supports Q00906, all16distractors remain inconclusive. Image-level local support, not all-pair localization/identity, held-out semantic/burst precision, default portfolio/collection integration, full458/formats/platforms/RSS/all500 qualification.'};out.write_text(json.dumps(report,indent=2)+'\n');print(report['status'])
if __name__=='__main__':main()
