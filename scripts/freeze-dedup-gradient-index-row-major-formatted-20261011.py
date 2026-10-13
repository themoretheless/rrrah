#!/usr/bin/env python3
import hashlib,json,shutil,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';DEST=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-index-row-major-formatted-snapshot-20261011');OUT=D/'dedup-gradient-index-row-major-formatted-snapshot-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not DEST.exists();pp=D/'dedup-gradient-index-row-major-snapshot-20261011.json';parent=json.loads(pp.read_text());source=Path(parent['snapshot'])
 for p,h in parent['file_hashes'].items():
  assert sha(source/p)==h;target=DEST/p;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source/p,target)
 name='crates/rrrah-dedup/src/gradient_index.rs';subprocess.run(['rustfmt','--edition','2024',str(DEST/name)],check=True)
 hashes={p:sha(DEST/p) for p in parent['file_hashes']}
 assert [p for p,h in hashes.items() if h!=parent['file_hashes'][p]]==[name]
 OUT.write_text(json.dumps({'snapshot':str(DEST),'parent_manifest':str(pp),'parent_manifest_sha256':sha(pp),'changes':[name],'file_hashes':hashes,'producer':str(Path(__file__).resolve()),'producer_sha256':sha(Path(__file__).resolve()),'scope':'Rustfmt-only child of row-major index candidate; root production remains baseline pending final qualification.'},indent=2)+'\n')
 print('formatted frozen child',len(hashes))
if __name__=='__main__':main()
