#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');DEST=T/'rrrah-scheduled-loftr-snapshot-20261011';OUT=D/'dedup-scheduled-loftr-snapshot-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not DEST.exists() and not OUT.exists();pp=D/'dedup-animation-timeline-linux-locked-snapshot-20261011.json';parent=json.loads(pp.read_text());base=Path(parent['snapshot'])
 for p,h in parent['file_hashes'].items():
  assert sha(base/p)==h;target=DEST/p;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(base/p,target)
 observed=[]
 for relative in ['crates/rrrah-dedup','crates/rrrah-core','crates/rrrah-decode','crates/rrrah-memory','scripts/dedup-loftr-native-probe']:
  observed += [p for p in (R/relative).rglob('*') if p.is_file() and 'target' not in p.parts and '.git' not in p.parts]
 live={str(p.relative_to(R)):sha(p) for p in observed};hashes=dict(parent['file_hashes']);changes={}
 for p,h in live.items():
  if hashes.get(p)!=h:
   target=DEST/p;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(R/p,target);assert sha(target)==h;changes[p]={'parent_sha256':hashes.get(p),'candidate_sha256':h}
  hashes[p]=h
 assert all(sha(R/p)==h for p,h in live.items());assert all(sha(DEST/p)==h for p,h in hashes.items())
 out={'snapshot':str(DEST),'parent_manifest':str(pp),'parent_manifest_sha256':sha(pp),'file_hashes':hashes,'changes':changes,'observed_root_hashes':live,'producer':str(Path(__file__).resolve()),'producer_sha256':sha(Path(__file__)),'scope':'Frozen baseline with current four-crate and native diagnostic file bytes verified by before/after reads; no atomic filesystem capture claim. Root baseline lock unchanged, native standalone lock copied with the standalone source. Includes concurrent core/decode/memory changes without altering them. CPU scheduled LoFTR build and bounded controls, not full corpus qualification.'}
 OUT.write_text(json.dumps(out,indent=2)+'\n');print('frozen',len(hashes),'files;',len(changes),'deltas')
if __name__=='__main__':main()
