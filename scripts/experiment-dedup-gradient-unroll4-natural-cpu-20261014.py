#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import os
import resource
import statistics
import subprocess
import time

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior_path = D / 'dedup-gradient-unroll4-cpu-20261014.json'
prior = json.loads(prior_path.read_text())
assert all(sha(p) == h for p, h in prior['pins'].items())
old_path = D / 'dedup-gradient-natural-bound-benchmark-20261013.json'
old = json.loads(old_path.read_text())
old_plan = D / 'dedup-gradient-natural-bound-benchmark-plan-20261013.json'
assert sha(old_plan) == old['plan_sha256']
plan = json.loads(old_plan.read_text())
assert all(sha(p) == h for p, h in plan['pins'].items())
features = {row['input']:row for row in old['extractions']}
assert all(row['returncode'] == 0 and sha(row['features_file']) == row['features_sha256'] for row in features.values())
cases = []
for row in old['results']:
    assert sha(row['report']) == row['sha256']
    case = json.loads(Path(row['report']).read_text())
    assert case['command'][1:3] == [features[case[key]]['features_file'] for key in ['left','right']]
    cases.append(dict(index=case['index'], label=case['label'], inputs=case['command'][1:3], source_images=[case['left'],case['right']]))
assert len(cases) == 8
F = Path('/private/tmp/rrrah-gradient-unroll4-natural-cpu-20261014')
F.mkdir(exist_ok=False)
source_text = Path('/private/tmp/rrrah-gradient-unroll4-cpu-20261014/experiment.rs').read_text()
main = r'''
fn load(path:&str)->Vec<GradientFeature> {
    let bytes=std::fs::read(path).unwrap();let count=u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(),8+count*1040);
    bytes[8..].chunks_exact(1040).map(|record| {
        let values:Vec<f64>=record.chunks_exact(8).map(|v|f64::from_le_bytes(v.try_into().unwrap())).collect();
        GradientFeature{position:[values[0],values[1]],descriptor:GradientDescriptor(values[2..].try_into().unwrap())}
    }).collect()
}
fn main() {
    let args:Vec<_>=std::env::args().collect();let left=load(&args[2]);let right=load(&args[3]);
    let mut checksum=0u64;
    if args[1]=="verify" {
        for a in &left {for chunk in right.chunks(4) {
            let baseline=distances_four(&a.descriptor,chunk);let changed=candidate(&a.descriptor,chunk);
            assert_eq!(baseline.map(f64::to_bits),changed.map(f64::to_bits));
            for value in baseline {checksum=checksum.wrapping_add(value.to_bits());}
        }}
        println!("{} {}",left.len()*right.len(),checksum);
    } else {
        for _ in 0..2 {let (_,sum)=run(&left,&right,if args[1]=="candidate" {candidate}else{distances_four});checksum=checksum.wrapping_add(sum);}
        println!("{}",checksum);
    }
}
'''
source = F / 'experiment.rs'
source.write_text(source_text[:source_text.index('fn main() {')] + main)
binary = F / 'experiment'
OUT = D / 'dedup-gradient-unroll4-natural-cpu-20261014.json'
assert not OUT.exists()
state = dict(status='building',producer_pid=os.getpid(),cases=cases,results=[],pins={str(p):sha(p) for p in [prior_path,old_path,old_plan,source,Path(__file__)]},feature_pins={row['features_file']:row['features_sha256'] for row in features.values()},scope='Retained natural1024-feature banks, eight directions/seven unique ordered pairs, CPU kernel timing only. Exact all-pair distance bits checked separately. No fresh extraction, complete matcher/cancellation/native search or corpus qualification; production unchanged.')

def save():
    temporary=OUT.with_suffix('.tmp');temporary.write_text(json.dumps(state,indent=2)+'\n');temporary.replace(OUT)

save()
with (F/'build.log').open('x') as log:
    child=subprocess.Popen(['rustc','+stable','--edition','2024','-C','opt-level=3',str(source),'-o',str(binary)],stdout=log,stderr=subprocess.STDOUT)
    state['child_pid']=child.pid;save();code=child.wait()
assert code==0
state['pins'][str(binary)]=sha(binary)
for case in cases:
    state.update(status='verifying',active_index=case['index']);save()
    child=subprocess.Popen([str(binary),'verify',*case['inputs']],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    state['child_pid']=child.pid;save();stdout,stderr=child.communicate();assert child.returncode==0,stderr
    count, checksum=map(int,stdout.split());expected=str((checksum*2)%(1<<64));rows=[]
    for pair in range(10):
        for version in (['baseline','candidate'] if pair%2==0 else ['candidate','baseline']):
            before=resource.getrusage(resource.RUSAGE_CHILDREN);wall=time.perf_counter()
            child=subprocess.Popen([str(binary),version,*case['inputs']],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            state.update(status='timing',child_pid=child.pid,active_pair=pair,active_version=version);save()
            stdout,stderr=child.communicate();elapsed=time.perf_counter()-wall;after=resource.getrusage(resource.RUSAGE_CHILDREN)
            assert child.returncode==0 and stdout.strip()==expected,(case,version,stderr)
            rows.append(dict(pair=pair,version=version,cpu_seconds=after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,wall_seconds=elapsed,checksum=expected))
    ratios=[]
    for pair in range(1,10):
        values={row['version']:row['cpu_seconds'] for row in rows if row['pair']==pair};assert values['baseline']>0;ratios.append(values['candidate']/values['baseline'])
    state['results'].append(dict(index=case['index'],label=case['label'],bitwise_distance_pairs=count,rows=rows,median_cpu_ratio=statistics.median(ratios),ratios=ratios));save()
assert all(sha(p)==h for p,h in state['pins'].items()) and all(sha(p)==h for p,h in state['feature_pins'].items())
state.update(status='completed_pending_audit_no_adoption');save()
print([(row['index'],row['median_cpu_ratio']) for row in state['results']])
