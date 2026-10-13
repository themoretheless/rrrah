#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-recovery-collection-snapshot-20261011.json';data=json.loads(parent.read_text());src=Path(data['snapshot']);dst=T/'rrrah-joint-projective-snapshot-20261011';out=D/'dedup-joint-projective-snapshot-20261011.json';assert not dst.exists() and not out.exists();dst.mkdir();hashes=data['file_hashes'].copy()
 for f,h in hashes.items():
  assert sha(src/f)==h,f
  q=dst/f;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src/f,q)
 changed=['crates/rrrah-dedup/src/geometry.rs','crates/rrrah-dedup/tests/joint_projective.rs','crates/rrrah-dedup/tests/fixtures/joint-geometry-204100.bits.rs','crates/rrrah-dedup/tests/fixtures/joint-geometry-211900.bits.rs','scripts/dedup-loftr-native-probe/src/bin/joint_library_geometry_trace.rs'];deltas=[]
 for f in changed:
  p=R/f;q=dst/f;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,q);h=sha(p);assert sha(q)==h;deltas.append({'path':f,'before':hashes.get(f),'after':h});hashes[f]=h
 for f,h in hashes.items():assert sha(dst/f)==h,f
 out.write_text(json.dumps({'snapshot':str(dst),'parent_manifest':str(parent),'parent_sha256':sha(parent),'file_hashes':hashes,'deltas':deltas,'scope':'Frozen native child: new joint geometry public primitive and exact-bit diagnostic. Other sources and compatible lock remain pinned parent; generic current-root gate qualified separately.'},indent=2)+'\n');print(str(out))
if __name__=='__main__':main()
