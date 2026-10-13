#!/usr/bin/env python3
"""Independent same-source full opt-level3 build including dependencies and binary; policy-equivalent corpus unknown replay. Both earlier runs remain intact."""
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-outcome-collection-native-fullopt3-20261011.json'
ARCHIVE=BASE/'outcome-collection-native-fullopt3-qualified-20261011'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
 mp=D/'dedup-outcome-collection-snapshot-20261011.json';m=json.loads(mp.read_text());snapshot=Path(m['snapshot'])
 assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items())
 binary=ARCHIVE/'tile_outcome_collection_files';source=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-exact-portfolio-20261011/debug')/binary.name
 shutil.copy2(source,binary);assert sha(source)==sha(binary) and str(snapshot).encode() in binary.read_bytes()
 a=BASE/'original-resolution-negative-200201/202300.jpg';b=BASE/'original-resolution-strong-all/200001.jpg'
 weights=BASE/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
 raw=BASE/'current-all-query-origins-qualified-20261011/00023.json.gz'
 runtime=D/'dedup-loftr-native-20261010-runtime-pins.json'
 torch='/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
 pins={str(p):sha(p) for p in [mp,Path(__file__),binary,a,b,weights,raw,runtime,D/'dedup-outcome-collection-native-fullopt3-isolated-build-20261011.log']}
 artifacts=D/'dedup-outcome-collection-native-fullopt3-isolated-build-artifacts-20261011.jsonl'
 rows=[json.loads(line) for line in artifacts.read_text().splitlines()]
 own=[row for row in rows if row.get('reason')=='compiler-artifact' and row['target']['name']=='rrrah_dedup']
 assert len(own)==1 and own[0]['profile']['opt_level']=='3' and str(snapshot) in own[0]['package_id']
 assert all(row['profile']['opt_level']=='3' for row in rows if row.get('reason')=='compiler-artifact' and not set(row['target']['kind']) & {'custom-build','proc-macro'})
 pins[str(artifacts)]=sha(artifacts)
 pins.update({p:h for p,h in json.loads(runtime.read_text())['pins'].items() if torch in p})
 def verify():assert all(sha(p)==h for p,h in pins.items())
 missing=ARCHIVE/'missing-weights';assert not missing.exists()
 cases=[('corpus_unknown',weights,'3.4214785442741977',[a,b]),('self_support',weights,'2',[a,a]),('missing_weights',missing,'2',[a,a])]
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'results':[],'source_manifest':str(mp),'scope':__doc__}
 def save():
  tmp=OUT.with_suffix('.json.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(OUT)
 save()
 try:
  for name,w,tolerance,paths in cases:
   verify();cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+torch,str(binary),str(w),'640',tolerance,'model-first',*map(str,paths)]
   row={'case':name,'status':'running','command':cmd};report['results'].append(row);save();start=time.monotonic()
   child=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=child.pid;save();stdout,stderr=child.communicate()
   row.update(returncode=child.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start)
   log=ARCHIVE/(name+'.log');log.write_text(stdout+stderr);row.update(log=str(log),log_sha256=sha(log))
   if name=='missing_weights':assert child.returncode!=0 and not stdout.strip() and 'No such file' in stderr
   else:
    assert child.returncode==0,stderr;e=json.loads(stdout);row['evidence']=e
    assert e['managed_used']==0
    pairs=e['pairs'];assert len(pairs)==1 and pairs[0]['left']==0 and pairs[0]['right']==1
    evidence=pairs[0]['evidence']
    if name=='corpus_unknown':
     assert evidence['terminal']=='inconclusive' and evidence['supported'] is None
     assert 'ModelTiles { attempts: 25, insufficient_attempts: 1,' in evidence['reason']
    else:assert evidence['supported'] is True
   verify();row['status']='passed';save();print(name,'passed',flush=True)
  assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items())
  report.update(status='complete_bounded_policy_equivalent_corpus_unknown_replay',invocations=3)
 except Exception as error:report.update(status='failed',error=str(error));raise
 finally:save()
if __name__=='__main__':main()
