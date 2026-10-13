#!/usr/bin/env python3
"""Paired benchmark of the actual frozen public managed matcher, both budget paths."""
import json, hashlib, subprocess, os
from pathlib import Path
R=Path(__file__).resolve().parents[1]; T=Path('/Users/themoretheless/.codex/tmp'); D=R/'docs/research'
F=T/'rrrah-managed-runner-benchmark-20261014'; F.mkdir(exist_ok=False); (F/'src').mkdir()
S=T/'rrrah-gradient-runner-source-20261014'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
(F/'Cargo.toml').write_text(f'''[package]
name="managed-cache-benchmark"
version="0.0.0"
edition="2024"
[workspace]
[dependencies]
rrrah-dedup={{path="{S}/crates/rrrah-dedup",default-features=false,features=["raster"]}}
rrrah-core={{path="{S}/crates/rrrah-core"}}
serde_json="1"
[profile.release]
opt-level=3
''')
(F/'src/main.rs').write_text(r'''use rrrah_dedup::gradient::{GradientFeature,GradientDescriptor,GradientMatchPolicy,match_gradients_distinct_locations_managed};
use rrrah_core::MemoryBudget;
use std::{time::Instant,hint::black_box};
fn load(p:&str)->Vec<GradientFeature>{let b=std::fs::read(p).unwrap();let n=u64::from_le_bytes(b[..8].try_into().unwrap()) as usize;assert_eq!(b.len(),8+n*1040);b[8..].chunks_exact(1040).map(|c|{let v:Vec<f64>=c.chunks_exact(8).map(|x|f64::from_le_bytes(x.try_into().unwrap())).collect();GradientFeature{position:[v[0],v[1]],descriptor:GradientDescriptor(v[2..].try_into().unwrap())}}).collect()}
fn run(a:&[GradientFeature],b:&[GradientFeature],extra:u64)->(u128,Vec<[u64;4]>,u64){let base=((a.len()+b.len())*std::mem::size_of::<(f64,f64,usize)>()+a.len().min(b.len())*std::mem::size_of::<rrrah_dedup::geometry::Correspondence>()) as u64;let budget=MemoryBudget::new(base+extra);let t=Instant::now();let out=match_gradients_distinct_locations_managed(black_box(a),black_box(b),GradientMatchPolicy{max_comparisons:2*(a.len()*b.len()) as u64,max_squared_distance:0.5,squared_ratio:0.64},2.,&budget,||false).unwrap();let ns=t.elapsed().as_nanos();let bits=out.iter().map(|p|[p.source[0].to_bits(),p.source[1].to_bits(),p.target[0].to_bits(),p.target[1].to_bits()]).collect();drop(out);assert_eq!(budget.used(),0);assert!(budget.peak()<=budget.limit());if extra>0{assert!(budget.peak()>=base+((a.len()+b.len())*std::mem::size_of::<(usize,bool)>()) as u64);}else{assert_eq!(budget.peak(),base);} (ns,bits,budget.peak())}
fn main(){let args:Vec<_>=std::env::args().collect();let a=load(&args[1]);let b=load(&args[2]);let (_,expected,_)=run(&a,&b,0);assert_eq!(run(&a,&b,16*1024*1024).1,expected);for i in 0..9{let (off,on)=if i%2==0{let off=run(&a,&b,0);(off,run(&a,&b,16*1024*1024))}else{let on=run(&a,&b,16*1024*1024);(run(&a,&b,0),on)};assert_eq!(off.1,expected);assert_eq!(on.1,expected);println!("{}",serde_json::json!({"pair":i,"uncached_ns":off.0,"cached_ns":on.0,"ratio":on.0 as f64/off.0 as f64,"matches":expected.len(),"uncached_peak":off.2,"cached_peak":on.2,"bits_identical":true,"released":true}));}}
''')
cases=[dict(index=0,label='derived_large_self',inputs=[str(T/'rrrah-managed-large-features-20261014/derived-1537.bin')]*2)]
pins={str(p):sha(p) for p in [Path(__file__),F/'Cargo.toml',F/'src/main.rs',S/'crates/rrrah-dedup/src/gradient.rs',S/'crates/rrrah-dedup/src/local_scan.rs']}
for c in cases:
 for p in c['inputs']:pins[p]=sha(p)
O=D/'dedup-managed-runner-benchmark-20261014.json';assert not O.exists()
d=dict(status='building',pid=os.getpid(),source=str(S),pins=pins,cases=cases,results=[],scope='Actual managed public matcher on frozen runner source; derived1537repeated natural bank, nine alternating budget-path pairs, distance0.5 ratio0.64 radius2. Matrix exceeds16MiBcache cap. Component timing including production linear scratch admission/allocation; not fresh native extraction/whole search timing. Shared host.')
def save():
 q=O.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(O)
save(); target=F/'target'
with (F/'build.log').open('w') as log:
 rc=subprocess.run(['cargo','+stable','build','--release','--offline','--manifest-path',str(F/'Cargo.toml'),'--target-dir',str(target)],stdout=log,stderr=subprocess.STDOUT).returncode
d['build_returncode']=rc
if rc:d['status']='build_failed';save();raise SystemExit(rc)
exe=target/'release/managed-cache-benchmark';d['executable_sha256']=sha(exe);d['status']='running';save()
for c in cases:
 r=subprocess.run([str(exe)]+c['inputs'],capture_output=True,text=True);out=F/f"case-{c['index']:02}.json";out.write_text(json.dumps(dict(case=c,returncode=r.returncode,stdout=r.stdout,stderr=r.stderr),indent=2)+'\n');d['results'].append(dict(index=c['index'],report=str(out),sha256=sha(out),returncode=r.returncode));save()
 if r.returncode:d['status']='execution_failed';save();raise SystemExit(r.returncode)
 rows=[json.loads(l) for l in r.stdout.splitlines()];assert len(rows)==9 and all(x['bits_identical'] and x['released'] for x in rows)
assert all(sha(p)==h for p,h in pins.items());d['status']='complete';save()
print(json.dumps([dict(index=r['index'],median_ratio=sorted(json.loads(l)['ratio'] for l in json.loads(Path(r['report']).read_text())['stdout'].splitlines())[4]) for r in d['results']]))
