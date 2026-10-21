from pathlib import Path
import subprocess
root=Path.cwd();old=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-detector-20261021');dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-realmodel-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
s=(old/'src/lib.rs').read_text();start=s.index('#[path=');s=s[start:]
model=root/'.dedup-work/visualvault076-20261021/crates/models/src/media_file.rs'
s='extern crate self as visualvault_models;\nuse std::{path::PathBuf,sync::Arc};\n'+f'#[path="{model}"] pub mod media_file;\npub use media_file::{{MediaFile,FileType}};\n'+s
(dest/'src/lib.rs').write_text(s)
t=(old/'Cargo.toml').read_text().replace('visualvault076-detector','visualvault076-realmodel').replace('chrono="0.4"','chrono={version="0.4",features=["serde"]}')+'serde={version="1",features=["derive","rc"]}\nserde_json="1"\n';(dest/'Cargo.toml').write_text(t)
with (root/'docs/research/dedup-500/visualvault076-controls-v4-20261021/run.log').open('w') as f:subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021')],stdout=f,stderr=subprocess.STDOUT,check=True)
