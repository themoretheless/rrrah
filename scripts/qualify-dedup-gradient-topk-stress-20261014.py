#!/usr/bin/env python3
"""Exact standalone stress qualification; retains all original callback semantics."""
from pathlib import Path
import ast
import hashlib
import json
import os
import subprocess

R=Path(__file__).resolve().parents[1]
D=R/'docs/research'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior_path=D/'dedup-gradient-topk-experiment-20261014.json'
prior=json.loads(prior_path.read_text())
assert prior['status']=='synthetic_correctness_passed'
assert all(sha(p)==h for p,h in prior['pins'].items())
original=Path('/private/tmp/rrrah-gradient-topk-experiment-20261014/experiment.rs')
s=original.read_text()
a=s.index('fn main()')
b=s.index('\nfn read_dump',a)
s=s[:a]+s[b:]
s += r'''
fn check(left:&[GradientFeature],right:&[GradientFeature],radius:Option<f64>,exhaustive:bool)->usize {
    let policy=GradientMatchPolicy{max_comparisons:10000000,max_squared_distance:2.,squared_ratio:0.8};
    let count=Cell::new(0usize);
    let expected=baseline_trace(left,right,policy,radius,||{count.set(count.get()+1);false});
    let calls=count.get();count.set(0);
    let actual=candidate_trace(left,right,policy,radius,||{count.set(count.get()+1);false});
    assert_eq!(expected,actual,"nearest/correspondence mismatch: n={} m={} radius={:?}",left.len(),right.len(),radius);
    assert_eq!(calls,count.get());
    let stops:Vec<_>=if exhaustive {(1..=calls).collect()} else {vec![1,calls/2+1,calls]};
    for stop in &stops {
        count.set(0);
        let expected=baseline_trace(left,right,policy,radius,||{count.set(count.get()+1);count.get()==*stop});
        let before=count.get();count.set(0);
        let actual=candidate_trace(left,right,policy,radius,||{count.set(count.get()+1);count.get()==*stop});
        assert_eq!(expected,actual);assert_eq!(before,count.get());
    }
    stops.len()
}
fn main() {
    let args:Vec<_>=std::env::args().skip(1).collect();
    if args.len()==2 {
        let left=read_dump(&args[0]);let right=read_dump(&args[1]);
        for radius in [None,Some(4.)] {
            let cancellations=check(&left,&right,radius,false);
            println!("{{\"kind\":\"natural_parity\",\"left\":{},\"right\":{},\"distinct\":{},\"cancellations\":{}}}",left.len(),right.len(),radius.is_some(),cancellations);
        }
        return;
    }
    let mut cases=0;let mut cancellations=0;
    for seed in 0..512 {
        let mut left=fixture(seed%49,seed%4,173+seed as u64);
        let mut right=fixture((seed*7)%67,(seed/4)%4,991+seed as u64);
        if seed%3==0 {
            for f in left.iter_mut().chain(&mut right) {f.position[0]=(f.position[0] as usize%9)as f64;f.position[1]=0.;}
        }
        if seed%5==0 && !left.is_empty() {
            for (i,f) in right.iter_mut().enumerate(){f.descriptor=left[i%left.len()].descriptor;}
        }
        for radius in [None,Some(0.),Some(4.),Some(625.),Some(1e9)] {cancellations+=check(&left,&right,radius,false);cases+=1;}
    }
    for mode in 0..4 {let left=fixture(5,mode,173);let right=fixture(7,mode,991);
        for radius in [None,Some(0.),Some(4.),Some(625.)] {cancellations+=check(&left,&right,radius,true);cases+=1;}
    }
    println!("{{\"kind\":\"stress_parity\",\"cases\":{},\"cancellations\":{},\"exact_nearest_bits\":true,\"callback_counts\":true}}",cases,cancellations);
}
'''
F=Path('/private/tmp/rrrah-gradient-topk-stress-20261014');F.mkdir(exist_ok=False)
src=F/'stress.rs';src.write_text(s);exe=F/'stress'
natural_path=D/'dedup-gradient-unroll4-natural-cpu-20261014.json'
natural=json.loads(natural_path.read_text())
assert all(sha(p)==h for p,h in natural['feature_pins'].items())
O=D/'dedup-gradient-topk-stress-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),pins={str(p):sha(p) for p in [prior_path,original,src,Path(__file__),natural_path]},feature_pins=natural['feature_pins'],results=[],scope='Standalone exact top16 versus original pre-cache matcher.512seeded bank pairs with five modes/radii plus16exhaustive-callback tiny cases, then eight retained natural directions. Exact nearest table bits, correspondence, callback counts and cancellation outcomes. No production admission, default quality or whole-operation performance claim.')
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
