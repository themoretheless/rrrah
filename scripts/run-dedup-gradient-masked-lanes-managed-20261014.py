#!/usr/bin/env python3
"""Isolated current managed matcher integration; never edits the working library."""
from pathlib import Path
import hashlib,json,subprocess,os,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest_path=D/'dedup-gradient-unroll4-v7-source-20261014.json';m=json.loads(manifest_path.read_text());S=Path(m['source'])
assert all(sha(S/p)==h for p,h in m['source_sha256'].items())
control_path=D/'dedup-gradient-unroll4-managed-focused-v2-20261014.json';control=json.loads(control_path.read_text())
assert control['status']=='passed_pending_audit' and all(sha(p)==h for p,h in control['dependency_pins'].items())
stress_path=D/'dedup-gradient-masked-lanes-stress-audit-20261014.json';stress=json.loads(stress_path.read_text());assert stress['status']=='passed_bounded_standalone_controls'
cpu_path=D/'dedup-gradient-masked-lanes-cpu-audit-20261014.json';cpu=json.loads(cpu_path.read_text());assert cpu['status']=='passed_bounded_measurement_audit'
F=Path('/private/tmp/rrrah-gradient-masked-lanes-managed-20261014');F.mkdir(exist_ok=False)
source=F/'source';crate=source/'crates/rrrah-dedup';crate.parent.mkdir(parents=True)
subprocess.run(['cp','-cR',str(S/'crates/rrrah-dedup'),str(crate)],check=True)
for p in crate.rglob('*'):
    if p.is_file():p.chmod(p.stat().st_mode|0o200)
gradient=crate/'src/gradient.rs';text=gradient.read_text()
a=text.index('                if runners.as_ref().is_some_and(|r| {');b=text.index('                for (lane, b) in chunk.iter().enumerate()',a)
section=text[a:b]
needle='''                } else {
                    distances_four(&a.descriptor, chunk)
                };'''
assert section.count(needle)==1
section=section.replace(needle,'''                } else if let Some(runners) = runners.as_ref().filter(|r| r[i].1) {
                    let mut needed = 0_u8;
                    for lane in 0..chunk.len() {
                        if !runners[left.len() + block * 4 + lane].1 {
                            needed |= 1 << lane;
                        }
                    }
                    // The all-resolved block was skipped above, after its
                    // original cancellation checkpoints. Only reverse lanes
                    // can still need distances when the forward row resolves.
                    if needed.count_ones() <= 2 {
                        let mut values = [0.; 4];
                        for (lane, b) in chunk.iter().enumerate() {
                            if needed & (1 << lane) != 0 {
                                values[lane] = a.descriptor.0.iter().zip(&b.descriptor.0)
                                    .map(|(x, y)| (x - y) * (x - y)).sum();
                            }
                        }
                        values
                    } else {
                        distances_four(&a.descriptor, chunk)
                    }
                } else {
                    distances_four(&a.descriptor, chunk)
                };''')
gradient.write_text(text[:a]+section+text[b:])
# Format only this isolated file, without module recursion or workspace mutation.
subprocess.run(['rustfmt','+stable','--edition=2024','--config','skip_children=true',str(gradient)],check=True)
source_pins={str(p.relative_to(source)):sha(p)for p in crate.rglob('*')if p.is_file()}
original_pins={str(p.relative_to(S)):sha(p)for p in (S/'crates/rrrah-dedup').rglob('*')if p.is_file()}
changed={p for p,h in source_pins.items()if original_pins.get(p)!=h};assert changed=={'crates/rrrah-dedup/src/gradient.rs'},changed
exe=F/'tests';command=[str(exe)if v==control['executable']else str(crate/'src/lib.rs')if v==str(S/'crates/rrrah-dedup/src/lib.rs')else v for v in control['command']]
env=os.environ.copy();env['CARGO_MANIFEST_DIR']=str(crate)
O=D/'dedup-gradient-masked-lanes-managed-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),source=str(source),source_sha256=source_pins,original_manifest=str(manifest_path),original_manifest_sha256=sha(manifest_path),dependency_pins=control['dependency_pins'],pins={str(p):sha(p)for p in [control_path,stress_path,cpu_path,Path(__file__)]},command=command,scope='Isolated V7rrrah-dedup clone differing only in second-pass sparse reverse lanes. Actual managed matcher/cache/runner with existing admitted memory; no extra heap arrays. Directrustc raster library against same pinned cached dependencies. Full raster unit inventory only, not Cargo/decode/native/workspace/default/fullcorpus/platform qualification; production checkout unchanged.')
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
with (F/'build.log').open('x')as log:
    p=subprocess.Popen(command,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
state['build_returncode']=code;save();assert code==0,(F/'build.log').read_text()[-5000:]
state['pins'][str(exe)]=sha(exe)
with (F/'inventory.log').open('x')as log:
    p=subprocess.Popen([str(exe),'--list'],stdout=log,stderr=subprocess.STDOUT);state.update(status='listing',child_pid=p.pid);save();assert p.wait()==0
with (F/'tests.log').open('x')as log:
    p=subprocess.Popen([str(exe),'--test-threads','2'],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT);state.update(status='testing',child_pid=p.pid);save();code=p.wait()
state['test_returncode']=code;save()
log=(F/'tests.log').read_text();summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',log)
assert code==0 and summary,(code,log[-5000:])
state.update(passed=int(summary[1]),failed=int(summary[2]),ignored=int(summary[3]))
assert state['passed']==59 and state['failed']==0 and state['ignored']==3
assert all(sha(source/p)==h for p,h in source_pins.items()) and all(sha(p)==h for p,h in state['dependency_pins'].items())
state['pins'].update({str(p):sha(p)for p in [F/'build.log',F/'inventory.log',F/'tests.log']})
state['status']='passed_pending_audit';save();print(O)
