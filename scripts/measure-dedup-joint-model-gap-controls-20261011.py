#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-joint-model-gap-controls-20261011.json';A=T/'rrrah-joint-model-gap-controls-qualified-20261011';assert not out.exists() and not A.exists();A.mkdir();audit=D/'dedup-joint-model-tiles-native-fixed-audit-20261011.json';a=json.loads(audit.read_text());assert all(sha(p)==h for p,h in a['pins'].items());src=T/'rrrah-joint-model-tiles-native-fixed-qualified-20261011/joint_model_tiles_files';binary=A/'joint_model_tiles_files';shutil.copy2(src,binary);bp=T/'rrrah-loftr-publisher-positives-qualified-20261011/audits/ordering-prefix-0200.json';b=json.loads(bp.read_text());weights=T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors';plan=[b['plan'][i] for i in [67,7]];pins={str(p):sha(p) for p in [audit,src,binary,bp,weights,Path(__file__)]}
 for row in plan:
  for key in ['left','right']:pins[row[key]]=sha(row[key])
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'plan':plan,'results':[],'scope':'Two unresolved publisher-positive directions through actual CPU joint25-window search,1m hypotheses/window and fixed filter12, original residual/pixel floors unchanged. Missing support is inconclusive, not semantic rejection. No full portfolio/precision/corpus/default claim.'}
 def save():out.write_text(json.dumps(report,indent=2)+'\n')
 save()
 for row in plan:
  assert all(sha(p)==h for p,h in pins.items());v={'group_id':row['group_id'],'status':'running'};report['results'].append(v);save();cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),str(binary),row['left'],row['right'],str(weights),'640',str(row['source_tolerance'])];start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v.update(command=cmd,pid=p.pid);save();stdout,stderr=p.communicate();log=A/(str(row['group_id'])+'.log');log.write_text(stdout+stderr);v.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start,log=str(log),log_sha256=sha(log));save()
  if p.returncode==0:
   value=json.loads(stdout);assert value['managed_used']==0;v['status']='supported' if value['supported'] else 'inconclusive';v['attempts']=value['attempts']
  else:
   assert p.returncode==1 and not stdout and stderr.startswith('Error: InsufficientAnchorGeometry(ModelTileEvidence'),stderr[:500];assert 'len: 25, model_digest:' in stderr;assert stderr.count('ModelTileAttempt {')==25;v['status']='typed_inconclusive';v['attempts']=25
  save();print(row['group_id'],v['status'],flush=True)
 assert all(sha(p)==h for p,h in pins.items());report['status']='complete_bounded_joint_model_gap_controls';save()
if __name__=='__main__':main()
