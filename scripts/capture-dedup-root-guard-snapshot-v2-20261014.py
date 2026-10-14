#!/usr/bin/env python3
"""Freeze repository code/fixtures while preserving active Root work untouched."""
from pathlib import Path
import os,json,hashlib,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();O=D/'dedup-root-guard-snapshot-v2-20261014.json';assert not O.exists();selected={'rrrah-dedup','rrrah-core','rrrah-decode','rrrah-memory'};excluded={'target','.git','__pycache__','.venv','node_modules'};paths=[];symlinks=[]
for top in [R/'crates',R/'vendor',R/'tests/fixtures']:
 for folder,dirs,files in os.walk(top,followlinks=False):
  dirs[:]=[d for d in dirs if d not in excluded and not (Path(folder)/d).is_symlink()]
  for name in files:
   p=Path(folder)/name;rel=p.relative_to(R)
   if p.is_symlink():symlinks.append(str(rel));continue
   if name in {'.env','credentials'}:continue
   full=(rel.parts[0]=='vendor' and rel.parts[1] in {'hayro','hayro-interpret','moxcms','loftr'})or(rel.parts[:2]==('tests','fixtures'))or(rel.parts[0]=='crates' and rel.parts[1]in selected)
   if full or p.suffix=='.rs' or name=='Cargo.toml':paths.append(p)
for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','rustfmt.toml','.rustfmt.toml']:
 if (R/name).is_file():paths.append(R/name)
pins={str(p.relative_to(R)):sha(p)for p in paths};F=Path('/private/tmp/rrrah-root-guard-snapshot-v2-20261014');F.mkdir(exist_ok=False)
for rel,h in pins.items():
 source=R/rel;dest=F/rel;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,dest);assert sha(dest)==h,('Source changed during copy',rel)
changed=[k for k,h in pins.items()if not (R/k).is_file()or sha(R/k)!=h];d=dict(status='stable_snapshot_captured'if not changed else'source_changed_during_capture',source=str(F),root=str(R),source_sha256=pins,changed_sources=changed,skipped_symlinks=symlinks,producer_sha256=sha(__file__),scope='Repository4selectedcrate code/assets, allworkspaceRust/manifest stubs, vendor source/assets and repositorytestfixtures only; generatedtargets/caches/credentials/symlinks excluded. No external/private media corpus copied. Rootwork unchanged. Fullworkspace sources for nonselectedcrates not claimed; snapshot enables selecteddecode gate even if Root evolves. No completion or currentRootparity inferred after capture.');O.write_text(json.dumps(d,indent=2)+'\n');assert not changed,changed;print(len(pins),'files pinned in stable snapshot')
