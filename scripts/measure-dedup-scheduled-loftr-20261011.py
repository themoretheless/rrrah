#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');BASE=T/'rrrah-dedup-copydays';A=T/'rrrah-scheduled-loftr-qualified-20261011';OUT=D/'dedup-scheduled-loftr-native-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists() and not OUT.exists();A.mkdir();mp=D/'dedup-scheduled-loftr-snapshot-20261011.json';m=json.loads(mp.read_text());snapshot=Path(m['snapshot']);assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items())
 source=T/'rrrah-dedup-exact-portfolio-20261011/debug/scheduled_outcome_collection_files';binary=A/source.name;digest=sha(source);shutil.copy2(source,binary);assert sha(binary)==digest and str(snapshot).encode() in binary.read_bytes()
 artifacts=D/'dedup-scheduled-loftr-build-artifacts-20261011.jsonl';compiled=[]
 for line in artifacts.read_text().splitlines():
  try:r=json.loads(line)
  except json.JSONDecodeError:continue
  if r.get('reason')=='compiler-artifact':compiled.append(r)
 for name in ['rrrah_dedup','loftr','tch','scheduled_outcome_collection_files','blake3']:
  found=[r for r in compiled if r['target']['name']==name];assert found and all(r['profile']['opt_level']=='3' for r in found)
 own=[r for r in compiled if r['target']['name']=='rrrah_dedup'];assert len(own)==1 and str(snapshot) in own[0]['package_id']
 generic_path=D/'dedup-scheduled-portfolio-audit-20261011.json';generic=json.loads(generic_path.read_text());assert generic['status']=='complete_bounded_scheduled_portfolio_generic_gate'
 for relative in ['crates/rrrah-dedup/src/collection.rs','crates/rrrah-dedup/src/portfolio_collection.rs','crates/rrrah-dedup/src/pair_schedule.rs']:
  archived=T/'rrrah-scheduled-portfolio-qualified-20261011/source'/relative;assert sha(archived)==generic['pins'][str(archived)]==sha(snapshot/relative)
 corpus=BASE/'prepared.json';data=json.loads(corpus.read_text());originals=sorted(data['images']['original'],key=lambda x:x['filename']);a=Path(originals[0]['normalized_path']);b=Path(originals[1]['normalized_path']);assert sha(a)==originals[0]['normalized_sha256'] and sha(b)==originals[1]['normalized_sha256']
 weights=BASE/'learned-matching-models/loftr-outdoor-native-20261010.safetensors';runtime=D/'dedup-loftr-native-20261010-runtime-pins.json';torch=T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib';pins={str(p):sha(p) for p in [mp,Path(__file__),binary,artifacts,D/'dedup-scheduled-loftr-build-20261011.log',corpus,a,b,weights,runtime,generic_path]};pins.update({p:h for p,h in json.loads(runtime.read_text())['pins'].items() if str(torch) in p})
 def verify():assert all(sha(p)==h for p,h in pins.items())
 cases=[('proposed_other_self_fallback',weights,'1:2',[a,a,b],[(0,1),(0,2),(1,2)]),('no_hints_self',weights,'-',[a,a],[(0,1)]),('missing_weights',A/'missing-weights','-',[a,a],[])]
 report={'status':'running','producer_pid':os.getpid(),'source_manifest':str(mp),'pins':pins,'results':[],'scope':'CPU LoFTR with normalized max-side320 inputs, model max-side160 and untagged-linear assumption; proposal-first exhaustive fallback controls. Self-support required, other-origin outputs recorded without semantic-negative claims. No full corpus or hard RSS qualification.'}
 def save():
  p=OUT.with_suffix('.tmp');p.write_text(json.dumps(report,indent=2)+'\n');p.replace(OUT)
 save()
 try:
  for name,w,hints,paths,pairs in cases:
   verify();cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(torch),str(binary),str(w),'160','2','model-first',hints,*map(str,paths)];row={'case':name,'status':'running','command':cmd};report['results'].append(row);save();start=time.monotonic();proc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['pid']=proc.pid;save();stdout,stderr=proc.communicate();log=A/(name+'.log');log.write_text(stdout+stderr);row.update(returncode=proc.returncode,elapsed_seconds=time.monotonic()-start,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log));save()
   if name=='missing_weights':assert proc.returncode!=0 and not stdout.strip() and ('No such file' in stderr or 'not found' in stderr)
   else:
    assert proc.returncode==0,stderr;value=json.loads(stdout);assert value['status']=='ok' and value['files']==list(range(len(paths))) and value['source_paths']==list(map(str,paths));assert [(p['left'],p['right']) for p in value['pairs']]==pairs
    assert value['proposal_pairs']==([[1,2]] if hints=='1:2' else []);assert value['managed_used']==0 and value['managed_peak']<=512*1024*1024
    for pair in value['pairs']:
     evidence=pair['evidence'];assert evidence['supported'] is None or isinstance(evidence['supported'],bool)
     if evidence['supported'] is None:assert evidence['terminal']=='inconclusive' and evidence['reason']
    assert value['pairs'][0]['evidence']['supported'] is True
    row['decisions']=[{'left':p['left'],'right':p['right'],'supported':p['evidence']['supported'],'terminal':p['evidence']['terminal']} for p in value['pairs']]
   row['status']='passed_bounded_control';save()
  verify();assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items());report['status']='complete_bounded_scheduled_loftr_native_controls';save();print(report['status'])
 except Exception as e:report.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
