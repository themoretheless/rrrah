#!/usr/bin/env python3
"""Actual CPU fixed-policy information/random-texture negative controls."""
import hashlib,json,os,re,shutil,struct,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def bmp(path,constant=None,seed=173):
 state=seed;data=bytearray()
 for _ in range(128*128):
  if constant is None:
   state^=(state<<13)&0xffffffffffffffff;state^=state>>7;state^=(state<<17)&0xffffffffffffffff;v=state&255
  else:v=constant
  data.extend([v,v,v])
 header=struct.pack('<2sIHHI',b'BM',54+len(data),0,0,54)+struct.pack('<IiiHHIIiiII',40,128,128,1,24,0,len(data),2835,2835,0,0)
 path.write_bytes(header+data)
def main():
 A=T/'rrrah-native-bank-generated-negatives-qualified-20261011';out=D/'dedup-native-bank-generated-negatives-20261011.json';assert not A.exists() and not out.exists();A.mkdir()
 mp=D/'dedup-native-bank-outcome-native-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items())
 ca=D/'dedup-native-bank-outcome-native-compile-audit-20261011.json';c=json.loads(ca.read_text());assert all(sha(k)==v for k,v in c['pins'].items())
 previous=D/'dedup-native-bank-all-positives-20261011.json';full=json.loads(previous.read_text());src=Path(next(k for k in full['pins'] if k.endswith('/native_bank_outcome_files') and 'all-positives-qualified' in k));assert sha(src)==full['pins'][str(src)];binary=A/src.name;shutil.copy2(src,binary)
 weights=T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors';runtime=D/'dedup-loftr-native-20261010-runtime-pins.json';rt=json.loads(runtime.read_text());libs={k:v for k,v in rt['pins'].items() if k.endswith('.dylib')};assert all(sha(k)==v for k,v in libs.items())
 cases=[]
 for label,spec in [('flat_brightness',[(64,173),(192,173)]),('independent_seeded_noise',[(None,173),(None,991)])]:
  paths=[]
  for i,(constant,seed) in enumerate(spec):
   p=A/f'{label}-{i}.bmp';bmp(p,constant,seed);paths.append(p)
  assert sha(paths[0])!=sha(paths[1]);cases.append((label,paths))
 pins={str(p):sha(p) for p in [mp,ca,binary,src,weights,runtime,Path(__file__),*[p for _,paths in cases for p in paths]]};pins.update(libs)
 r={'status':'running','producer_pid':os.getpid(),'pins':pins,'required':2,'results':[],'policy':'4096 direct native;1m joint opposite;25 model banks;2/6 radii;0.5 source tolerance;maxside640','scope':'Independent generated uniform-brightness and seeded-random texture controls. Non-support expected; typed unknown allowed. Actual CPU provider. No real burst/semantic/look-alike/general precision/full corpus/RSS claim.'}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 save()
 for label,paths in cases:
  cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),str(binary),str(paths[0]),str(paths[1]),str(weights),'640','0.5','1000000'];v={'case':label,'status':'running','command':cmd};r['results'].append(v);save();start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v['pid']=p.pid;save();stdout,stderr=p.communicate();log=A/(label+'.log');log.write_text(stdout+stderr);v.update(returncode=p.returncode,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log),elapsed_seconds=time.monotonic()-start);save();assert p.returncode==0,stderr
  value=json.loads(stdout);assert value['files']==[2,9] and len(value['pairs'])==1 and value['managed_used']==0;pair=value['pairs'][0];assert pair['support'] is not True,'FALSE SUPPORT on generated control';assert pair['model_attempted'] is True;v.update(status='complete_no_support',evidence=pair,managed_peak=value['managed_peak'],max_rss_bytes=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1]));save();print(label,pair['support'],flush=True)
 assert all(sha(k)==v for k,v in pins.items());r['status']='complete_bounded_generated_negative_controls';save()
if __name__=='__main__':main()
