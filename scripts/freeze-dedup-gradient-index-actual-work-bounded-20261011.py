#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');DEST=T/'rrrah-gradient-index-actual-work-bounded-snapshot-20261011';OUT=D/'dedup-gradient-index-actual-work-bounded-snapshot-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not DEST.exists();pp=D/'dedup-gradient-index-actual-work-snapshot-20261011.json';parent=json.loads(pp.read_text());source=Path(parent['snapshot'])
 for p,h in parent['file_hashes'].items():
  assert sha(source/p)==h;target=DEST/p;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source/p,target)
 edits={'crates/rrrah-dedup/src/gradient_index.rs':T/'rrrah-gradient-index-actual-work-bounded-source-20261011.rs','crates/rrrah-dedup/tests/gradient_index.rs':T/'rrrah-gradient-index-actual-work-bounded-integration-20261011.rs'}
 for p,edit in edits.items():shutil.copy2(edit,DEST/p)
 hashes={p:sha(DEST/p) for p in parent['file_hashes']}
 OUT.write_text(json.dumps({'snapshot':str(DEST),'parent_manifest':str(pp),'parent_manifest_sha256':sha(pp),'changes':list(edits),'file_hashes':hashes,'producer':str(Path(__file__).resolve()),'producer_sha256':sha(Path(__file__).resolve()),'edit_pins':{str(p):sha(p) for p in edits.values()},'scope':'Candidate opt-in actual descriptor-comparison admission; legacy n*n preflight remains, exact radius retrieval and atomic failure retained.'},indent=2)+'\n');print('frozen actual-work candidate',len(hashes))
if __name__=='__main__':main()
