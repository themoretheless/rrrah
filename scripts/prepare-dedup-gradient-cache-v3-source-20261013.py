#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,shutil,os,sys
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-gradient-cache-v3-source-20261013';assert not S.is_symlink();sealed=R/'docs/research/dedup-gradient-cache-v3-source-20261013.json';assert not sealed.exists();resume=S.exists();assert not resume or '--resume-unsealed' in sys.argv;folders=['crates/rrrah-dedup','crates/rrrah-core','crates/rrrah-decode','crates/rrrah-memory','vendor'];sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def inventory(base,folder):
 out={}
 for parent,dirs,files in os.walk(base/folder):
  dirs[:]=[d for d in dirs if d not in ['target','.git']]
  for f in files:
   p=Path(parent)/f
   if p.is_file():out[str(p.relative_to(base))]=sha(p)
 return out
context=['Cargo.toml','Cargo.lock','rust-toolchain.toml','rustfmt.toml']+[str(p.relative_to(R)) for p in (R/'.cargo').glob('*') if p.is_file()];before={folder:inventory(R,folder) for folder in folders};context_before={f:sha(R/f) for f in context};assert '.child(MAX_DISTANCE_CACHE_BYTES)' in (R/'crates/rrrah-dedup/src/gradient.rs').read_text();
if not resume:subprocess.run(['cp','-cR',str(T/'rrrah-consensus-native-source-20261013'),str(S)],check=True)
for owned in [S,S/'crates']:
 assert not owned.is_symlink();owned.chmod(owned.stat().st_mode|0o700)
for folder in folders:
 dst=S/folder;assert not dst.is_symlink()
 if dst.exists():
  for parent,dirs,files in os.walk(dst):
   owned=Path(parent);assert owned.is_relative_to(S);owned.chmod(owned.stat().st_mode|0o700)
  shutil.rmtree(dst)
 subprocess.run(['cp','-cR',str(R/folder),str(dst)],check=True)
for f in context:
 dst=S/f;dst.parent.mkdir(parents=True,exist_ok=True)
 owned=dst.parent
 while owned.is_relative_to(S):
  assert not owned.is_symlink();owned.chmod(owned.stat().st_mode|0o700)
  if owned==S:break
  owned=owned.parent
 if dst.exists():dst.chmod(dst.stat().st_mode|0o200)
 subprocess.run(['cp','-cp',str(R/f),str(dst)],check=True)
assert all(before[f]==inventory(R,f)==inventory(S,f) for f in folders);assert context_before=={f:sha(R/f) for f in context}=={f:sha(S/f) for f in context};meta=subprocess.run(['cargo','+stable','metadata','--offline','--locked','--no-deps','--format-version','1'],cwd=S,capture_output=True,text=True);assert meta.returncode==0,meta.stderr;pins={str(p.relative_to(S)):sha(p) for p in S.rglob('*') if p.is_file() and 'target' not in p.relative_to(S).parts and '.git' not in p.relative_to(S).parts};P=R/'docs/research/dedup-gradient-cache-v3-source-20261013.json';assert not P.exists();P.write_text(json.dumps(dict(status='fresh_frozen_source_selected_root_stable_before_after_preparation',source=str(S),source_sha256=pins,root_parity=[dict(folder=f,files=len(before[f]),match=True) for f in folders],context_sha256=context_before,scope='Fresh current four-crate/vendor source copied after observed global format drift; immutable selected bytes verified before/after preparation. Includes isolated16MiBcache child and deferred allocation. Other workspace/app source remains inherited and unqualified; Cargo metadata resolution verified offline/locked. Test from this snapshot, not live root.'),indent=2)+'\n');print(P)
