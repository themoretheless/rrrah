#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 prepared=D/'dedup-loftr-selected-windows-real-inputs-20261012.json';p=json.loads(prepared.read_text());assert all(sha(f)==h for f,h in p['pins'].items());sourceprobe=R/'scripts/dedup-loftr-native-probe/src/main.rs';code=sourceprobe.read_text();code=code.replace('if count > 5000 {','if count == 0 || count > 5000 {').replace('"confidence":confidence}', '"confidence":confidence,"batch_indexes":Vec::<i64>::try_from(&result.batch_indexes)?}')
 assert '"batch_indexes"' in code
 code=code.replace('Vec::<Vec<f32>>','Vec::<Vec<f64>>').replace('Vec::<f32>','Vec::<f64>').replace('"batch_indexes":','"kinds":[format!("{:?}",result.keypoints0.kind()),format!("{:?}",result.keypoints1.kind()),format!("{:?}",result.confidence.kind())],"batch_indexes":')
 weights=T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors';runtime=T/'rrrah-loftr-native-runtime-20261010';env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH'];pins={str(f):sha(f) for f in [prepared,sourceprobe,weights,Path(__file__)]};out=D/'dedup-loftr-selected-windows-real-model-v2-20261012.json';assert not out.exists();r={'status':'building','producer_pid':os.getpid(),'pins':pins,'builds':[],'results':[],'scope':'Current root LoFTR baseline versus v2 selected-window candidate, fixed real Copydays grayscale inputs, both directions, full public model.forward with pretrained weights. Nonempty raw proposals required. Coordinates/confidences serialized through f64 to retain original Float/Double precision; tensor kinds captured. Not original-pixel duplicate confirmation, recall/fullcorpus/platform/full500/hardcap or isolated timing qualification. Not adopted.'}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 save()
 try:
  for mode,name in [('baseline','dedup-loftr-selected-windows-current-baseline-20261012.json'),('selected','dedup-loftr-selected-windows-source-v2-20261012.json')]:
   mp=D/name;m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());pins[str(mp)]=sha(mp);h=T/f'rrrah-loftr-selected-windows-real-model-v2-{mode}-20261012';assert not h.exists();(h/'src/bin').mkdir(parents=True);binary='selected_window_model_'+mode;probe=h/'src/bin'/f'{binary}.rs';probe.write_text(code)
   (h/'Cargo.toml').write_text('[package]\nname="selected-window-model-control"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\nloftr={path="'+str(Path(m['snapshot'])/'vendor/loftr')+'"}\ntch="=0.23.0"\nserde_json="1"\n[profile.dev]\nopt-level=3\n')
   lock=subprocess.run(['cargo','generate-lockfile','--offline','--manifest-path',str(h/'Cargo.toml')],cwd='/tmp',env=env,capture_output=True,text=True);assert lock.returncode==0,lock.stderr
   for f in [h/'Cargo.toml',h/'Cargo.lock',probe]:pins[str(f)]=sha(f)
   row={'mode':mode,'status':'building'};r['builds'].append(row);log=T/f'rrrah-loftr-selected-windows-real-model-v2-{mode}-build-20261012.log';cmd=['cargo','build','--locked','--offline','--manifest-path',str(h/'Cargo.toml'),'--bin',binary,'--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')]
   with log.open('w') as f:
    child=subprocess.Popen(cmd,cwd='/tmp',env=env,stdout=f,stderr=subprocess.STDOUT);row.update(child_pid=child.pid,command=cmd);save();rc=child.wait()
   row['returncode']=rc;pins[str(log)]=sha(log);save();assert rc==0
   exe=T/f'rrrah-loftr-selected-windows-real-model-v2-{mode}-executable-20261012';assert not exe.exists();shutil.copy2(T/'rrrah-dedup-exact-portfolio-20261011/debug'/binary,exe);pins[str(exe)]=sha(exe);row.update(status='compiled',executable=str(exe));save();print('Built',mode,flush=True)
  r['status']='running';save();exes={b['mode']:b['executable'] for b in r['builds']}
  for i,(mode,direction) in enumerate([('baseline',0),('selected',0),('selected',1),('baseline',1)]):
   left,right=p['images'][::1 if direction==0 else -1];cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(runtime/'lib/python3.12/site-packages/torch/lib'),exes[mode],str(weights),left['raw'],*map(str,left['dimensions']),right['raw'],*map(str,right['dimensions'])];row={'mode':mode,'direction':direction,'status':'running','command':cmd};r['results'].append(row);log=T/f'rrrah-loftr-selected-windows-real-model-v2-{i:02d}-20261012';stdout=log.with_suffix('.stdout');stderr=log.with_suffix('.stderr');start=time.monotonic()
   with stdout.open('w') as o,stderr.open('w') as e:
    child=subprocess.Popen(cmd,cwd='/tmp',stdout=o,stderr=e);row['child_pid']=child.pid;save();rc=child.wait()
   pins[str(stdout)]=sha(stdout);pins[str(stderr)]=sha(stderr);row.update(returncode=rc,stdout_path=str(stdout),stderr_path=str(stderr),elapsed_seconds=time.monotonic()-start);save();assert rc==0
   value=json.loads(stdout.read_text());n=len(value['confidence']);assert n>0 and all(len(value[k])==n for k in ['keypoints0','keypoints1','batch_indexes']);row.update(status='complete',matches=n,rss_bytes=int(re.search(r'(\d+)\s+maximum resident set size',stderr.read_text())[1]));save();print(mode,direction,n,row['rss_bytes'],flush=True)
  for direction in [0,1]:
   rows=[x for x in r['results'] if x['direction']==direction];assert Path(rows[0]['stdout_path']).read_bytes()==Path(rows[1]['stdout_path']).read_bytes(),'Raw match tensor serialization drift'
  r['status']='complete_two_direction_nonempty_raw_output_parity_not_adopted';save()
 except Exception as error:r.update(status='failed',error=str(error));save();raise
if __name__=='__main__':main()
