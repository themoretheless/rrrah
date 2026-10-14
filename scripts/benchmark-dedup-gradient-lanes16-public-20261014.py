#!/usr/bin/env python3
"""Actual1040byte public managed matcher; include small/cache/linear/fallback cases."""
from pathlib import Path
import hashlib,json,os,subprocess,resource,time,statistics,struct
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest_path=D/'dedup-gradient-lanes16-managed-20261014.json';m=json.loads(manifest_path.read_text());assert m['status']=='passed_pending_audit'
audit_path=D/'dedup-gradient-lanes16-managed-audit-20261014.json';assert json.loads(audit_path.read_text())['passed']==62
assert all(sha(Path(m['source'])/p)==h for p,h in m['source_sha256'].items())and all(sha(Path(m['baseline_source'])/p)==h for p,h in m['baseline_source_sha256'].items())and all(sha(p)==h for p,h in m['dependency_pins'].items())
deps=[]
for k,v in zip(m['command'],m['command'][1:]):
    if k in ['--extern','-L']:deps.extend([k,v])
core=next(v for k,v in zip(m['command'],m['command'][1:])if k=='--extern'and v.startswith('rrrah_core='));dependency=next(v.split('=',1)[1]for k,v in zip(deps,deps[1:])if k=='-L')
F=Path('/private/tmp/rrrah-gradient-lanes16-public-20261014');F.mkdir(exist_ok=False)
probe=F/'probe.rs';probe.write_text(r'''use rrrah_dedup::gradient::{GradientDescriptor,GradientFeature,GradientMatchPolicy,match_gradients_distinct_locations_managed};use rrrah_core::MemoryBudget;
fn load(path:&str)->Vec<GradientFeature>{let b=std::fs::read(path).unwrap();let n=u64::from_le_bytes(b[..8].try_into().unwrap())as usize;assert_eq!(b.len(),8+n*1040);b[8..].chunks_exact(1040).map(|v|{let values:Vec<_>=v.chunks_exact(8).map(|b|f64::from_le_bytes(b.try_into().unwrap())).collect();GradientFeature{position:[values[0],values[1]],descriptor:GradientDescriptor(values[2..].try_into().unwrap())}}).collect()}
fn main(){let args:Vec<_>=std::env::args().collect();if args.get(1).is_some_and(|v|v=="--describe"){println!("{} {} {}",std::mem::size_of::<GradientFeature>(),std::env::consts::ARCH,std::env::consts::OS);return;}assert_eq!(args.len(),5);let left=load(&args[1]);let right=load(&args[2]);let extra=args[3].parse::<u64>().unwrap();let reps=args[4].parse::<usize>().unwrap();assert!(reps>0&&reps<=100000);let base=((left.len()+right.len())*std::mem::size_of::<(f64,f64,usize)>()+left.len().min(right.len())*std::mem::size_of::<rrrah_dedup::geometry::Correspondence>())as u64;let budget=MemoryBudget::new(base+extra);for round in 0..reps{let out=match_gradients_distinct_locations_managed(&left,&right,GradientMatchPolicy{max_comparisons:2*(left.len()*right.len())as u64,max_squared_distance:0.5,squared_ratio:0.64},2.,&budget,||false).unwrap();std::hint::black_box(&out);if round+1==reps{println!("{} {}",out.len(),budget.peak());for p in out.iter(){println!("{} {} {} {}",p.source[0].to_bits(),p.source[1].to_bits(),p.target[0].to_bits(),p.target[1].to_bits());}}drop(out);assert_eq!(budget.used(),0);assert!(budget.peak()<=budget.limit());}}
''')
fixtures_path=D/'dedup-natural-large-gradient-banks-v2-20261014.json';fixtures=json.loads(fixtures_path.read_text());assert fixtures['status']=='complete_pending_audit';natural=[r['features']for r in fixtures['extractions']];assert all(sha(r['features'])==r['features_sha256']for r in fixtures['extractions'])
legacy_path=D/'dedup-gradient-unroll4-natural-cpu-20261014.json';legacy=json.loads(legacy_path.read_text());table=legacy['cases'][2]['inputs'];assert all(sha(p)==legacy['feature_pins'][p]for p in table)
large='/Users/themoretheless/.codex/tmp/rrrah-managed-large-features-20261014/derived-1537.bin'
# Keep source record bytes exact; these prefix fixtures contain no repeated/padded records.
def prefix(path,count,label):
    b=Path(path).read_bytes();n=struct.unpack('<Q',b[:8])[0];assert n>=count and len(b)==8+n*1040
    p=F/(label+'.bin');p.write_bytes(struct.pack('<Q',count)+b[8:8+1040*count]);return str(p)
