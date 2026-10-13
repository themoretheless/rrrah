#!/usr/bin/env python3
import hashlib,json,math,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');B=T/'rrrah-dedup-copydays';A=T/'rrrah-scheduled-loftr-original-positive-qualified-20261011';OUT=D/'dedup-scheduled-loftr-original-positive-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists() and not OUT.exists();A.mkdir();rp=D/'dedup-scheduled-loftr-native-runtime-20261011.json';parent=json.loads(rp.read_text());assert parent['status']=='complete_bounded_scheduled_loftr_native_controls';assert all(sha(p)==h for p,h in parent['pins'].items())
 corpus=B/'prepared.json';data=json.loads(corpus.read_text());original=next(r for r in data['images']['original'] if r['filename']=='200000.jpg');query=next(r for r in data['images']['strong'] if r['filename']=='200001.jpg');assert original['group_id']==query['group_id']
 left=B/'original-resolution-negative-200201'/original['filename'];right=B/'original-resolution-strong-all'/query['filename'];assert sha(left)==original['source_sha256'] and sha(right)==query['source_sha256']
 source=T/'rrrah-scheduled-loftr-runtime-qualified-20261011/scheduled_outcome_collection_files';binary=A/source.name;shutil.copy2(source,binary);assert sha(binary)==sha(source)
 weights=B/'learned-matching-models/loftr-outdoor-native-20261010.safetensors';torch=T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib';pins={str(p):sha(p) for p in [rp,Path(__file__),corpus,left,right,binary,weights]};pins.update(parent['pins']);report={'status':'running','producer_pid':os.getpid(),'source_manifest':parent['source_manifest'],'pins':pins,'results':[],'scope':'Original-resolution publisher positive 200000/200001, both directions, max-side640 model, empty proposals forcing fallback, source tolerance follows existing corpus formula. Not full corpus qualification or hard RSS.'}
 def save():
  p=OUT.with_suffix('.tmp');p.write_text(json.dumps(report,indent=2)+'\n');p.replace(OUT)
 save()
 try:
  for name,a,b,ad,bd in [('forward',left,right,original['source_size'],query['source_size']),('reverse',right,left,query['source_size'],original['source_size'])]:
   assert all(sha(p)==h for p,h in pins.items());tolerance=2*math.hypot(*ad)/math.hypot(*bd);cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(torch),str(binary),str(weights),'640',str(tolerance),'model-first','-',str(a),str(b)];row={'case':name,'status':'running','command':cmd,'source_tolerance':tolerance};report['results'].append(row);save();start=time.monotonic();proc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['pid']=proc.pid;save();stdout,stderr=proc.communicate();log=A/(name+'.log');log.write_text(stdout+stderr);row.update(returncode=proc.returncode,elapsed_seconds=time.monotonic()-start,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log));save();assert proc.returncode==0,stderr
   value=json.loads(stdout);assert value['status']=='ok' and value['proposal_pairs']==[] and value['source_paths']==[str(a),str(b)] and value['files']==[0,1] and len(value['pairs'])==1;pair=value['pairs'][0];assert (pair['left'],pair['right'])==(0,1);assert pair['evidence']['supported'] is True;assert value['managed_used']==0 and value['managed_peak']<=512*1024*1024;row.update(status='passed_publisher_positive',terminal=pair['evidence']['terminal'],managed_peak=value['managed_peak']);save()
  assert all(sha(p)==h for p,h in pins.items());report['status']='complete_bounded_original_positive_scheduled_loftr_controls';save();print(report['status'])
 except Exception as e:report.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
