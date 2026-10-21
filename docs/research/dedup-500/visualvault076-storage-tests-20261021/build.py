from pathlib import Path
import subprocess
root=Path.cwd()
old=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-realmodel-20261021')
dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-storage-20261021')
(dest/'src').mkdir(parents=True,exist_ok=True)
s=(old/'src/lib.rs').read_text()
for name,path in [('file_manager','crates/core/src/file_manager.rs'),('bytes','crates/utils/src/bytes.rs')]:
 s+='\n#[path="'+str(root/'.dedup-work/visualvault076-20261021'/path)+'"] mod '+name+';\n'
(dest/'src/lib.rs').write_text(s)
(dest/'Cargo.toml').write_text((old/'Cargo.toml').read_text().replace('visualvault076-realmodel','visualvault076-storage'))
log=root/'docs/research/dedup-500/visualvault076-storage-tests-20261021/run.log'
with log.open('w') as f:
 for name in ['file_manager::tests::','bytes::tests::']:
  subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021'),name],stdout=f,stderr=subprocess.STDOUT,check=True)
log.write_text(log.read_text().rstrip()+'\n')