tiny=prefix(natural[0],1,'tiny1');small_a=prefix(natural[0],5,'small5');small_b=prefix(natural[1],7,'small7')
cases=[dict(label='tiny1_self',inputs=[tiny,tiny],repetitions=100000,extras=[0]),dict(label='small5x7',inputs=[small_a,small_b],repetitions=5000,extras=[0]),dict(label='natural_table1024',inputs=table,repetitions=1,extras=[0,16*1024*1024]),dict(label='derived1537_self',inputs=[large,large],repetitions=1,extras=[0,16*1024*1024]),dict(label='natural2048_forward',inputs=natural,repetitions=1,extras=[0,16*1024*1024]),dict(label='natural2048_reverse',inputs=natural[::-1],repetitions=1,extras=[0,16*1024*1024]),dict(label='natural2048_self',inputs=[natural[0],natural[0]],repetitions=1,extras=[0,16*1024*1024])]
O=D/'dedup-gradient-lanes16-public-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),manifest=str(manifest_path),manifest_sha256=sha(manifest_path),pins={str(p):sha(p)for p in [Path(__file__),manifest_path,audit_path,fixtures_path,legacy_path,probe]},dependency_pins=m['dependency_pins'],input_pins={p:sha(p)for c in cases for p in c['inputs']},cases=cases,builds=[],results=[],scope='Actual current-root frozen baseline firstpass4versus isolated firstpass16, secondpass4unchanged. Public managed API, real1040byte GradientFeature, identical pinned deps and policy. Seven bounded cases/12memory groups/240freshprocesses,10alternating pairs/group with warmup excluded. Tiny/small repeated calls, old natural1024densecache, derived1537and genuine2048forward/reverse/self linear/fallback. Exact coordinate bits and allocator peaks required identical, every call releases credit. No Cargo/decode/native/default/UI/fullcorpus/platform or universal speedup/adoption claim.')
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
for version,key in [('baseline','baseline_source'),('candidate','source')]:
    folder=F/version;folder.mkdir();source=Path(m[key]);library=folder/'librrrah_dedup.rlib';env=os.environ.copy();env['CARGO_MANIFEST_DIR']=str(source/'crates/rrrah-dedup')
    commands=[['rustc','+stable','--edition=2024','--crate-name','rrrah_dedup','--crate-type','rlib',str(source/'crates/rrrah-dedup/src/lib.rs'),'--cfg','feature="raster"','-C','opt-level=3',*deps,'-o',str(library)],['rustc','+stable','--edition=2024','-C','opt-level=3',str(probe),'--extern','rrrah_dedup='+str(library),'--extern',core,'-L','dependency='+dependency,'-o',str(folder/'probe')]]
    for index,command in enumerate(commands):
        log=folder/f'build-{index}.log'
        with log.open('x')as stdout:
            p=subprocess.Popen(command,cwd=source,env=env,stdout=stdout,stderr=subprocess.STDOUT);state.update(child_pid=p.pid,active_version=version);save();code=p.wait()
        state['builds'].append(dict(version=version,command=command,returncode=code,log=str(log),log_sha256=sha(log)));save();assert code==0,log.read_text()[-5000:]
    description=subprocess.run([str(folder/'probe'),'--describe'],capture_output=True,text=True,check=True);assert description.stdout.strip()=='1040 aarch64 macos'and description.stderr==''
    (folder/'describe.stdout').write_text(description.stdout);state['pins'].update({str(p):sha(p)for p in [library,folder/'probe',folder/'describe.stdout']});save()
for case in cases:
    for extra in case['extras']:
        rows=[];canonical=None
        for pair in range(10):
            for version in (['baseline','candidate']if pair%2==0 else ['candidate','baseline']):
                out=F/f"{case['label']}-{extra}-{pair}-{version}.stdout";err=out.with_suffix('.stderr');command=[str(F/version/'probe'),*case['inputs'],str(extra),str(case['repetitions'])]
                with out.open('x')as stdout,err.open('x')as stderr:
                    before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.perf_counter();p=subprocess.Popen(command,stdout=stdout,stderr=stderr);state.update(status='timing',active_case=case['label'],active_extra=extra,active_pair=pair,active_version=version,child_pid=p.pid);save();code=p.wait();wall=time.perf_counter()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN)
                if canonical is None:canonical=out.read_bytes()
                assert code==0 and out.read_bytes()==canonical and err.read_bytes()==b'',(case,version,err.read_text())
                rows.append(dict(pair=pair,version=version,command=command,returncode=code,cpu_seconds=after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,wall_seconds=wall,stdout=str(out),stdout_sha256=sha(out),stderr=str(err),stderr_sha256=sha(err)));state['active_rows']=rows;save()
        ratios=[]
        for pair in range(1,10):
            cpu={r['version']:r['cpu_seconds']for r in rows if r['pair']==pair};assert cpu['baseline']>0;ratios.append(cpu['candidate']/cpu['baseline'])
        n,peak=map(int,canonical.splitlines()[0].split());state['results'].append(dict(label=case['label'],inputs=case['inputs'],repetitions=case['repetitions'],extra_bytes=extra,matches=n,peak_bytes=peak,rows=rows,ratios=ratios,median_cpu_ratio=statistics.median(ratios)));save()
assert all(sha(p)==h for p,h in state['pins'].items())and all(sha(p)==h for p,h in state['input_pins'].items())and all(sha(Path(m['source'])/p)==h for p,h in m['source_sha256'].items())
state.pop('active_rows',None);state['status']='complete_pending_audit';save();print([(r['label'],r['extra_bytes'],r['median_cpu_ratio'])for r in state['results']])
