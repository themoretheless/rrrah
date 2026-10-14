#!/usr/bin/env python3
"""Pure ordered-loop variants against current unroll4, no matcher-policy changes."""
from pathlib import Path
import hashlib,json,subprocess,os,resource,time,statistics
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest_path=D/'dedup-gradient-unroll4-v7-source-20261014.json';m=json.loads(manifest_path.read_text());kernel=Path(m['source'])/'crates/rrrah-dedup/src/gradient.rs';assert sha(kernel)==m['source_sha256']['crates/rrrah-dedup/src/gradient.rs']
s=kernel.read_text();a=s.index('fn distances_four(');b=s.index('\nfn match_gradients_selected(',a);baseline=s[a:b];start=baseline.index('    for base in (0..128).step_by(4) {');end=baseline.index('    [s0, s1, s2, s3]',start)
variants=[]
for width in [8,16]:
    block='    for base in (0..128).step_by('+str(width)+') {\n'
    for offset in range(width):
        block+='        {\n            let k = base + '+str(offset)+';\n            let x = a.0[k];\n'
        for lane in range(4):block+=f'            let d{lane} = x - b{lane}[k];\n            s{lane} += d{lane} * d{lane};\n'
        block+='        }\n'
    block+='    }\n'
    variants.append('#[inline(never)]\n'+(baseline[:start]+block+baseline[end:]).replace('fn distances_four(','fn unroll'+str(width)+'('))
source='''use std::{hint::black_box,io};
#[derive(Clone,Copy)]struct GradientDescriptor([f64;128]);
#[derive(Clone,Copy)]struct GradientFeature{descriptor:GradientDescriptor}
'''+ '#[inline(never)]\n'+baseline+''.join(variants)+r'''
fn feature(seed:&mut u64)->GradientFeature{let mut d=[0.;128];for v in &mut d{*seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);*v=((*seed>>11)as f64)/((1u64<<53)as f64);}let norm=d.iter().map(|v|v*v).sum::<f64>().sqrt();for v in &mut d{*v/=norm;}GradientFeature{descriptor:GradientDescriptor(d)}}
fn load(path:&str)->Vec<GradientFeature>{let bytes=std::fs::read(path).unwrap();let n=u64::from_le_bytes(bytes[..8].try_into().unwrap())as usize;assert_eq!(bytes.len(),8+n*1040);bytes[8..].chunks_exact(1040).map(|b|GradientFeature{descriptor:GradientDescriptor(std::array::from_fn(|i|f64::from_le_bytes(b[16+i*8..24+i*8].try_into().unwrap())))}).collect()}
fn main()->Result<(),io::Error>{let args:Vec<_>=std::env::args().skip(1).collect();assert_eq!(args.len(),3);let left=load(&args[1]);let right=load(&args[2]);
 if args[0]=="verify"{let mut seed=173;let mut synthetic=0;for _ in 0..2048{let a=feature(&mut seed);let b:[_;4]=std::array::from_fn(|_|feature(&mut seed));for n in 0..=4{let original=distances_four(&a.descriptor,&b[..n]).map(f64::to_bits);for f in [unroll8 as fn(&GradientDescriptor,&[GradientFeature])->[f64;4],unroll16]{assert_eq!(original,f(&a.descriptor,&b[..n]).map(f64::to_bits));synthetic+=n;}for lane in 0..n{let scalar=a.descriptor.0.iter().zip(&b[lane].descriptor.0).map(|(x,y)|(x-y)*(x-y)).sum::<f64>();assert_eq!(original[lane],scalar.to_bits());}}}
 for a in &left{for block in right.chunks(4){let expected=distances_four(&a.descriptor,block).map(f64::to_bits);assert_eq!(expected,unroll8(&a.descriptor,block).map(f64::to_bits));assert_eq!(expected,unroll16(&a.descriptor,block).map(f64::to_bits));}}
 println!("{} {}",synthetic,2*left.len()*right.len());return Ok(());}
 let f:fn(&GradientDescriptor,&[GradientFeature])->[f64;4]=match args[0].as_str(){"4"=>distances_four,"8"=>unroll8,"16"=>unroll16,_=>panic!("version")};let mut checksum=0u64;
 for _ in 0..2{for a in &left{for block in right.chunks(4){for value in f(black_box(&a.descriptor),black_box(block)){checksum=checksum.wrapping_add(value.to_bits());}}}}
 println!("{}",checksum);Ok(())}
'''
fixture_path=D/'dedup-natural-large-gradient-banks-v2-20261014.json';fixture=json.loads(fixture_path.read_text());assert fixture['status']=='complete_pending_audit';inputs=[e['features']for e in fixture['extractions']]
assert all(sha(e['features'])==e['features_sha256']for e in fixture['extractions'])
F=Path('/private/tmp/rrrah-gradient-unroll8-16-cpu-20261014');F.mkdir(exist_ok=False);src=F/'experiment.rs';src.write_text(source);exe=F/'experiment'
O=D/'dedup-gradient-unroll8-16-cpu-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),pins={str(p):sha(p)for p in [Path(__file__),manifest_path,kernel,fixture_path,src]},input_pins={p:sha(p)for p in inputs},results=[],scope='Pure ordered kernel loop unroll8and16 versus pinned currentunroll4, same scalar addition order/tail lanes, no matcher/cancellation/heap changes.2048synthetic fixtures0..4lanes versus scalar and natural2048x2048pair full bit parity. Sixty fresh processes, two kernel repetitions each,10rotated/alternating three-version groups with warmup excluded. Function-pointer/noinline kernel component only, not actual managed/native/default/fulloperation/platform or production adoption.')
def save():
    temp=O.with_suffix('.tmp');temp.write_text(json.dumps(state,indent=2)+'\n');temp.replace(O)
