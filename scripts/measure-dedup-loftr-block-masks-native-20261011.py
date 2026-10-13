#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');A=T/'rrrah-loftr-block-masks-native-qualified-20261011';OUT=D/'dedup-loftr-block-masks-native-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists() and not OUT.exists();A.mkdir();mp=D/'dedup-loftr-block-masks-native-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());ap=D/'dedup-loftr-publisher-positives-ordering-prefix-0200-audit-20261011.json';audit=json.loads(ap.read_text());capture=Path(audit['captured_report']);assert sha(capture)==audit['captured_report_sha256'];baseline=json.loads(capture.read_text());rp=D/'dedup-loftr-native-20261010-runtime-pins.json';runtime=json.loads(rp.read_text());binary=A/'joint_recovery_collection_files';shutil.copy2(T/'rrrah-dedup-exact-portfolio-20261011/debug/joint_recovery_collection_files',binary)
 build=T/'dedup-loftr-block-masks-native-build-20261011.jsonl';entries=[json.loads(line) for line in build.read_text().splitlines()];assert any(e.get('reason')=='build-finished' and e['success'] for e in entries);artifact=next(e for e in entries if e.get('reason')=='compiler-artifact' and e['target']['name']=='joint_recovery_collection_files');assert sha(artifact['executable'])==sha(binary)
 weights=T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors';normalized=T/'rrrah-dedup-copydays/normalized/original';plan=[]
 for index in [57]:
  row=baseline['plan'][index];plan.append({'case':f"original_reverse_{row['group_id']}",'args':[str(weights),'640',str(row['source_tolerance']),'model-first','-',row['left'],row['right']],'baseline_index':index,'baseline_status':baseline['results'][index]['status'],'expected':'supported' if index in [57,3] else 'observe'})
 pins={str(p):sha(p) for p in [mp,ap,capture,rp,binary,Path(__file__),build,T/'dedup-loftr-block-masks-native-build-20261011.log',weights]};pins.update({p:h for p,h in runtime['pins'].items() if p.endswith('.dylib')})
 for row in plan:
  for f in row['args'][5:]:pins[f]=sha(f)
 assert all(sha(p)==h for p,h in pins.items());report={'status':'running','producer_pid':os.getpid(),'source_manifest':str(mp),'pins':pins,'plan':plan,'results':[],'required':len(plan),'scope':'Blocked-mask CPU LoFTR candidate: one original-resolution reverse204100 combined recovery at unchanged thresholds, for comparison with separately captured pre-change pipeline. Not full-corpus, precision/scalability/platform/RSS-cap qualification. Untagged-linear-sRGB diagnostic assumption.'}
 def save():
  temp=OUT.with_suffix('.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
 save();torch=T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
 for index,row in enumerate(plan):
  assert all(sha(p)==h for p,h in pins.items());cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(torch),str(binary),*row['args']];v={'index':index,'case':row['case'],'status':'running','command':cmd};report['results'].append(v);save();start=time.monotonic();proc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v['pid']=proc.pid;save();stdout,stderr=proc.communicate();log=A/f'{index}.log';log.write_text(stdout+stderr);v.update(returncode=proc.returncode,elapsed_seconds=time.monotonic()-start,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log))
  if row['expected']=='source_failure':
   assert proc.returncode!=0 and not stdout and 'ModelSource' in stderr,stderr;v['status']='passed_source_failure'
  else:
   assert proc.returncode==0,stderr;value=json.loads(stdout);assert value['managed_used']==0 and value['source_paths']==row['args'][5:];pairs=value['pairs'];assert [(p['left'],p['right']) for p in pairs]==([(0,1),(0,2),(1,2)] if row['expected']=='complete_three_pairs' else [(0,1)])
   if row['expected']=='supported':
    e=pairs[0]['evidence'];assert e['terminal']=='joint_opposite_native' and e['supported'] is True and e['primary_insufficient']!='None';assert min(e['pixels']['anchors'])>=10;assert e['candidate_points']>=10
   elif row['expected']=='observe':
    e=pairs[0]['evidence'];assert e['supported'] in [None,True,False];v['observed_terminal']=e['terminal'];v['observed_support']=e['supported']
   else:assert pairs[0]['evidence']['supported'] is True
   v['status']='passed_bounded_control';v['managed_peak']=value['managed_peak']
  save();print(index,row['case'],v['status'],flush=True)
 assert all(sha(p)==h for p,h in pins.items());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());report['status']='complete_bounded_native_joint_recovery_collection_controls';save()
if __name__=='__main__':main()
