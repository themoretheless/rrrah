#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import os
import resource
import statistics
import subprocess
import time

R=Path(__file__).resolve().parents[1];D=R/'docs/research'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
control=D/'dedup-gradient-unroll4-managed-focused-v2-20261014.json';qualified=json.loads(control.read_text());assert qualified['status']=='passed_pending_audit'
assert all(sha(p)==h for p,h in qualified['dependency_pins'].items())
deps=[]
for i,arg in enumerate(qualified['command']):
    if arg in ['--extern','-L']:deps += [arg,qualified['command'][i+1]]
core=next(value for arg,value in zip(qualified['command'],qualified['command'][1:]) if arg=='--extern' and value.startswith('rrrah_core='))
plans={name:D/file for name,file in [('baseline','dedup-gradient-unroll4-v7-source-20261014.json'),('candidate','dedup-gradient-masked-lanes-managed-20261014.json')]}
manifests={name:json.loads(path.read_text()) for name,path in plans.items()}
assert manifests['candidate']['status']=='passed_pending_audit'
changed={p for p,h in manifests['candidate']['source_sha256'].items() if sha(Path(manifests['baseline']['source'])/p)!=h}
assert changed=={'crates/rrrah-dedup/src/gradient.rs'}
for m in manifests.values():assert all(sha(Path(m['source'])/p)==h for p,h in m['source_sha256'].items())
natural=json.loads((D/'dedup-gradient-unroll4-natural-cpu-20261014.json').read_text());assert natural['status']=='completed_pending_audit_no_adoption'
table=natural['cases'][2]['inputs'];large='/Users/themoretheless/.codex/tmp/rrrah-managed-large-features-20261014/derived-1537.bin'
cases=[dict(label='table_natural',inputs=table),dict(label='derived_large_self',inputs=[large,large])]
F=Path('/private/tmp/rrrah-gradient-masked-lanes-managed-benchmark-20261014');F.mkdir(exist_ok=False)
driver=r'''use rrrah_dedup::gradient::{GradientDescriptor,GradientFeature,GradientMatchPolicy,match_gradients_distinct_locations_managed};
use rrrah_core::MemoryBudget;
fn load(path:&str)->Vec<GradientFeature>{let bytes=std::fs::read(path).unwrap();let count=u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;assert_eq!(bytes.len(),8+count*1040);bytes[8..].chunks_exact(1040).map(|record|{let v:Vec<_>=record.chunks_exact(8).map(|b|f64::from_le_bytes(b.try_into().unwrap())).collect();GradientFeature{position:[v[0],v[1]],descriptor:GradientDescriptor(v[2..].try_into().unwrap())}}).collect()}
fn main(){let args:Vec<_>=std::env::args().collect();let left=load(&args[1]);let right=load(&args[2]);let extra=args[3].parse::<u64>().unwrap();let base=((left.len()+right.len())*std::mem::size_of::<(f64,f64,usize)>()+left.len().min(right.len())*std::mem::size_of::<rrrah_dedup::geometry::Correspondence>()) as u64;let budget=MemoryBudget::new(base+extra);let output=match_gradients_distinct_locations_managed(&left,&right,GradientMatchPolicy{max_comparisons:2*(left.len()*right.len()) as u64,max_squared_distance:0.5,squared_ratio:0.64},2.,&budget,||false).unwrap();println!("{} {}",output.len(),budget.peak());for point in output.iter(){println!("{} {} {} {}",point.source[0].to_bits(),point.source[1].to_bits(),point.target[0].to_bits(),point.target[1].to_bits());}drop(output);assert_eq!(budget.used(),0);assert!(budget.peak()<=budget.limit());}
'''
(F/'probe.rs').write_text(driver)
O=D/'dedup-gradient-masked-lanes-managed-benchmark-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),cases=cases,results=[],pins={str(p):sha(p) for p in [control,*plans.values(),F/'probe.rs',Path(__file__)]},input_pins={p:sha(p) for c in cases for p in c['inputs']},dependency_pins=qualified['dependency_pins'],scope='Actual public managed distinct-location matcher, V7baseline versus isolated selective second-pass reverse-lane candidate, differing only in gradient.rs sparse-lane branch. Direct optimized rustc build with identical real cached dependencies, raster-only; no Cargo recipe/decode/native whole operation/platform/corpus claim. Natural table pair plus derived1537selfbank, with baseline-only or16MiBextra managed admission.')

