#!/usr/bin/env python3
import hashlib,json,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 vp=D/'dedup-loftr-masked-values-verification-20261012.json';v=json.loads(vp.read_text());assert v['passed']==51 and v['ignored']==0 and all(sha(p)==h for p,h in v['pins'].items())
 mp=D/'dedup-loftr-masked-values-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);bp=Path(m['parent_manifest']);b=json.loads(bp.read_text());assert all(sha(s/p)==h for p,h in m['file_hashes'].items())
 for p,h in b['file_hashes'].items():
  if p.startswith(('vendor/loftr/src/','crates/rrrah-dedup/src/')):assert sha(R/p)==h,p
 paths=['vendor/loftr/src/linear_attention.rs','vendor/loftr/src/linear_attention/tests.rs'];assert set(m['deltas_from_parent'])==set(paths)
 backup=T/'rrrah-loftr-masked-values-adoption-before-20261012';out=D/'dedup-loftr-masked-values-adoption-20261012.json';assert not backup.exists() and not out.exists();backup.mkdir();before={}
 for p in paths:
  dest=backup/p;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(R/p,dest);before[p]=sha(dest);shutil.copy2(s/p,R/p);assert sha(R/p)==m['file_hashes'][p]
 for p,h in m['file_hashes'].items():
  if p.startswith(('vendor/loftr/src/','crates/rrrah-dedup/src/')):assert sha(R/p)==h,p
 subprocess.run(['git','diff','--check','--',*paths],cwd=R,check=True)
 pins={str(p):sha(p) for p in [vp,mp,bp,Path(__file__),*[backup/p for p in paths]]};out.write_text(json.dumps({'status':'adopted_masked_value_ownership_fix','changed_files':paths,'before_hashes':before,'after_hashes':{p:sha(R/p) for p in paths},'passed':51,'ignored':0,'pins':pins,'scope':'Only masked LinearAttention value result becomes owned; borrowed inputs and aliases remain unchanged and leaf gradients work. Three pre-fix failures, full51nativebackendunitpasses, exact source reconstruction and preserved pre-adoption files. Existing unmasked arithmetic and all other dedup/LoFTR source unchanged. No commit/push, other platforms/fullformats/full500/corpus or total memory cap qualification.'},indent=2)+'\n');print('Adopted masked-value ownership fix after51passes')
if __name__=='__main__':main()
