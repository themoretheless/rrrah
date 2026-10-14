#!/usr/bin/env python3
"""Isolated first-pass16 integration; preserve second-pass4 skip granularity."""
from pathlib import Path
import hashlib,json,os,subprocess,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
root_gate_path=D/'dedup-duration-default-gate-20261014.json';root_gate=json.loads(root_gate_path.read_text());assert root_gate['status']=='passed_pending_audit'and root_gate['passed']==61
assert all(sha(R/p)==h for p,h in root_gate['source_sha256'].items())and all(sha(p)==h for p,h in root_gate['dependency_pins'].items())
primitive_path=D/'dedup-gradient-lanes8-16-cpu-audit-20261014.json';primitive=json.loads(primitive_path.read_text());assert primitive['status']=='verified_bounded_independent_lane_kernel_gain_pending_integration'
prototype=Path('/private/tmp/rrrah-gradient-lanes8-16-cpu-20261014/experiment.rs');prototype_report=json.loads((D/'dedup-gradient-lanes8-16-cpu-20261014.json').read_text());assert sha(prototype)==prototype_report['pins'][str(prototype)]
F=Path('/private/tmp/rrrah-gradient-lanes16-managed-20261014');F.mkdir(exist_ok=False)
baseline=F/'baseline-source';source=F/'candidate-source'
for dest in [baseline,source]:
    crate=dest/'crates/rrrah-dedup';crate.parent.mkdir(parents=True);subprocess.run(['cp','-cR',str(R/'crates/rrrah-dedup'),str(crate)],check=True)
    for p in crate.rglob('*'):
        if p.is_file():p.chmod(p.stat().st_mode|0o200)
assert all(sha(baseline/p)==h==sha(source/p)for p,h in root_gate['source_sha256'].items())
crate=source/'crates/rrrah-dedup';gradient=crate/'src/gradient.rs';text=gradient.read_text();proto=prototype.read_text();a=proto.index('fn lanes16(');b=proto.index('\nfn feature',a)
helper=proto[a:b].replace('fn lanes16(','fn distances_sixteen(').replace('four(a,chunk)','distances_four(a,chunk)')
index=text.index('\nfn match_gradients_selected(');text=text[:index]+'\n// Sixteen independent lanes preserve every per-pair addition order.\n// Partial blocks reuse the exact four-lane/scalar paths without padding work.\n'+helper+'\n'+text[index:]
a=text.index('    for (i, a) in left.iter().enumerate() {',text.index('fn match_gradients_selected_with_scratch<'));b=text.index('    if let Some(radius) = distinct_radius_squared {',a)
first=text[a:b];assert first.count('right.chunks(4)')==1 and first.count('distances_four(&a.descriptor, chunk)')==1
first=first.replace('right.chunks(4)','right.chunks(16)').replace('block * 4','block * 16').replace('distances_four(&a.descriptor, chunk)','distances_sixteen(&a.descriptor, chunk)')
text=text[:a]+first+text[b:]
text=text.replace('in blocks of at most four; every pair retains its cancellation checkpoint','in first-pass blocks of at most sixteen; every pair retains its cancellation checkpoint')
gradient.write_text(text)
test=crate/'src/gradient_matching_tests.rs';body=test.read_text();a=body.index('#[test]');b=body.index('\nfn exhaustive(',a);existing=body[a:b]
assert existing.count('fn unrolled_distance_blocks_preserve_scalar_bits_and_partial_lanes')==1
added=existing.replace('fn unrolled_distance_blocks_preserve_scalar_bits_and_partial_lanes','fn sixteen_descriptor_lanes_preserve_scalar_bits_and_every_tail').replace('[GradientFeature; 4]','[GradientFeature; 16]').replace('0..=4','0..=16').replace('distances_four(&a,','distances_sixteen(&a,')
test.write_text(body[:b]+'\n'+added+body[b:])
subprocess.run(['rustfmt','+stable','--edition=2024','--config','skip_children=true',str(gradient),str(test)],check=True)
pins={str(p.relative_to(source)):sha(p)for p in crate.rglob('*')if p.is_file()};changed={p for p,h in pins.items()if sha(baseline/p)!=h};assert changed=={'crates/rrrah-dedup/src/gradient.rs','crates/rrrah-dedup/src/gradient_matching_tests.rs'}
command=root_gate['command'].copy();command[command.index(str(R/'crates/rrrah-dedup/src/lib.rs'))]=str(crate/'src/lib.rs');command[command.index('-o')+1]=str(F/'tests')
env=os.environ.copy();env['CARGO_MANIFEST_DIR']=str(crate)
O=D/'dedup-gradient-lanes16-managed-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),source=str(source),source_sha256=pins,baseline_source=str(baseline),baseline_source_sha256=root_gate['source_sha256'],command=command,dependency_pins=root_gate['dependency_pins'],pins={str(p):sha(p)for p in [Path(__file__),root_gate_path,primitive_path,prototype]},scope='Isolated actual current-root dedup clone; first-pass16independent descriptor lanes only, second-pass4unchanged to preserve runner skip granularity/work. No new heap scratch or cache/admission policy changes. Actual GradientFeature includes position and descriptor,1040byte stride, unlike descriptor-only1024byte primitive benchmark stub. Existing complete raster unit suite plus256fixture16lane all-tail bit regression. Directrustc real cached deps, not Cargo/decode/native/workspace/currentcorpus/default/platform proof; working runtime kernel unchanged.')
def save():
    temp=O.with_suffix('.tmp');temp.write_text(json.dumps(state,indent=2)+'\n');temp.replace(O)
save()
with (F/'build.log').open('x')as log:
    p=subprocess.Popen(command,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
state['build_returncode']=code;save();assert code==0,(F/'build.log').read_text()[-5000:];exe=F/'tests';state['pins'][str(exe)]=sha(exe)
with (F/'inventory.log').open('x')as log:
    p=subprocess.Popen([str(exe),'--list'],stdout=log,stderr=subprocess.STDOUT);state.update(status='listing',child_pid=p.pid);save();assert p.wait()==0
with (F/'tests.log').open('x')as log:
    p=subprocess.Popen([str(exe),'--test-threads','2'],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT);state.update(status='testing',child_pid=p.pid);save();code=p.wait()
state['test_returncode']=code;save();log=(F/'tests.log').read_text();summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',log);assert code==0 and summary
assert (int(summary[1]),int(summary[2]),int(summary[3]))==(62,0,3)
assert 'sixteen_descriptor_lanes_preserve_scalar_bits_and_every_tail ... ok'in log
assert all(sha(source/p)==h for p,h in pins.items())and all(sha(baseline/p)==h for p,h in root_gate['source_sha256'].items())and all(sha(p)==h for p,h in root_gate['dependency_pins'].items())
state.update(status='passed_pending_audit',passed=62,failed=0,ignored=3);state['pins'].update({str(p):sha(p)for p in [F/'build.log',F/'inventory.log',F/'tests.log']});save();print(O)