save()
with (F/'build.log').open('x')as log:
    p=subprocess.Popen(['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'-o',str(exe)],stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
assert code==0,(F/'build.log').read_text();state['pins'][str(exe)]=sha(exe)
with (F/'verify.stdout').open('x')as out,(F/'verify.stderr').open('x')as err:
    p=subprocess.Popen([str(exe),'verify',*inputs],stdout=out,stderr=err);state.update(status='verifying',child_pid=p.pid);save();code=p.wait()
assert code==0 and (F/'verify.stderr').read_bytes()==b''
synthetic,natural=map(int,(F/'verify.stdout').read_text().split());assert synthetic==40960 and natural==8388608;state['bitwise_controls']=dict(synthetic_distance_values=synthetic,natural_distance_values=natural);save()
canonical=None
# Twenty cyclically rotated groups: two warmup/measurement rounds of each order,
# so each version occurs equally often in every process position.
for group in range(20):
    order=['4','8','16'];offset=group%3;order=order[offset:]+order[:offset]
    if group%2:order=order[::-1]
    for version in order:
        out=F/f'{group}-{version}.stdout';err=out.with_suffix('.stderr')
        with out.open('x')as stdout,err.open('x')as stderr:
            before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.perf_counter();p=subprocess.Popen([str(exe),version,*inputs],stdout=stdout,stderr=stderr);state.update(status='timing',active_group=group,active_version=version,child_pid=p.pid);save();code=p.wait();wall=time.perf_counter()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN)
        if canonical is None:canonical=out.read_bytes()
        assert code==0 and out.read_bytes()==canonical and err.read_bytes()==b''
        state['results'].append(dict(group=group,version=version,returncode=code,cpu_seconds=after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,wall_seconds=wall,stdout=str(out),stdout_sha256=sha(out),stderr=str(err),stderr_sha256=sha(err)));save()
summary=[]
for version in ['8','16']:
    ratios=[]
    for group in range(2,20):
        cpu={r['version']:r['cpu_seconds']for r in state['results']if r['group']==group};assert cpu['4']>0;ratios.append(cpu[version]/cpu['4'])
    summary.append(dict(version=version,ratios=ratios,median_cpu_ratio=statistics.median(ratios)))
assert all(sha(p)==h for p,h in state['pins'].items())and all(sha(p)==h for p,h in state['input_pins'].items())
state.update(status='complete_pending_audit',summary=summary);save();print(summary)
