#!/usr/bin/env python3
import os,json,hashlib,subprocess,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');D=R/'docs/research';F=T/'rrrah-vendor-recipe-cargo-controls-20261013';assert not F.exists();F.mkdir();sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();old=T/'rrrah-mesh-deformation-source-20261013/crates/rrrah-dedup/build.rs';new=R/'crates/rrrah-dedup/build.rs';context=R/'crates/rrrah-dedup/build_context.rs';rows=[];pins={str(p):sha(p) for p in [old,new,context,Path(__file__)]}
for label,script in [('original',old),('fixed',new)]:
 root=F/label;crate=root/'crates/rrrah-dedup';(crate/'src').mkdir(parents=True);vendor=root/'vendor/pixel/src';vendor.mkdir(parents=True);(root/'Cargo.toml').write_text('[workspace]\nmembers=["crates/rrrah-dedup"]\nresolver="2"\n');(crate/'Cargo.toml').write_text('[package]\nname="recipe_fixture"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nvendor_pixel={path="../../vendor/pixel"}\n[build-dependencies]\nblake3="=1.8.5"\n');(crate/'build.rs').write_bytes(script.read_bytes());(crate/'build_context.rs').write_bytes(context.read_bytes());(crate/'src/main.rs').write_text('fn main(){println!("{} {}",env!("RRRAH_BUILT_RECIPE_ID"),vendor_pixel::value());}\n');(vendor.parent/'Cargo.toml').write_text('[package]\nname="vendor_pixel"\nversion="0.0.0"\nedition="2024"\n')
 for name in ['rrrah-core','rrrah-decode','rrrah-memory']:
  p=root/'crates'/name;p.mkdir();(p/'Cargo.toml').write_text('# fixture manifest for source-root identification\n');(p/'lib.rs').write_text('// unchanged source fixture\n')
 target=F/(label+'-target');outputs=[];lock_hash=None
 for value in [1,2]:
  (vendor/'lib.rs').write_text('pub fn value()->u8{'+str(value)+'}\n');cmd=['cargo','run','--offline','--manifest-path',str(crate/'Cargo.toml'),'--target-dir',str(target),'--quiet'];r=subprocess.run(cmd,cwd=R,capture_output=True,text=True);log=F/(label+'-'+str(value)+'.log');log.write_text(r.stdout+r.stderr);assert r.returncode==0,r.stderr;m=re.fullmatch(r'([0-9a-f]{64}) ([12])\n',r.stdout);assert m and int(m[2])==value;current_lock=sha(root/'Cargo.lock')
  if lock_hash is None:lock_hash=current_lock
  else:assert current_lock==lock_hash
  outputs.append(dict(vendor_value=value,recipe=m[1],log=str(log),log_sha256=sha(log),cargo_lock_sha256=current_lock,command=cmd))
 same=outputs[0]['recipe']==outputs[1]['recipe'];assert same==(label=='original');rows.append(dict(build_script=label,outputs=outputs,recipe_changes_with_vendor=not same))
 for p in root.rglob('*'):
  if p.is_file():pins[str(p)]=sha(p)
assert all(sha(p)==h for p,h in pins.items());out=D/'dedup-vendor-build-recipe-regression-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_real_cargo_vendor_semantic_change_invalidates_only_fixed_recipe',rows=rows,pins=pins,scope='Actual original/fixed build.rs compiled as Cargo build scripts in two isolated synthetic complete-workspace fixtures. Same Cargo.lock, four crate source trees and compiler context within each experiment; only a path dependency implementation changes its returned value1to2. Original recipe stays equal despite changed executed dependency behavior; fixed recipe changes. This is a real Cargo/compiler regression, not an actual PDF raster/cache-file reproduction. No root vendor files changed; no general build-system/source-tamper guarantee.'),indent=2)+'\n');print(out)
