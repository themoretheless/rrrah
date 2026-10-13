#!/usr/bin/env python3
"""Complete458-direction publisher positives on the frozen bank collection path.
Seven previously unresolved directions first; every original plan row retained.
"""
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-model-bank-all-positives-20261011.json';a=T/'rrrah-model-bank-all-positives-qualified-20261011';assert not out.exists() and not a.exists();a.mkdir()
 mp=D/'dedup-model-bank-collection-native-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items())
 hp=D/'dedup-model-bank-outcome-native-harness-20261011.json';h=json.loads(hp.read_text());assert all(sha(k)==v for k,v in h['pins'].items())
 gp=D/'dedup-model-bank-collection-verification-20261011.json';g=json.loads(gp.read_text());assert g['status']=='verified_110_tests_model_bank_collection_routing' and all(sha(k)==v for k,v in g['pins'].items())
 build=T/'dedup-model-bank-outcome-native-build-20261011.jsonl';entries=[json.loads(x) for x in build.read_text().splitlines()];assert entries[-1]['reason']=='build-finished' and entries[-1]['success'];bins=[e['executable'] for e in entries if e.get('reason')=='compiler-artifact' and e.get('target',{}).get('name')=='model_bank_outcome_files' and e.get('executable')];assert len(bins)==1;src=Path(bins[0]);binary=a/src.name;shutil.copy2(src,binary)
 bp=T/'rrrah-loftr-publisher-positives-qualified-20261011/audits/ordering-prefix-0458.json';b=json.loads(bp.read_text());assert len(b['plan'])==458;priority=[3,7,27,57,67,153,255];indices=priority+[i for i in range(458) if i not in priority];plan=[dict(b['plan'][i],baseline_index=i) for i in indices];assert sorted(x['baseline_index'] for x in plan)==list(range(458))
 weights=T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors';runtime=D/'dedup-loftr-native-20261010-runtime-pins.json';rt=json.loads(runtime.read_text());libs={k:v for k,v in rt['pins'].items() if k.endswith('.dylib')};assert libs and all(sha(k)==v for k,v in libs.items())
 paths=[mp,hp,gp,build,T/'dedup-model-bank-outcome-native-build-20261011.log',src,binary,bp,weights,runtime,Path(__file__)];pins={str(p):sha(p) for p in paths};pins.update(libs)
 for row in plan:
  for key in ['left','right']:pins[row[key]]=sha(row[key])
 r={'status':'running','producer_pid':os.getpid(),'pins':pins,'plan':plan,'required':458,'completed':0,'results':[],'scope':'All229 publisher positives in both directions on one frozen current bank collection core, ModelFirst, all25 banks/radii2/6/filter12/1m, original files, typed unknown preserved. Seven prior unresolved directions prioritized without pruning. Tensors outside managed credit, shared host timings; no negative precision/full formats/platforms/all500/RSS cap claim.'}
 def save():
  temp=out.with_suffix('.tmp');temp.write_text(json.dumps(r,indent=2,ensure_ascii=False)+'\n');temp.replace(out)
 save()
 for index,row in enumerate(plan):
  for p in [row['left'],row['right'],str(binary),str(weights)]:assert sha(p)==pins[p]
  cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),str(binary),row['left'],row['right'],str(weights),'640',str(row['source_tolerance'])];v={'index':index,'baseline_index':row['baseline_index'],'status':'running','command':cmd};r['results'].append(v);save();start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v['pid']=p.pid;save();stdout,stderr=p.communicate();log=a/f'{index:04d}.log';log.write_text(stdout+stderr);v.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start,log=str(log),log_sha256=sha(log))
  if p.returncode:v['status']='execution_failed'
  else:
   value=json.loads(stdout);assert value['files']==[2,9] and len(value['pairs'])==1 and value['managed_used']==0;pair=value['pairs'][0];assert (pair['left'],pair['right'])==(2,9);support=pair['support'];v.update(evidence=pair,managed_peak=value['managed_peak'],rss_bytes=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1]));v['status']='publisher_positive_supported' if support is True else ('publisher_positive_rejected' if support is False else 'publisher_positive_inconclusive')
  for p in [row['left'],row['right'],str(binary),str(weights)]:assert sha(p)==pins[p]
  r['completed']=index+1;save();print(index,row['baseline_index'],v['status'],flush=True)
 assert all(sha(k)==v for k,v in pins.items());r['status']='complete_bounded_publisher_positive_execution';save()
if __name__=='__main__':main()
