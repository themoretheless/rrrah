#!/usr/bin/env python3
"""More independent descriptor pairs per kernel; ordered per-pair f64 sums unchanged."""
from pathlib import Path
import hashlib,json,subprocess,os,resource,time,statistics
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest_path=D/'dedup-gradient-unroll4-v7-source-20261014.json';m=json.loads(manifest_path.read_text());kernel=Path(m['source'])/'crates/rrrah-dedup/src/gradient.rs';assert sha(kernel)==m['source_sha256']['crates/rrrah-dedup/src/gradient.rs']
s=kernel.read_text();a=s.index('fn distances_four(');b=s.index('\nfn match_gradients_selected(',a);baseline=s[a:b].replace('fn distances_four(','fn four(')
functions=[]
for width in [8,16]:
    fn=f'#[inline(never)]\nfn lanes{width}(a:&GradientDescriptor,b:&[GradientFeature])->[f64;{width}]{{\n'
    fn+=f'if b.len()<{width}{{let mut out=[0.;{width}];for (block,chunk)in b.chunks(4).enumerate(){{let v=four(a,chunk);out[block*4..block*4+chunk.len()].copy_from_slice(&v[..chunk.len()]);}}return out;}}\n'
    for lane in range(width):fn+=f'let b{lane}=&b[{lane}].descriptor.0;\n'
    fn+='let('+','.join(f'mut s{i}'for i in range(width))+')=('+','.join('0.'for _ in range(width))+');\nfor base in(0..128).step_by(4){\n'
    for offset in range(4):
        fn+=f'{{let k=base+{offset};let x=a.0[k];\n'
        for lane in range(width):fn+=f'let d{lane}=x-b{lane}[k];s{lane}+=d{lane}*d{lane};\n'
        fn+='}\n'
    fn+='}\n['+','.join(f's{i}'for i in range(width))+']\n}\n';functions.append(fn)
source='''use std::hint::black_box;
#[derive(Clone,Copy)]struct GradientDescriptor([f64;128]);
#[derive(Clone,Copy)]struct GradientFeature{descriptor:GradientDescriptor}
'''+ '#[inline(never)]\n'+baseline+''.join(functions)+r'''
fn feature(seed:&mut u64)->GradientFeature{let mut d=[0.;128];for v in &mut d{*seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);*v=((*seed>>11)as f64)/((1u64<<53)as f64);}let norm=d.iter().map(|v|v*v).sum::<f64>().sqrt();for v in &mut d{*v/=norm;}GradientFeature{descriptor:GradientDescriptor(d)}}
fn load(path:&str)->Vec<GradientFeature>{let b=std::fs::read(path).unwrap();let n=u64::from_le_bytes(b[..8].try_into().unwrap())as usize;assert_eq!(b.len(),8+n*1040);b[8..].chunks_exact(1040).map(|b|GradientFeature{descriptor:GradientDescriptor(std::array::from_fn(|i|f64::from_le_bytes(b[16+i*8..24+i*8].try_into().unwrap())))}).collect()}
fn expected(a:&GradientDescriptor,b:&[GradientFeature],width:usize)->Vec<u64>{let mut output=vec![0;width];for (block,chunk)in b.chunks(4).enumerate(){let v=four(a,chunk);for (lane,f)in chunk.iter().enumerate(){let scalar=a.0.iter().zip(&f.descriptor.0).map(|(x,y)|(x-y)*(x-y)).sum::<f64>();assert_eq!(v[lane].to_bits(),scalar.to_bits());output[block*4+lane]=v[lane].to_bits();}}output}
'''
for width,name in [(4,'four'),(8,'lanes8'),(16,'lanes16')]:
    source+=f'fn run{width}(left:&[GradientFeature],right:&[GradientFeature])->u64{{let mut sum=0u64;for _ in 0..2{{for a in left{{for b in right.chunks({width}){{let values={name}(black_box(&a.descriptor),black_box(b));for v in values.iter().take(b.len()){{sum=sum.wrapping_add(v.to_bits());}}}}}}}}sum}}\n'
