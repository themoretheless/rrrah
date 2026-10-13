#!/usr/bin/env python3
"""Execute fixed architecture noncopy pairs through both collection directions."""
import hashlib,json,os,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-bidirectional-noncopy-20261012.json';assert not out.exists()
 parent=D/'dedup-disc21-architecture-controls-prepared-20261012.json';a=json.loads(parent.read_text());assert all(sha(p)==v for p,v in a['pins'].items());images={x['id']:x['path']for x in a['images']};assert all(sha(x['path'])==x['sha256']for x in a['images'])
 base=D/'dedup-disc21-bidirectional-exposure-collection-20261012.json';b=json.loads(base.read_text());assert b['build_returncode']==0 and b['status']in ['running','complete_bidirectional_collection_control'];assert all(sha(p)==v for p,v in b['pins'].items())
 command=b['command'];assert Path(command[4]).name=='disc21_bidirectional_exposure_collection'
 plan=[{'source':x,'target':y,'expected':'inconclusive_both_directions'}for x,y in [('R014120','R020965'),('Q00061','R014120'),('Q00906','R020965')]]
 prep=D/'dedup-disc21-bidirectional-noncopy-prepared-20261012.json';assert not prep.exists();pins=dict(b['pins']);pins.update({str(p):sha(p)for p in [parent,Path(__file__)]});pins.update({x['path']:x['sha256']for x in a['images']})
 prepared={'status':'prepared_three_unordered_development_noncopy_pairs_six_directions','plan':plan,'pins':pins,'scope':a['scope']+' Same fixed ordinary25/calibrated9 thresholds as positive control; no case tuning.'};prep.write_text(json.dumps(prepared,indent=2)+'\n');pins[str(prep)]=sha(prep)
 r={'status':'running','producer_pid':os.getpid(),'prepared':str(prep),'plan':plan,'completed':0,'pins':pins,'results':[],'scope':prepared['scope']}
 def save():p=out.with_suffix('.tmp');p.write_text(json.dumps(r,indent=2)+'\n');p.replace(out)
 save()
 try:
  for i,row in enumerate(plan):
   assert all(sha(p)==v for p,v in pins.items());cmd=command[:5]+[images[row['source']],images[row['target']]]+command[7:];start=time.monotonic();proc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);r['child_pid']=proc.pid;r['active_index']=i;save();stdout,stderr=proc.communicate();log=T/f'dedup-disc21-bidirectional-noncopy-{i:04d}-20261012.log';assert not log.exists();log.write_text(stdout+stderr);result={'index':i,'command':cmd,'returncode':proc.returncode,'stdout':stdout,'stderr':stderr,'elapsed_seconds':time.monotonic()-start,'log':str(log),'log_sha256':sha(log)};r['results'].append(result);r['completed']=i+1;save();assert proc.returncode==0
   v=json.loads(stdout);assert v['managed_used']==0 and v['managed_report_used']>0;pair=v['evidence']['pairs'][0];assert not pair['supported'] and pair['reverse']is not None and not pair['forward']['supported']and not pair['reverse']['supported']
  assert all(sha(p)==v for p,v in pins.items());r['status']='complete_three_unordered_noncopy_pairs';save();print('Completed three noncopy pairs / six directed searches')
 except Exception as e:r.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
