#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');A=T/'rrrah-native-direction-trace-211900-qualified-20261011';OUT=D/'dedup-native-direction-trace-211900-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists() and not OUT.exists();A.mkdir();mp=D/'dedup-native-direction-trace-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());fp=D/'dedup-directional-211900-211901-fixture-20261011.json';fixture=json.loads(fp.read_text());assert all(sha(p)==h for p,h in fixture['pins'].items());rp=D/'dedup-loftr-native-20261010-runtime-pins.json';runtime=json.loads(rp.read_text());binary=A/'native_direction_trace';shutil.copy2(T/'rrrah-dedup-exact-portfolio-20261011/debug/native_direction_trace',binary);pins={str(p):sha(p) for p in [mp,fp,rp,binary,Path(__file__)]};pins.update({p:h for p,h in runtime['pins'].items() if p.endswith('.dylib')});pins.update(fixture['pins']);plan=fixture['forward']['plan'];args=[plan['left'],plan['right'],str(plan['source_tolerance']),str(fixture['reverse']['plan']['source_tolerance'])];torch=T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib';report={'status':'running','producer_pid':os.getpid(),'source_manifest':str(mp),'pins':pins,'results':[],'required':7,'scope':'One original-resolution211900/211901 publisher pair; forward native phase0 and all6 reverse native phases, unchanged recorded source/target tolerances. Real descriptor-qualified points; inverse proof diagnostic, not new search API or full corpus qualification.'}
 def save():
  temp=OUT.with_suffix('.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
 save()
 try:
  for phase in ['forward','0','1','2','3','4','5']:
   assert all(sha(p)==h for p,h in pins.items());cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(torch),str(binary),*args,phase];row={'phase':phase,'status':'running','command':cmd};report['results'].append(row);save();start=time.monotonic();proc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['pid']=proc.pid;save();stdout,stderr=proc.communicate();log=A/(phase+'.log');log.write_text(stdout+stderr);row.update(returncode=proc.returncode,elapsed_seconds=time.monotonic()-start,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log));assert proc.returncode==0,stderr;v=json.loads(stdout);assert v['managed_used']==0;row['status']='observed';save();print(phase,len(v['correspondences']),v.get('anchor_result'),v.get('swapped_actual_correspondence_anchor_result'),flush=True)
  assert all(sha(p)==h for p,h in pins.items());report['status']='complete_bounded_native_direction_trace';save()
 except Exception as e:report.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
