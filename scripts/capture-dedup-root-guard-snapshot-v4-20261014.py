#!/usr/bin/env python3
"""Freeze repository code/fixtures while preserving active Root work untouched."""
from pathlib import Path
import os,json,hashlib,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();O=D/'dedup-root-guard-snapshot-v4-20261014.json';assert not O.exists();selected={'rrrah-dedup','rrrah-core','rrrah-decode','rrrah-memory'};excluded={'target','.git','__pycache__','.venv','node_modules'};paths=[];symlinks=[]
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
F=Path('/private/tmp/rrrah-root-guard-snapshot-v4-20261014');F.mkdir(exist_ok=False);pins={}
for source in sorted(set(paths)):
 rel=str(source.relative_to(R));dest=F/rel;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,dest);pins[rel]=sha(dest)
changed=[]
for validation in range(2):
 changed=sorted(set(changed)|{k for k,h in pins.items()if not (R/k).is_file()or sha(R/k)!=h})
d=dict(status='stable_snapshot_captured'if not changed else'source_drift_observed_snapshot_not_qualified',source=str(F),root=str(R),source_sha256=pins,changed_sources=changed,skipped_symlinks=symlinks,producer_sha256=sha(__file__),scope='Snapshotfile bytes pinned immediately after copy, then two completeRoot validation passes. Repositorycode/fixtures only, no external/private corpus, targets/cache/credentials/symlinks. Rootwork untouched. Not claimed an OSatomic snapshot; any observed persistentRoot drift refuses stable qualification. Selected4crate/source assets plus otherworkspaceRust/manifests only, not wholeworkspace validation.')
O.write_text(json.dumps(d,indent=2)+'\n');assert not changed,changed;print(len(pins),'files pinned and twice revalidated')
