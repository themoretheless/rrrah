#!/usr/bin/env python3
"""Alternating fresh-process CPU comparison against existing exact runner-up."""
from pathlib import Path
import hashlib,json,subprocess,resource,time,statistics,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
stress_path=D/'dedup-gradient-masked-lanes-stress-20261014.json';stress=json.loads(stress_path.read_text())
assert stress['status']=='passed_pending_audit' and all(sha(p)==h for p,h in stress['pins'].items())
work_source=Path('/private/tmp/rrrah-gradient-masked-lanes-stress-20261014/stress.rs')
s=work_source.read_text()
cluster_source=Path('/private/tmp/rrrah-gradient-topk-work-20261014/work.rs').read_text()
a=cluster_source.index('fn cluster_bank(');b=cluster_source.index('fn main()',a)
cluster=cluster_source[a:b]
a=s.index('fn main()');s=s[:a]+cluster+r'''
fn main(){let args:Vec<_>=std::env::args().skip(1).collect();assert!(args.len()==1||args.len()==3);
    let (left,right)=if args.len()==3{(read_dump(&args[1]),read_dump(&args[2]))}else{let b=cluster_bank();(b.clone(),b)};
    let policy=GradientMatchPolicy{max_comparisons:10000000,max_squared_distance:2.,squared_ratio:0.8};
    let mut retained=Vec::new();
    for _ in 0..4 {retained=if args[0]=="runner"{baseline(black_box(&left),black_box(&right),policy,Some(4.),||false)}else{assert_eq!(args[0],"masked");candidate(black_box(&left),black_box(&right),policy,Some(4.),||false)}.unwrap();black_box(&retained);}
    println!("{}",retained.len());for p in retained{println!("{} {} {} {}",p.source[0].to_bits(),p.source[1].to_bits(),p.target[0].to_bits(),p.target[1].to_bits());}
}
'''
F=Path('/private/tmp/rrrah-gradient-masked-lanes-cpu-20261014');F.mkdir(exist_ok=False);src=F/'cpu.rs';src.write_text(s);exe=F/'cpu'
natural_path=D/'dedup-gradient-unroll4-natural-cpu-20261014.json';natural=json.loads(natural_path.read_text())
assert all(sha(p)==h for p,h in natural['feature_pins'].items())
cases=[dict(label='synthetic_clustered_self',inputs=[])]+[dict(label=f"natural_{i}",inputs=natural['cases'][i]['inputs']) for i in [0,5]]
O=D/'dedup-gradient-masked-lanes-cpu-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),pins={str(p):sha(p) for p in [stress_path,work_source,src,Path(__file__),natural_path]},feature_pins=natural['feature_pins'],cases=cases,results=[],scope='Standalone exact global runner-up versus selective second-pass lanes, both use pinned current unrolled kernel, four matcher repetitions per fresh process,10alternating pairs with first warmup excluded. Two retained natural1024feature directions plus artificial512clustered self bank. CPU includes startup/loading/printing. No managed cache, native whole-operation, allocation cap, universal speedup or production adoption claim.')
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
with (F/'build.log').open('x') as log:
    p=subprocess.Popen(['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'-o',str(exe)],stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
assert code==0,(F/'build.log').read_text()
state['pins'][str(exe)]=sha(exe)
for case in cases:
    rows=[];expected=None
    for pair in range(10):
        for version in (['runner','masked'] if pair%2==0 else ['masked','runner']):
            out=F/f"{case['label']}-{pair}-{version}.stdout";err=out.with_suffix('.stderr')
            command=[str(exe),version,*case['inputs']]
            with out.open('x') as stdout,err.open('x') as stderr:
                before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.perf_counter()
                p=subprocess.Popen(command,stdout=stdout,stderr=stderr);state.update(status='timing',active_case=case['label'],active_pair=pair,active_version=version,child_pid=p.pid);save();code=p.wait()
                elapsed=time.perf_counter()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN)
            if expected is None:expected=out.read_bytes()
            assert code==0 and out.read_bytes()==expected,(case,version,err.read_text())
            rows.append(dict(pair=pair,version=version,command=command,returncode=code,cpu_seconds=after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,wall_seconds=elapsed,stdout=str(out),stdout_sha256=sha(out),stderr=str(err),stderr_sha256=sha(err)))
            state.update(active_rows=rows);save()
    ratios=[]
    for pair in range(1,10):
        cpu={r['version']:r['cpu_seconds'] for r in rows if r['pair']==pair};assert cpu['runner']>0;ratios.append(cpu['masked']/cpu['runner'])
    state['results'].append(dict(**case,rows=rows,ratios=ratios,median_cpu_ratio=statistics.median(ratios)));save()
assert all(sha(p)==h for p,h in state['pins'].items()) and all(sha(p)==h for p,h in state['feature_pins'].items())
state.pop('active_rows',None);state['status']='complete_pending_audit';save();print([(r['label'],r['median_cpu_ratio'])for r in state['results']])
