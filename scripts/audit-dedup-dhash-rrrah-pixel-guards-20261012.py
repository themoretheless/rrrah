#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-dhash-rrrah-pixel-guards-verification-20261012.json';copies=T/'rrrah-dhash-pixel-guards-executables-20261012';assert not out.exists()and not copies.exists()
 mp=D/'dedup-exposure-recovery-search-snapshot-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items());assert all(sha(R/k)==m['file_hashes'][k] for k in ['crates/rrrah-dedup/src/pixels.rs','crates/rrrah-dedup/src/linear.rs','crates/rrrah-dedup/tests/pixels.rs','crates/rrrah-dedup/tests/linear.rs'])
 log=T/'rrrah-dhash-rrrah-pixel-guards-20261012.log';text=log.read_text();assert re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out',text)==['4','6'];assert 'invisible_rgb_and_signed_zero_are_canonical ... ok' in text and 'uniform_hash_collision_is_not_informative ... ok'in text and 'invisible_rgb_is_canonical_but_visible_pixels_are_exact ... ok'in text
 binaries=re.findall(r'Running tests/(linear|pixels)\.rs \(([^)]+)\)',text);assert len(binaries)==2;copies.mkdir();pins={str(p):sha(p) for p in [mp,log,Path(__file__)]}
 for name,path in binaries:
  p=copies/name;shutil.copy2(path,p);pins[str(p)]=sha(p)
 out.write_text(json.dumps({'status':'verified_ten_frozen_source_linear_pixel_guard_tests','tests':10,'pins':pins,'scope':'Existing canonical invisible RGB/signed zero and uniform hash collision controls pass on unchanged184-gate source; no P-mode decoder-specific fixture or new full194-test gate is claimed. Upstream dhash counterexample remains separate and no production code copied.'},indent=2)+'\n');print('Verified10 existing linear/pixel guard tests')
if __name__=='__main__':main()
