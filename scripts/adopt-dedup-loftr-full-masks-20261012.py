#!/usr/bin/env python3
import hashlib,json,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 vp=D/'dedup-loftr-full-masks-verification-20261012.json';v=json.loads(vp.read_text());assert v['passed']==55 and v['ignored']==0 and all(sha(p)==h for p,h in v['pins'].items())
 mp=D/'dedup-loftr-full-masks-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);bp=Path(m['parent_manifest']);b=json.loads(bp.read_text());assert all(sha(s/p)==h for p,h in m['file_hashes'].items())
 for p,h in b['file_hashes'].items():
  if p.startswith(('vendor/loftr/src/','crates/rrrah-dedup/src/')):assert sha(R/p)==h,p
 paths=['vendor/loftr/src/linear_attention.rs','vendor/loftr/src/linear_attention/tests.rs'];assert set(m['deltas_from_parent'])==set(paths)
 backup=T/'rrrah-loftr-full-masks-adoption-before-20261012';out=D/'dedup-loftr-full-masks-adoption-20261012.json';assert not backup.exists() and not out.exists();backup.mkdir();before={}
 for p in paths:
  dest=backup/p;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(R/p,dest);before[p]=sha(dest);shutil.copy2(s/p,R/p);assert sha(R/p)==m['file_hashes'][p]
 for p,h in m['file_hashes'].items():
  if p.startswith(('vendor/loftr/src/','crates/rrrah-dedup/src/')):assert sha(R/p)==h,p
 subprocess.run(['git','diff','--check','--',*paths],cwd=R,check=True)
 pins={str(p):sha(p) for p in [vp,mp,bp,Path(__file__),*[backup/p for p in paths]]};out.write_text(json.dumps({'status':'adopted_full_attention_mask_fix','changed_files':paths,'before_hashes':before,'after_hashes':{p:sha(R/p) for p in paths},'passed':55,'ignored':0,'pins':pins,'scope':'Independent FullAttention masks validated; unsupported rows produce finite zero outputs and gradients. Four pre-fix failures, full55nativebackendunitpasses, exact source reconstruction and preserved pre-adoption files. Existing unmasked arithmetic and all other dedup/LoFTR source unchanged. No commit/push, other platforms/fullformats/full500/corpus or total memory cap qualification.'},indent=2)+'\n');print('Adopted full-attention mask fix after55passes')
if __name__=='__main__':main()
