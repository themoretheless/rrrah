#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');A=T/'rrrah-joint-opposite-native-qualified-20261011';OUT=D/'dedup-joint-opposite-native-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists() and not OUT.exists();A.mkdir();mp=D/'dedup-joint-opposite-native-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());ap=D/'dedup-loftr-publisher-positives-ordering-prefix-0200-audit-20261011.json';audit=json.loads(ap.read_text());capture=Path(audit['captured_report']);assert sha(capture)==audit['captured_report_sha256'];baseline=json.loads(capture.read_text());rp=D/'dedup-loftr-native-20261010-runtime-pins.json';runtime=json.loads(rp.read_text());binary=A/'joint_opposite_native_files';build=T/'dedup-joint-opposite-native-build-20261011.jsonl';entries=[json.loads(line) for line in build.read_text().splitlines()];assert entries[-1].get('reason')=='build-finished' and entries[-1]['success'];bins=[e['executable'] for e in entries if e.get('reason')=='compiler-artifact' and e.get('executable') and e['target']['name']=='joint_opposite_native_files'];assert len(bins)==1;shutil.copy2(bins[0],binary);pins={str(p):sha(p) for p in [mp,ap,capture,rp,binary,Path(__file__),build,T/'dedup-joint-opposite-native-build-20261011.log']};pins.update({p:h for p,h in runtime['pins'].items() if p.endswith('.dylib')});plan=[baseline['plan'][n] for n in [57,3,153]]
 for row in plan:
  for k in ['left','right']:pins[row[k]]=baseline['pins'][row[k]]
 assert all(sha(p)==h for p,h in pins.items());report={'status':'running','producer_pid':os.getpid(),'source_manifest':str(mp),'pins':pins,'plan':plan,'results':[],'required':3,'scope':'Joint caller-coordinate six-opposite-recipe file API on three original-resolution baseline unknowns in prefix200. Extraction skips target-only fitting; projection work is prepaid before IO. Unchanged caller-direction thresholds, no synthetic points or opposite inlier relabelling. Not automatic portfolio/collection integration or full-corpus qualification.'}
 def save():
  temp=OUT.with_suffix('.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
 save();torch=T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
 for index,row in enumerate(plan):
  assert all(sha(p)==h for p,h in pins.items());cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(torch),str(binary),row['left'],row['right'],str(row['source_tolerance'])];v={'index':index,'status':'running','command':cmd};report['results'].append(v);save();start=time.monotonic();proc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v['pid']=proc.pid;save();stdout,stderr=proc.communicate();log=A/f'{index}.log';log.write_text(stdout+stderr);v.update(returncode=proc.returncode,elapsed_seconds=time.monotonic()-start,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log))
  if proc.returncode==0:
   value=json.loads(stdout);assert value['managed_used']==0 and value['source_paths']==[row['left'],row['right']] and value['source_tolerance']==row['source_tolerance'] and value['target_tolerance']==2;v['status']='supported' if value['pixels'] and value['pixels']['supported'] is True else 'not_supported';v['managed_peak']=value['managed_peak']
  elif not stdout and 'Error: Rank(InsufficientGeometry)' in stderr:v['status']='inconclusive'
  else:v['status']='execution_failed'
  save();print(index,row['group_id'],v['status'],flush=True)
 assert all(sha(p)==h for p,h in pins.items());report['status']='complete_bounded_joint_opposite_native_controls';save()
if __name__=='__main__':main()