def save():
    temporary=O.with_suffix('.tmp');temporary.write_text(json.dumps(state,indent=2)+'\n');temporary.replace(O)

save()
for version,m in manifests.items():
    folder=F/version;folder.mkdir();source=Path(m['source']);library=folder/'librrrah_dedup.rlib';env=os.environ.copy();env['CARGO_MANIFEST_DIR']=str(source/'crates/rrrah-dedup')
    command=['rustc','+stable','--edition=2024','--crate-name','rrrah_dedup','--crate-type','rlib',str(source/'crates/rrrah-dedup/src/lib.rs'),'--cfg','feature="raster"','-C','opt-level=3',*deps,'-o',str(library)]
    with (folder/'build.log').open('x') as log:
        child=subprocess.Popen(command,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT);state.update(child_pid=child.pid,active_version=version);save();code=child.wait()
    assert code==0,(version,'library')
    dependency_folder=next(value.split('=',1)[1] for arg,value in zip(deps,deps[1:]) if arg=='-L')
    command=['rustc','+stable','--edition=2024','-C','opt-level=3',str(F/'probe.rs'),'--extern','rrrah_dedup='+str(library),'--extern',core,'-L','dependency='+dependency_folder,'-o',str(folder/'probe')]
    with (folder/'link.log').open('x') as log:
        child=subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT);state['child_pid']=child.pid;save();code=child.wait()
    assert code==0,(version,'probe')
    state['pins'].update({str(p):sha(p) for p in [library,folder/'probe',folder/'build.log',folder/'link.log']});save()
for case in cases:
    for extra in [0,16*1024*1024]:
        rows=[];expected=None
        for pair in range(10):
            for version in (['baseline','candidate'] if pair%2==0 else ['candidate','baseline']):
                before=resource.getrusage(resource.RUSAGE_CHILDREN);wall=time.perf_counter()
                child=subprocess.Popen([str(F/version/'probe'),*case['inputs'],str(extra)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
                state.update(status='timing',child_pid=child.pid,active_case=case['label'],active_extra=extra,active_pair=pair,active_version=version);save();stdout,stderr=child.communicate();after=resource.getrusage(resource.RUSAGE_CHILDREN)
                assert child.returncode==0,stderr
                if expected is None:expected=stdout;(F/(case['label']+'-'+str(extra)+'-expected.txt')).write_text(expected)
                assert stdout==expected
                raw=F/f"{case['label']}-{extra}-{pair}-{version}.stdout";err=raw.with_suffix('.stderr');raw.write_text(stdout);err.write_text(stderr)
                rows.append(dict(stdout=str(raw),stdout_sha256=sha(raw),stderr=str(err),stderr_sha256=sha(err),pair=pair,version=version,cpu_seconds=after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,wall_seconds=time.perf_counter()-wall,output_sha256=hashlib.sha256(stdout.encode()).hexdigest()))
        ratios=[]
        for pair in range(1,10):
            cpu={r['version']:r['cpu_seconds'] for r in rows if r['pair']==pair};assert cpu['baseline']>0;ratios.append(cpu['candidate']/cpu['baseline'])
        state['results'].append(dict(case=case['label'],extra_bytes=extra,rows=rows,ratios=ratios,median_cpu_ratio=statistics.median(ratios),matches=int(expected.splitlines()[0].split()[0]),peak_bytes=int(expected.splitlines()[0].split()[1])));save()
assert all(sha(p)==h for p,h in state['pins'].items()) and all(sha(p)==h for p,h in state['input_pins'].items())
for m in manifests.values():assert all(sha(Path(m['source'])/p)==h for p,h in m['source_sha256'].items())
state['status']='complete_pending_audit';save();print([(r['case'],r['extra_bytes'],r['median_cpu_ratio']) for r in state['results']])
