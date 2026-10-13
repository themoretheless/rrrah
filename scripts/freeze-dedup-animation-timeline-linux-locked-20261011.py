#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');DEST=T/'rrrah-animation-timeline-linux-locked-snapshot-20261011';OUT=D/'dedup-animation-timeline-linux-locked-snapshot-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not DEST.exists() and not OUT.exists();pp=D/'dedup-animation-timeline-linux-snapshot-20261011.json';m=json.loads(pp.read_text());base=Path(m['snapshot']);baseline_path=D/'dedup-gradient-index-actual-work-bounded-snapshot-20261011.json';baseline=json.loads(baseline_path.read_text());lock=Path(baseline['snapshot'])/'Cargo.lock';assert sha(lock)==baseline['file_hashes']['Cargo.lock']
 for p,h in m['file_hashes'].items():
  assert sha(base/p)==h;target=DEST/p;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(base/p,target)
 for p in ['Cargo.toml','crates/rrrah-core/Cargo.toml','crates/rrrah-decode/Cargo.toml','crates/rrrah-memory/Cargo.toml']:
  assert m['file_hashes'][p]==baseline['file_hashes'][p]
 shutil.copy2(lock,DEST/'Cargo.lock');hashes=dict(m['file_hashes']);hashes['Cargo.lock']=sha(lock)
 r={'snapshot':str(DEST),'parent_manifest':str(pp),'parent_manifest_sha256':sha(pp),'dependency_baseline_manifest':str(baseline_path),'dependency_baseline_manifest_sha256':sha(baseline_path),'file_hashes':hashes,'changes':['Cargo.lock'],'producer':str(Path(__file__).resolve()),'producer_sha256':sha(Path(__file__)),'scope':'Same frozen current four-crate code, restoring the verified dependency-baseline lock matching unchanged baseline workspace manifests. Current root lock is untouched. Diagnostic example stanzas add no dependencies. Not equivalence to the newer root dependency resolution.'}
 OUT.write_text(json.dumps(r,indent=2)+'\n');print('frozen consistent dependency-baseline lock',len(hashes))
if __name__=='__main__':main()
