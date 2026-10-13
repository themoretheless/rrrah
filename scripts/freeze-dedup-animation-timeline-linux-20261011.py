#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');DEST=T/'rrrah-animation-timeline-linux-snapshot-20261011';OUT=D/'dedup-animation-timeline-linux-snapshot-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not DEST.exists() and not OUT.exists();pp=D/'dedup-gradient-index-actual-work-bounded-snapshot-20261011.json';parent=json.loads(pp.read_text());base=Path(parent['snapshot'])
 for p,h in parent['file_hashes'].items():
  assert sha(base/p)==h;target=DEST/p;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(base/p,target)
 observed=[R/'Cargo.toml',R/'Cargo.lock']
 for crate in ['rrrah-dedup','rrrah-core','rrrah-decode','rrrah-memory']:
  observed += [p for p in (R/'crates'/crate).rglob('*') if p.is_file() and 'target' not in p.parts and '.git' not in p.parts]
 live={str(p.relative_to(R)):sha(p) for p in observed}
 changes={};hashes=dict(parent['file_hashes'])
 for p,h in live.items():
  if hashes.get(p)!=h:
   target=DEST/p;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(R/p,target);assert sha(target)==h;changes[p]={'parent_sha256':hashes.get(p),'candidate_sha256':h}
  hashes[p]=h
 assert all(sha(R/p)==h for p,h in live.items())
 assert all(sha(DEST/p)==h for p,h in hashes.items())
 report={'snapshot':str(DEST),'parent_manifest':str(pp),'parent_manifest_sha256':sha(pp),'file_hashes':hashes,'changes':changes,'observed_root_hashes':live,'producer':str(Path(__file__).resolve()),'producer_sha256':sha(Path(__file__)),'scope':'Current four-crate sources plus root Cargo manifests captured atomically by before/after hashes over a verified frozen dependency/source baseline. Includes observed unrelated concurrent core/decode/memory work without changing the working tree. Bounded animation and sequence Linux gate only.'}
 OUT.write_text(json.dumps(report,indent=2)+'\n');print('frozen',len(hashes),'files;',len(changes),'observed deltas')
if __name__=='__main__':main()
