#!/usr/bin/env python3
from pathlib import Path
import subprocess,hashlib,json,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';F=Path('/private/tmp/rrrah-gradient-index-debug-v1-20261015');F.mkdir(exist_ok=False);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();s=R/'crates/rrrah-dedup/src/gradient_index.rs';shutil.copyfile(s,F/'index.rs');h=F/'harness.rs';h.write_text('pub use rrrah_dedup::{gradient, local};\n#[path="index.rs"]pub mod gradient_index;\n');deps=Path('/private/tmp/rrrah-consensus-collection-target-20261014/debug/deps');attempts=[];chosen=None
for lib in sorted(deps.glob('librrrah_dedup-*.rlib'),key=lambda p:p.stat().st_mtime,reverse=True):
 for core in sorted(deps.glob('librrrah_core-*.rlib'),key=lambda p:p.stat().st_mtime,reverse=True):
  cmd=['rustc','+stable','--edition=2024','--crate-type','lib','--emit=metadata','-Dwarnings','-L','dependency='+str(deps),'--extern','rrrah_dedup='+str(lib),'--extern','rrrah_core='+str(core),str(h),'-o',str(F/'qualified.rmeta')]
  result=subprocess.run(cmd,capture_output=True,text=True);log=F/f'attempt-{len(attempts)}.log';log.write_text(result.stdout+result.stderr);attempts.append(dict(command=cmd,returncode=result.returncode,log=str(log),sha256=sha(log)))
  if result.returncode==0:chosen=(lib,core,cmd);break
 if chosen:break
assert chosen,'No matching cached real dependency pair compiled; inspect retained diagnostics'
lib,core,cmd=chosen
# A real prechange diagnostic is the refusal oracle; no mocked crate types.
old=(F/'index.rs').read_text().replace('#[derive(Debug)]\npub struct GradientDescriptorIndex', 'pub struct GradientDescriptorIndex',1);(F/'index.rs').write_text(old);before=subprocess.run(cmd,capture_output=True,text=True);(F/'before.log').write_text(before.stdout+before.stderr);assert before.returncode!=0 and 'does not implement `Debug`'in before.stderr
shutil.copyfile(s,F/'index.rs');after=subprocess.run(cmd,capture_output=True,text=True);(F/'after.log').write_text(after.stdout+after.stderr);assert after.returncode==0 and sha(s)==sha(F/'index.rs')
O=D/'dedup-gradient-index-debug-v1-audit-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_before_failure_after_strict_metadata_success',producer_sha256=sha(__file__),source=str(s.relative_to(R)),source_sha256=sha(s),compiler=subprocess.check_output(['rustc','+stable','-Vv'],text=True),attempts=attempts,dependency_pins={str(p):sha(p)for p in [lib,core]},before_returncode=before.returncode,after_returncode=after.returncode,log_pins={str(p):sha(p)for p in F.glob('*.log')},scope='Actual unchanged current gradient_index module with onlyderiveDebug change, reexported real cached gradient/local types and actual rrrah_core SharedBuffer/MemoryBudget. Strictwarnings standalone metadata compile succeeds after fix, fails before. No mocks, no runtime/performance/fullRoot Cargo/wholeworkspace qualification.'),indent=2)+'\n');print('Real module strict metadata: beforefailed, afterpassed')
