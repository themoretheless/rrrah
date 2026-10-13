"""Isolated exact eight-accumulator experiment; never edits production source."""
import hashlib,json,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';OUT=D/'dedup-gradient-eight-kernel-20261011.json';assert not OUT.exists()
work=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-eight-kernel-20261011');work.mkdir(exist_ok=False)
source=ROOT/'crates/rrrah-dedup/src/gradient.rs';text=source.read_text();start=text.index('fn distances_four(');opening=text.index('{',start);depth=1;end=opening+1
while depth:
 if text[end]=='{':depth+=1
 if text[end]=='}':depth-=1
 end+=1
four=text[start:end]
eight='fn distances_eight(a: &GradientDescriptor, b: &[GradientFeature]) -> [f64; 8] {\n if b.len()<8 { let mut out=[0.;8]; for (n,c) in b.chunks(4).enumerate() {let v=distances_four(a,c); out[n*4..n*4+c.len()].copy_from_slice(&v[..c.len()]);} return out; }\n'
eight+='\n'.join(f'let b{i}=&b[{i}].descriptor.0; let mut s{i}=0.;' for i in range(8))
eight+='\nfor k in 0..128 {let x=a.0[k];\n'+'\n'.join(f'let d{i}=x-b{i}[k];s{i}+=d{i}*d{i};' for i in range(8))+'\n}\n['+','.join(f's{i}' for i in range(8))+']\n}\n'
harness=r'''
#[derive(Clone)] struct GradientDescriptor([f64;128]);
#[derive(Clone)] struct GradientFeature { position:[f64;2],descriptor:GradientDescriptor }
fn bank(n:usize,seed:u64)->Vec<GradientFeature> {
 let mut state=seed; (0..n).map(|i| {let mut a=[0.;128];for x in &mut a {state^=state<<13;state^=state>>7;state^=state<<17;*x=(state%10000) as f64/10000.;} if i%7==0 {a[0]=1e-300;} let norm=a.iter().map(|x|x*x).sum::<f64>().sqrt(); for x in &mut a {*x/=norm;} GradientFeature{position:[i as f64,0.],descriptor:GradientDescriptor(a)}}).collect()
}
fn run(a:&[GradientFeature],b:&[GradientFeature],eight:bool)->f64 {
 let mut sum=0.;for x in a {if eight {for c in b.chunks(8) {let v=distances_eight(std::hint::black_box(&x.descriptor),std::hint::black_box(c));for d in &v[..c.len()] {sum+=std::hint::black_box(*d);}}} else {for c in b.chunks(4) {let v=distances_four(std::hint::black_box(&x.descriptor),std::hint::black_box(c));for d in &v[..c.len()] {sum+=std::hint::black_box(*d);}}}} sum
}
fn main(){
 let a=bank(17,173);let b=bank(1031,319);
 for n in [1,2,3,4,5,6,7,8,9,31,65,259,1031] {for x in &a {for c in b[..n].chunks(8) {let v=distances_eight(&x.descriptor,c);for (i,y) in c.iter().enumerate() {let scalar=x.descriptor.0.iter().zip(&y.descriptor.0).map(|(x,y)|(x-y)*(x-y)).sum::<f64>();assert_eq!(v[i].to_bits(),scalar.to_bits());}}}}
 println!("parity,17527");
 for (n,m) in [(64,65),(257,259),(1024,1025)] {let a=bank(n,173);let b=bank(m,319);let expected=run(&a,&b,false);assert_eq!(expected.to_bits(),run(&a,&b,true).to_bits());for (trial,eight) in [false,true,true,false,false,true,true,false].into_iter().enumerate(){let t=std::time::Instant::now();let sum=run(&a,&b,eight);assert_eq!(sum.to_bits(),expected.to_bits());println!("sample,{n},{m},{trial},{eight},{:.9},{:x}",t.elapsed().as_secs_f64(),sum.to_bits());}}
}
'''
rs=work/'kernel.rs';rs.write_text(harness.replace('fn main(){',four+'\n'+eight+'\nfn main(){'))
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(source):sha(source),str(rs):sha(rs),str(Path(__file__)):sha(__file__)}
rustc=Path(subprocess.check_output(['rustup','which','rustc'],text=True).strip());pins[str(rustc)]=sha(rustc)
report={'status':'running_isolated_microkernel_experiment','scope':'Copied actual four-distance helper versus explicit eight-accumulator candidate. No production changes, no matcher/cancellation/geometry/runtime qualification. Seeded normalized synthetic banks only.','pins':pins,'runs':[]}
OUT.write_text(json.dumps(report,indent=2)+'\n')
for level in [1,3]:
 binary=work/f'kernel-opt{level}';build=subprocess.run([str(rustc),'--edition=2024','-C',f'opt-level={level}',str(rs),'-o',str(binary)],text=True,capture_output=True);(work/f'build-opt{level}.log').write_text(build.stdout+build.stderr);assert build.returncode==0,build.stderr
 result=subprocess.run([str(binary)],text=True,capture_output=True);(work/f'run-opt{level}.log').write_text(result.stdout+result.stderr);assert result.returncode==0,result.stderr
 samples=[]
 for line in result.stdout.splitlines():
  if line.startswith('sample,'):
   _,n,m,trial,eight,seconds,checksum=line.split(',');samples.append({'left':int(n),'right':int(m),'trial':int(trial),'eight':eight=='true','seconds':float(seconds),'checksum_bits':checksum})
 assert len(samples)==24
 report['runs'].append({'opt_level':level,'samples':samples,'binary':str(binary),'binary_sha256':sha(binary),'exact_distance_and_checksum_assertions_passed':True});OUT.write_text(json.dumps(report,indent=2)+'\n')
assert all(sha(p)==h for p,h in pins.items());report['status']='complete_isolated_microkernel_experiment_not_adopted';OUT.write_text(json.dumps(report,indent=2)+'\n');print(report['status'])