source+=r'''
fn main(){let args:Vec<_>=std::env::args().skip(1).collect();assert_eq!(args.len(),3);let left=load(&args[1]);let right=load(&args[2]);
if args[0]=="verify"{let mut seed=173;let mut comparisons=0;for _ in 0..1024{let a=feature(&mut seed);let b:[_;16]=std::array::from_fn(|_|feature(&mut seed));for n in 0..=8{assert_eq!(expected(&a.descriptor,&b[..n],8),lanes8(&a.descriptor,&b[..n]).map(f64::to_bits));comparisons+=n;}for n in 0..=16{assert_eq!(expected(&a.descriptor,&b[..n],16),lanes16(&a.descriptor,&b[..n]).map(f64::to_bits));comparisons+=n;}}
for a in &left{for b in right.chunks(16){let expected=expected(&a.descriptor,b,16);assert_eq!(expected,lanes16(&a.descriptor,b).map(f64::to_bits));let mut actual=vec![0;16];for(block,chunk)in b.chunks(8).enumerate(){actual[block*8..block*8+chunk.len()].copy_from_slice(&lanes8(&a.descriptor,chunk)[..chunk.len()].iter().map(|v|v.to_bits()).collect::<Vec<_>>());}assert_eq!(expected,actual);}}
println!("{} {}",comparisons,2*left.len()*right.len());return;}
let sum=match args[0].as_str(){"4"=>run4(&left,&right),"8"=>run8(&left,&right),"16"=>run16(&left,&right),_=>panic!("version")};println!("{}",sum);}
'''
fixture_path=D/'dedup-natural-large-gradient-banks-v2-20261014.json';fixture=json.loads(fixture_path.read_text());assert fixture['status']=='complete_pending_audit';inputs=[e['features']for e in fixture['extractions']];assert all(sha(e['features'])==e['features_sha256']for e in fixture['extractions'])
F=Path('/private/tmp/rrrah-gradient-lanes8-16-cpu-20261014');F.mkdir(exist_ok=False);src=F/'experiment.rs';src.write_text(source);exe=F/'experiment';O=D/'dedup-gradient-lanes8-16-cpu-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),pins={str(p):sha(p)for p in [Path(__file__),manifest_path,kernel,fixture_path,src]},input_pins={p:sha(p)for p in inputs},results=[],scope='Pure independent descriptor-lane batch8/16against current4, all use ordered dimensionunroll4. No numerical reordering, matcher policy or heap scratch changes in production because prototype only.1024synthetic fixtures all partial lengths versus scalar and natural2048x2048pair all-value bit parity.72fresh processes,24three-version groups,6warmup18measured balanced positions,2kernel repetitions/process. Kernel-plus-batch-loop/noinline component; no current public managed/native/cancellation/wholeoperation/default/corpus/platform adoption proof.')
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
with (F/'build.log').open('x')as log:
    p=subprocess.Popen(['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'-o',str(exe)],stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
assert code==0,(F/'build.log').read_text();state['pins'][str(exe)]=sha(exe)
with (F/'verify.stdout').open('x')as out,(F/'verify.stderr').open('x')as err:
    p=subprocess.Popen([str(exe),'verify',*inputs],stdout=out,stderr=err);state.update(status='verifying',child_pid=p.pid);save();code=p.wait()
assert code==0 and (F/'verify.stderr').read_bytes()==b'';synthetic,natural=map(int,(F/'verify.stdout').read_text().split());assert synthetic==176128 and natural==8388608;state['bitwise_controls']=dict(synthetic_distance_values=synthetic,natural_distance_values=natural);save()
canonical=None
for group in range(24):
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
    for group in range(6,24):
        cpu={r['version']:r['cpu_seconds']for r in state['results']if r['group']==group};assert cpu['4']>0;ratios.append(cpu[version]/cpu['4'])
    summary.append(dict(version=version,ratios=ratios,median_cpu_ratio=statistics.median(ratios)))
assert all(sha(p)==h for p,h in state['pins'].items())and all(sha(p)==h for p,h in state['input_pins'].items())
state.update(status='complete_pending_audit',summary=summary);save();print(summary)
