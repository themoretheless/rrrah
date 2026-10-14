#!/usr/bin/env python3
"""Instrument actual distance work; counters are not timing or throughput evidence."""
from pathlib import Path
import hashlib,json,subprocess,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior_path=D/'dedup-gradient-topk-experiment-20261014.json';prior=json.loads(prior_path.read_text())
assert prior['status']=='synthetic_correctness_passed' and all(sha(p)==h for p,h in prior['pins'].items())
runner_report=D/'dedup-gradient-runner-up-experiment-20261014.json';rr=json.loads(runner_report.read_text())
assert rr['returncode']==0 and all(sha(p)==h for p,h in rr['pins'].items())
original=Path('/private/tmp/rrrah-gradient-topk-experiment-20261014/experiment.rs')
runner_source=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-runner-up-experiment-20261014/experiment.rs')
s=original.read_text();a=s.index('fn main()');b=s.index('\nfn read_dump',a);s=s[:a]+s[b:]
r=runner_source.read_text();a=r.index('fn candidate(');b=r.index('fn baseline_trace(',a)
runner=r[a:b].replace('fn candidate(','fn runner(')
s += runner
needle='fn baseline_distances(a: &GradientDescriptor, b: &[GradientFeature]) -> [f64; 4] {'
assert s.count(needle)==1
s=s.replace(needle,needle+'\n    PAIRS.with(|v|v.set(v.get()+b.len() as u64));')
s += r'''
thread_local!{static PAIRS:Cell<u64>=const{Cell::new(0)};}
fn cluster_bank()->Vec<GradientFeature>{
    (0..512).map(|i|{let cluster=i/8;let variant=i%8;let mut d=[0.;128];d[cluster]=1.;d[64+variant]=0.01;
        let norm=d.iter().map(|v|v*v).sum::<f64>().sqrt();for v in &mut d{*v/=norm;}
        GradientFeature{position:[cluster as f64*10.+variant as f64*0.01,0.],descriptor:GradientDescriptor(d)}
    }).collect()
}
fn main(){let args:Vec<_>=std::env::args().skip(1).collect();let (left,right)=if args.len()==2{(read_dump(&args[0]),read_dump(&args[1]))}else{let bank=cluster_bank();(bank.clone(),bank)};
    let policy=GradientMatchPolicy{max_comparisons:10000000,max_squared_distance:2.,squared_ratio:0.8};
    let expected=baseline_trace(&left,&right,policy,Some(4.),||false).unwrap();
    assert_eq!(expected,candidate_trace(&left,&right,policy,Some(4.),||false).unwrap());
    for version in 0..3{PAIRS.with(|v|v.set(0));let calls=Cell::new(0usize);
        let cancel=||{calls.set(calls.get()+1);false};
        let output=match version{0=>baseline(&left,&right,policy,Some(4.),&cancel),1=>runner(&left,&right,policy,Some(4.),&cancel),_=>candidate(&left,&right,policy,Some(4.),&cancel)}.unwrap();
        assert_eq!(output,expected.0);
        println!("{{\"version\":{},\"distance_pairs\":{},\"callbacks\":{},\"matches\":{},\"left\":{},\"right\":{}}}",version,PAIRS.with(Cell::get),calls.get(),output.len(),left.len(),right.len());
    }
}
'''
F=Path('/private/tmp/rrrah-gradient-topk-work-20261014');F.mkdir(exist_ok=False)
src=F/'work.rs';src.write_text(s);exe=F/'work'
natural_path=D/'dedup-gradient-unroll4-natural-cpu-20261014.json';natural=json.loads(natural_path.read_text())
assert all(sha(p)==h for p,h in natural['feature_pins'].items())
O=D/'dedup-gradient-topk-work-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),pins={str(p):sha(p) for p in [prior_path,runner_report,original,runner_source,src,Path(__file__),natural_path]},feature_pins=natural['feature_pins'],results=[],scope='Instrumented standalone original matcher, exact global runner-up and sorted top16, all with same original scalar kernel. Counts actual evaluated descriptor pairs including partial4blocks. One artificial512descriptor64clustersx8variants self bank and eight retained natural directions. Exact correspondence parity, top16 nearest bit parity, callbacks invariant. Counters are not CPU speedups; no production/managed/native/default/corpus claims.')
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
with (F/'build.log').open('x') as log:
    p=subprocess.Popen(['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'-o',str(exe)],stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
assert code==0,(F/'build.log').read_text()
state['pins'][str(exe)]=sha(exe)
cases=[dict(label='synthetic_clustered_self',inputs=[])]+[dict(label=f"natural_{c['index']}",inputs=c['inputs']) for c in natural['cases']]
for case in cases:
    out=F/(case['label']+'.jsonl');err=F/(case['label']+'.stderr');cmd=[str(exe),*case['inputs']]
    with out.open('x') as stdout,err.open('x') as stderr:
        p=subprocess.Popen(cmd,stdout=stdout,stderr=stderr);state.update(status='running',active_case=case['label'],child_pid=p.pid);save();code=p.wait()
    rows=[json.loads(l) for l in out.read_text().splitlines()]
    state['results'].append(dict(**case,command=cmd,returncode=code,stdout=str(out),stdout_sha256=sha(out),stderr=str(err),stderr_sha256=sha(err),rows=rows));save()
    assert code==0,(case,err.read_text())
    assert len(rows)==3 and len({r['callbacks'] for r in rows})==1 and len({r['matches'] for r in rows})==1
    assert rows[0]['distance_pairs']==2*rows[0]['left']*rows[0]['right']
    assert rows[2]['distance_pairs']<=rows[1]['distance_pairs']<=rows[0]['distance_pairs']
assert all(sha(p)==h for p,h in state['pins'].items()) and all(sha(p)==h for p,h in state['feature_pins'].items())
state['status']='passed_pending_audit';save();print(O)
