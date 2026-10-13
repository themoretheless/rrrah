#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-model-bank-recovery-remaining-native-20261011.json';a=T/'rrrah-model-bank-recovery-remaining-native-qualified-20261011';assert not out.exists() and not a.exists();a.mkdir()
 parent=D/'dedup-model-bank-joint-recovery-native-20261011.json';pr=json.loads(parent.read_text());assert pr['status']=='complete_bounded_real_cpu_model_bank_joint_recovery' and all(sha(k)==v for k,v in pr['pins'].items());src=Path(pr['command'][4]);binary=a/src.name;shutil.copy2(src,binary)
 mp=D/'dedup-model-bank-joint-recovery-native-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items());gp=D/'dedup-model-bank-joint-recovery-verification-20261011.json';g=json.loads(gp.read_text());assert g['status']=='verified_134_tests_model_bank_joint_recovery' and all(sha(k)==v for k,v in g['pins'].items())
 bp=T/'rrrah-loftr-publisher-positives-qualified-20261011/audits/ordering-prefix-0458.json';b=json.loads(bp.read_text());plan=[dict(b['plan'][i],baseline_index=i,opposite_trials=trials) for i,trials in [(57,4096),(153,1000000)]];weights=T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors';rp=D/'dedup-loftr-native-20261010-runtime-pins.json';rt=json.loads(rp.read_text());libs={k:v for k,v in rt['pins'].items() if k.endswith('.dylib')};assert libs and all(sha(k)==v for k,v in libs.items());pins={str(p):sha(p) for p in [parent,src,binary,mp,gp,bp,weights,rp,Path(__file__)]};pins.update(libs)
 for row in plan:
  for key in ['left','right']:pins[row[key]]=sha(row[key])
 r={'status':'running','producer_pid':os.getpid(),'pins':pins,'plan':plan,'results':[],'required':2,'completed':0,'scope':'Actual CPU integrated bank primary25 windows/radii2/6/filter12/1m then joint opposite-native filter8, original reverse204101→204100 at4096 trials and207701→207700 at1m. Same134-test frozen pair executable as qualified211900; not combined collection/full458/semantic negatives/all500/RSS cap/speedup qualification.'}
 def save():
  temp=out.with_suffix('.tmp');temp.write_text(json.dumps(r,indent=2,ensure_ascii=False)+'\n');temp.replace(out)
 save()
 for index,row in enumerate(plan):
  assert all(sha(k)==v for k,v in pins.items());cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),str(binary),row['left'],row['right'],str(weights),'640',str(row['source_tolerance']),str(row['opposite_trials'])];v={'index':index,'baseline_index':row['baseline_index'],'status':'running','command':cmd};r['results'].append(v);save();start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v['pid']=p.pid;save();stdout,stderr=p.communicate();log=a/f'{index}.log';log.write_text(stdout+stderr);v.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start,log=str(log),log_sha256=sha(log));v['status']='execution_failed'
  if p.returncode==0:
   value=json.loads(stdout);assert value['managed_used']==0;v['status']='supported' if value['supported'] is True else ('rejected' if value['supported'] is False else 'inconclusive')
  assert all(sha(k)==v for k,v in pins.items());r['completed']=index+1;save();print(row['baseline_index'],v['status'],flush=True)
 r['status']='complete_bounded_actual_cpu_remaining_recovery_controls';save()
if __name__=='__main__':main()
