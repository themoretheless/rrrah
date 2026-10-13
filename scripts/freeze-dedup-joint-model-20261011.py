#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-joint-recovery-testfix-snapshot-20261011.json';data=json.loads(parent.read_text());src=Path(data['snapshot']);dst=T/'rrrah-joint-model-snapshot-20261011';out=D/'dedup-joint-model-snapshot-20261011.json';assert not dst.exists() and not out.exists();dst.mkdir();hashes=data['file_hashes'].copy()
 for f,h in hashes.items():
  assert sha(src/f)==h,f
  q=dst/f;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src/f,q)
 changed=['crates/rrrah-dedup/src/model_compare.rs','crates/rrrah-dedup/tests/model_compare.rs'];deltas=[]
 for f in changed:
  p=R/f;q=dst/f;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,q);h=sha(p);assert sha(q)==h;deltas.append({'path':f,'before':hashes.get(f),'after':h});hashes[f]=h
 for f,h in hashes.items():assert sha(dst/f)==h,f
 out.write_text(json.dumps({'snapshot':str(dst),'parent_manifest':str(parent),'parent_sha256':sha(parent),'file_hashes':hashes,'deltas':deltas,'scope':'Joint opposite file integration child: extraction without redundant legacy fit, caller-coordinate joint fitting and six-recipe preflight. Compatible parent decoder and lock preserved; not whole current HDR workspace qualification.'},indent=2)+'\n');print(str(out))
if __name__=='__main__':main()
