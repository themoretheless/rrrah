#!/usr/bin/env python3
"""Selective second-pass lanes prototype; production source remains unchanged."""
from pathlib import Path
import hashlib,json,os,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior_path=D/'dedup-gradient-topk-stress-20261014.json';prior=json.loads(prior_path.read_text())
assert prior['status']=='passed_pending_audit' and all(sha(p)==h for p,h in prior['pins'].items())
runner_report=D/'dedup-gradient-runner-up-experiment-20261014.json';rr=json.loads(runner_report.read_text())
assert rr['returncode']==0 and all(sha(p)==h for p,h in rr['pins'].items())
original=Path('/private/tmp/rrrah-gradient-topk-stress-20261014/stress.rs')
runner_source=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-runner-up-experiment-20261014/experiment.rs')
kernel_source=Path('/private/tmp/rrrah-gradient-unroll4-v7-source-20261014/crates/rrrah-dedup/src/gradient.rs')
root_kernel=R/'crates/rrrah-dedup/src/gradient.rs';assert sha(kernel_source)==sha(root_kernel)
s=original.read_text();r=runner_source.read_text()
a=r.index('fn candidate(');b=r.index('fn baseline_trace(',a)
baseline=r[a:b].replace('fn candidate(','fn baseline(')
candidate=baseline.replace('fn baseline(','fn candidate(')
needle='if forward_resolved[i] && (block*4..block*4+chunk.len()).all(|j|reverse_resolved[j]){continue;}\n                let distances = baseline_distances(&a.descriptor, chunk);'
assert candidate.count(needle)==1
replacement="""let distances = if forward_resolved[i] {
                    let mut needed = 0u8;
                    for lane in 0..chunk.len() {
                        if !reverse_resolved[block*4+lane] { needed |= 1 << lane; }
                    }
                    if needed == 0 { continue; }
                    if needed.count_ones() <= 2 {
                        let mut values = [0.;4];
                        for lane in 0..chunk.len() {
                            if needed & (1 << lane) != 0 {
                                values[lane] = a.descriptor.0.iter().zip(&chunk[lane].descriptor.0).map(|(x,y)|(x-y)*(x-y)).sum();
                            }
                        }
                        values
                    } else { baseline_distances(&a.descriptor,chunk) }
                } else { baseline_distances(&a.descriptor,chunk) };"""
candidate=candidate.replace(needle,replacement)
def traced(text,old,new):
    text=text.replace('fn '+old+'(','fn '+new+'(')
    text=text.replace('Result<Vec<crate::geometry::Correspondence>, LocalError>','Result<(Vec<crate::geometry::Correspondence>,Vec<(u64,u64,usize)>,Vec<(u64,u64,usize)>),LocalError>')
    text=text.replace('return Ok(Vec::new());','return Ok((Vec::new(),Vec::new(),Vec::new()));')
    return text.replace('Ok(output)','Ok((output,forward.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect(),reverse.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect()))')
a=s.index('fn baseline(');b=s.index('fn random(',a)
s=s[:a]+baseline+candidate+traced(baseline,'baseline','baseline_trace')+traced(candidate,'candidate','candidate_trace')+s[b:]
k=kernel_source.read_text();a=k.index('fn distances_four(');b=k.index('\nfn match_gradients_selected(',a)
k=k[a:b].replace('distances_four','baseline_distances')
a=s.index('fn baseline_distances(');b=s.index('\nfn baseline(',a)
s=s[:a]+k+s[b:]
F=Path('/private/tmp/rrrah-gradient-masked-lanes-stress-20261014');F.mkdir(exist_ok=False)
src=F/'stress.rs';src.write_text(s);exe=F/'stress'
natural_path=D/'dedup-gradient-unroll4-natural-cpu-20261014.json'
natural=json.loads(natural_path.read_text())
assert all(sha(p)==h for p,h in natural['feature_pins'].items())
O=D/'dedup-gradient-masked-lanes-stress-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),pins={str(p):sha(p) for p in [prior_path,runner_report,original,runner_source,kernel_source,src,Path(__file__),natural_path]},feature_pins=natural['feature_pins'],results=[],scope='Standalone exact existing global runner-up versus selective second-pass reverse-lane evaluation when only one or two lanes remain needed. Both use pinned current unrolled distance kernel.2576synthetic cases including16exhaustive-callback tiny cases then eight retained natural directions. Exact nearest bits, correspondence/callback/cancellation parity. No production adoption, current managed-cache integration or performance claim.')
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
with (F/'build.log').open('x') as log:
    p=subprocess.Popen(['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'-o',str(exe)],stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
assert code==0
state['pins'][str(exe)]=sha(exe)
commands=[dict(label='synthetic_stress',command=[str(exe)])]+[dict(label=f"natural_{c['index']}",command=[str(exe),*c['inputs']]) for c in natural['cases']]
for case in commands:
    log=F/(case['label']+'.jsonl');err=F/(case['label']+'.stderr')
    with log.open('x') as stdout,err.open('x') as stderr:
        p=subprocess.Popen(case['command'],stdout=stdout,stderr=stderr);state.update(status='running',active_case=case['label'],child_pid=p.pid);save();code=p.wait()
    row=dict(**case,returncode=code,stdout=str(log),stderr=str(err),stdout_sha256=sha(log),stderr_sha256=sha(err));state['results'].append(row);save()
    assert code==0,(case,err.read_text())
    for line in log.read_text().splitlines():json.loads(line)
assert all(sha(p)==h for p,h in state['pins'].items())
assert all(sha(p)==h for p,h in state['feature_pins'].items())
state['status']='passed_pending_audit';save();print(O)
