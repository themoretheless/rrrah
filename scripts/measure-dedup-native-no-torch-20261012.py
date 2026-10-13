#!/usr/bin/env python3
"""Repeat a pinned native success without linking the optional LoFTR runtime."""
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1]; D=R/'docs/research'; T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 manifest=D/'dedup-native-bank-outcome-native-snapshot-20261011.json'; m=json.loads(manifest.read_text()); s=Path(m['snapshot']); h=T/'rrrah-native-no-torch-harness-20261012'; out=D/'dedup-native-no-torch-20261012.json'
 assert not h.exists() and not out.exists()
 pins={str(s/k):v for k,v in m['file_hashes'].items()}
 assert all(sha(p)==v for p,v in pins.items())
 h.mkdir(); (h/'src/bin').mkdir(parents=True)
 old=s/'scripts/dedup-loftr-native-probe/src/bin/native_bank_outcome_files.rs'
 text=old.read_text().replace('    tch::set_num_threads(2);\n    tch::manual_seed(173);\n','')
 text=text.replace('rrrah_dedup::model_loftr::scan_loftr_native_bank_recovery_collection','rrrah_dedup::portfolio_collection::scan_native_bank_recovery_collection_with')
 text=text.replace('&native_policy::factors(),&budget,||false)?;', '&native_policy::factors(),&budget,|_,_,_,_,_| panic!("model provider must never be called in this native control"),||false)?;')
 q=h/'src/bin/native_no_torch.rs';q.write_text(text)
 shutil.copy2(old.parent.parent/'native_policy.rs',h/'src/native_policy.rs')
 cargo=(s/'scripts/dedup-loftr-native-probe/Cargo.toml').read_text()
 cargo=cargo.replace('name = "dedup-loftr-native-probe"','name = "dedup-native-no-torch"').replace('loftr = { path = "../../vendor/loftr" }\n','').replace('tch = "=0.23.0"\n','').replace('features = ["loftr"]','features = ["decode"]')
 cargo=cargo.replace('path = "../../',f'path = "{s}/')
 (h/'Cargo.toml').write_text(cargo)
 pins.update({str(p):sha(p) for p in [manifest,Path(__file__),q,h/'src/native_policy.rs',h/'Cargo.toml']})
 left=T/'rrrah-dedup-copydays/original-resolution-negative-200201/212400.jpg';right=T/'rrrah-dedup-copydays/original-resolution-strong-all/212402.jpg';weights=T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors'
 pins.update({str(p):sha(p) for p in [left,right,weights]})
 r={'status':'building','producer_pid':os.getpid(),'pins':pins,'scope':'Same frozen native policy and original inputs as baseline run163. Decode feature only; provider invocation is fatal. RSS comparison is one concurrent-host control, not a universal cap or benchmark.'}
 def save():p=out.with_suffix('.tmp');p.write_text(json.dumps(r,indent=2)+'\n');p.replace(out)
 save()
 try:
  log=T/'rrrah-native-no-torch-build-20261012.log'; target=T/'rrrah-native-no-torch-target-20261012'
  with log.open('w') as f:
   p=subprocess.Popen(['cargo','build','--manifest-path',str(h/'Cargo.toml'),'--bin','native_no_torch','--offline','--target-dir',str(target)],cwd='/tmp',stdout=f,stderr=subprocess.STDOUT);r['build_pid']=p.pid;save();code=p.wait()
  r.update(build_returncode=code,build_log=str(log),build_log_sha256=sha(log));save();assert code==0
  binary=T/'rrrah-native-no-torch-executable-20261012';assert not binary.exists();shutil.copy2(target/'debug/native_no_torch',binary)
  pins.update({str(p):sha(p) for p in [binary,h/'Cargo.lock']})
  deps=subprocess.check_output(['otool','-L',str(binary)],text=True);r['dynamic_libraries']=deps;assert 'torch' not in '\n'.join(deps.splitlines()[1:]).lower()
  cmd=['/usr/bin/time','-l',str(binary),str(left),str(right),str(weights),'640','2.4743086145893445','1000000'];r.update(status='running',command=cmd);save();start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);r['child_pid']=p.pid;save();stdout,stderr=p.communicate();r.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start)
  assert p.returncode==0;v=json.loads(stdout);assert v['managed_used']==0 and v['pairs'][0]['support'] and not v['pairs'][0]['model_attempted'];assert all(sha(p)==v for p,v in pins.items())
  r['status']='complete_native_no_torch_control';save()
 except Exception as e:r.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
