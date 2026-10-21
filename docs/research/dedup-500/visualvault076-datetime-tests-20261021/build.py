from pathlib import Path
import subprocess
root=Path.cwd();dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-datetime-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
s=root/'.dedup-work/visualvault076-20261021/crates/utils/src/datetime.rs';(dest/'src/lib.rs').write_text(f'#[path="{s}"] mod datetime;\n');(dest/'Cargo.toml').write_text('[package]\nname="visualvault076-datetime"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nchrono="0.4"\n')
with (root/'docs/research/dedup-500/visualvault076-datetime-tests-20261021/run.log').open('w') as f:subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021')],stdout=f,stderr=subprocess.STDOUT,check=True)
