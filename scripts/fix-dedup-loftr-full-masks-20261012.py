#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 redp=D/'dedup-loftr-full-masks-red-20261012.json';red=json.loads(redp.read_text());assert red['status']=='reproduced_four_current_source_failures' and all(sha(p)==h for p,h in red['pins'].items())
 parent=D/'dedup-loftr-masked-values-source-20261012.json';m=json.loads(parent.read_text());s=T/'rrrah-loftr-full-masks-qualified-20261012';assert not s.exists()
 for p,h in m['file_hashes'].items():
  original=Path(m['snapshot'])/p;assert sha(original)==h
  if p.startswith(('vendor/loftr/src/','crates/rrrah-dedup/src/')):assert sha(R/p)==h,p
  dest=s/p;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(original,dest)
 source=s/'vendor/loftr/src/linear_attention.rs';code=source.read_text()
 start=code.index('        let mut qk = Tensor::einsum("nlhd,nshd->nlsh"')
 end=code.index('        if self.use_dropout',start)
 assert 'if let (Some(q_mask), Some(kv_mask))' in code[start:end]
 body=D/'dedup-loftr-full-mask-body-20261012.rs'
 source.write_text('// Modified by rrrah (2026-10-12): independent full-attention masks and finite empty support.\n'+code[:start]+body.read_text()+code[end:])
 tests=s/'vendor/loftr/src/linear_attention/tests.rs';shutil.copy2(T/'rrrah-loftr-full-mask-red-harness-20261012/src/linear_attention/tests.rs',tests)
 subprocess.run(['rustfmt','--edition','2024','--config-path',str(R/'vendor/loftr/rustfmt.toml'),str(source),str(tests)],cwd='/tmp',check=True)
 hashes={p:sha(s/p) for p in m['file_hashes']};mp=D/'dedup-loftr-full-masks-source-20261012.json';assert not mp.exists();mp.write_text(json.dumps({'status':'isolated_full_attention_mask_fix','snapshot':str(s),'parent_manifest':str(parent),'parent_sha256':sha(parent),'file_hashes':hashes,'deltas_from_parent':[p for p in hashes if hashes[p]!=m['file_hashes'][p]],'scope':'FullAttention validates independent masks and gives unsupported rows finite zero attention, preserving unmasked arithmetic and previously adopted borrowed-value fix. Four original-source failures preserved. Not adopted yet.'},indent=2)+'\n')
 h=T/'rrrah-loftr-full-mask-green-harness-20261012';assert not h.exists();h.mkdir();shutil.copytree(s/'vendor/loftr/src',h/'src');old=T/'rrrah-loftr-full-mask-red-harness-20261012'
 for f in ['Cargo.toml','Cargo.lock']:shutil.copy2(old/f,h/f)
 pins={str(p):sha(p) for p in [redp,mp,Path(__file__),*[p for p in h.rglob('*') if p.is_file()]]};out=D/'dedup-loftr-full-masks-green-20261012.json';assert not out.exists();r={'status':'building','producer_pid':os.getpid(),'pins':pins}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 save();runtime=T/'rrrah-loftr-native-runtime-20261010';env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH'];log=T/'rrrah-loftr-full-mask-green-build-20261012.jsonl';err=log.with_suffix('.stderr');cmd=['cargo','test','--no-run','--message-format=json','--lib','--locked','--offline','--manifest-path',str(h/'Cargo.toml'),'--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')]
 with log.open('w') as o,err.open('w') as e:
  child=subprocess.Popen(cmd,cwd='/tmp',env=env,stdout=o,stderr=e);r['build_pid']=child.pid;save();rc=child.wait()
 pins[str(log)]=sha(log);pins[str(err)]=sha(err);r['build_returncode']=rc;save();assert rc==0
 artifacts=[json.loads(x) for x in log.read_text().splitlines() if x.startswith('{')];paths=[x['executable'] for x in artifacts if x.get('reason')=='compiler-artifact' and x.get('executable') and x['profile']['test']];assert len(paths)==1
 exe=T/'rrrah-loftr-full-mask-green-executable-20261012';assert not exe.exists();shutil.copy2(paths[0],exe);pins[str(exe)]=sha(exe);cmd=['/usr/bin/env','DYLD_LIBRARY_PATH='+str(runtime/'lib/python3.12/site-packages/torch/lib'),str(exe),'--test-threads=1'];r.update(status='testing',command=cmd);save();log=T/'rrrah-loftr-full-mask-green-tests-20261012.log'
 with log.open('w') as f:
  child=subprocess.Popen(cmd,cwd='/tmp',stdout=f,stderr=subprocess.STDOUT);r['test_pid']=child.pid;save();rc=child.wait()
 pins[str(log)]=sha(log);r.update(status='tests_passed_not_adopted' if rc==0 else 'failed',returncode=rc,log=str(log));save();assert rc==0;print('Full-attention mask fix passed full native backend unit suite')
if __name__=='__main__':main()
