use std::hint::black_box;
#[derive(Clone,Copy)]struct GradientDescriptor([f64;128]);
#[derive(Clone,Copy)]struct GradientFeature{descriptor:GradientDescriptor}
#[inline(never)]
fn four(a: &GradientDescriptor, b: &[GradientFeature]) -> [f64; 4] {
    // No repeated/phantom comparisons in partial blocks.
    if b.len() < 4 {
        let mut output = [0.; 4];
        for (lane, b) in b.iter().enumerate() {
            output[lane] =
                a.0.iter()
                    .zip(&b.descriptor.0)
                    .map(|(x, y)| (x - y) * (x - y))
                    .sum();
        }
        return output;
    }
    let last = 3;
    let b0 = &b[0].descriptor.0;
    let b1 = &b[1.min(last)].descriptor.0;
    let b2 = &b[2.min(last)].descriptor.0;
    let b3 = &b[3.min(last)].descriptor.0;
    let (mut s0, mut s1, mut s2, mut s3) = (0., 0., 0., 0.);
    for base in (0..128).step_by(4) {
        {
            let k = base + 0;
            let x = a.0[k];
            let d0 = x - b0[k];
            s0 += d0 * d0;
            let d1 = x - b1[k];
            s1 += d1 * d1;
            let d2 = x - b2[k];
            s2 += d2 * d2;
            let d3 = x - b3[k];
            s3 += d3 * d3;
        }
        {
            let k = base + 1;
            let x = a.0[k];
            let d0 = x - b0[k];
            s0 += d0 * d0;
            let d1 = x - b1[k];
            s1 += d1 * d1;
            let d2 = x - b2[k];
            s2 += d2 * d2;
            let d3 = x - b3[k];
            s3 += d3 * d3;
        }
        {
            let k = base + 2;
            let x = a.0[k];
            let d0 = x - b0[k];
            s0 += d0 * d0;
            let d1 = x - b1[k];
            s1 += d1 * d1;
            let d2 = x - b2[k];
            s2 += d2 * d2;
            let d3 = x - b3[k];
            s3 += d3 * d3;
        }
        {
            let k = base + 3;
            let x = a.0[k];
            let d0 = x - b0[k];
            s0 += d0 * d0;
            let d1 = x - b1[k];
            s1 += d1 * d1;
            let d2 = x - b2[k];
            s2 += d2 * d2;
            let d3 = x - b3[k];
            s3 += d3 * d3;
        }
    }
    [s0, s1, s2, s3]
}
#[inline(never)]
fn lanes8(a:&GradientDescriptor,b:&[GradientFeature])->[f64;8]{
if b.len()<8{let mut out=[0.;8];for (block,chunk)in b.chunks(4).enumerate(){let v=four(a,chunk);out[block*4..block*4+chunk.len()].copy_from_slice(&v[..chunk.len()]);}return out;}
let b0=&b[0].descriptor.0;
let b1=&b[1].descriptor.0;
let b2=&b[2].descriptor.0;
let b3=&b[3].descriptor.0;
let b4=&b[4].descriptor.0;
let b5=&b[5].descriptor.0;
let b6=&b[6].descriptor.0;
let b7=&b[7].descriptor.0;
let(mut s0,mut s1,mut s2,mut s3,mut s4,mut s5,mut s6,mut s7)=(0.,0.,0.,0.,0.,0.,0.,0.);
for base in(0..128).step_by(4){
{let k=base+0;let x=a.0[k];
let d0=x-b0[k];s0+=d0*d0;
let d1=x-b1[k];s1+=d1*d1;
let d2=x-b2[k];s2+=d2*d2;
let d3=x-b3[k];s3+=d3*d3;
let d4=x-b4[k];s4+=d4*d4;
let d5=x-b5[k];s5+=d5*d5;
let d6=x-b6[k];s6+=d6*d6;
let d7=x-b7[k];s7+=d7*d7;
}
{let k=base+1;let x=a.0[k];
let d0=x-b0[k];s0+=d0*d0;
let d1=x-b1[k];s1+=d1*d1;
let d2=x-b2[k];s2+=d2*d2;
let d3=x-b3[k];s3+=d3*d3;
let d4=x-b4[k];s4+=d4*d4;
let d5=x-b5[k];s5+=d5*d5;
let d6=x-b6[k];s6+=d6*d6;
let d7=x-b7[k];s7+=d7*d7;
}
{let k=base+2;let x=a.0[k];
let d0=x-b0[k];s0+=d0*d0;
let d1=x-b1[k];s1+=d1*d1;
let d2=x-b2[k];s2+=d2*d2;
let d3=x-b3[k];s3+=d3*d3;
let d4=x-b4[k];s4+=d4*d4;
let d5=x-b5[k];s5+=d5*d5;
let d6=x-b6[k];s6+=d6*d6;
let d7=x-b7[k];s7+=d7*d7;
}
{let k=base+3;let x=a.0[k];
let d0=x-b0[k];s0+=d0*d0;
let d1=x-b1[k];s1+=d1*d1;
let d2=x-b2[k];s2+=d2*d2;
let d3=x-b3[k];s3+=d3*d3;
let d4=x-b4[k];s4+=d4*d4;
let d5=x-b5[k];s5+=d5*d5;
let d6=x-b6[k];s6+=d6*d6;
let d7=x-b7[k];s7+=d7*d7;
}
}
[s0,s1,s2,s3,s4,s5,s6,s7]
}
#[inline(never)]
fn lanes16(a:&GradientDescriptor,b:&[GradientFeature])->[f64;16]{
if b.len()<16{let mut out=[0.;16];for (block,chunk)in b.chunks(4).enumerate(){let v=four(a,chunk);out[block*4..block*4+chunk.len()].copy_from_slice(&v[..chunk.len()]);}return out;}
let b0=&b[0].descriptor.0;
let b1=&b[1].descriptor.0;
let b2=&b[2].descriptor.0;
let b3=&b[3].descriptor.0;
let b4=&b[4].descriptor.0;
let b5=&b[5].descriptor.0;
let b6=&b[6].descriptor.0;
let b7=&b[7].descriptor.0;
let b8=&b[8].descriptor.0;
let b9=&b[9].descriptor.0;
let b10=&b[10].descriptor.0;
let b11=&b[11].descriptor.0;
let b12=&b[12].descriptor.0;
let b13=&b[13].descriptor.0;
let b14=&b[14].descriptor.0;
let b15=&b[15].descriptor.0;
let(mut s0,mut s1,mut s2,mut s3,mut s4,mut s5,mut s6,mut s7,mut s8,mut s9,mut s10,mut s11,mut s12,mut s13,mut s14,mut s15)=(0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.,0.);
for base in(0..128).step_by(4){
{let k=base+0;let x=a.0[k];
let d0=x-b0[k];s0+=d0*d0;
let d1=x-b1[k];s1+=d1*d1;
let d2=x-b2[k];s2+=d2*d2;
let d3=x-b3[k];s3+=d3*d3;
let d4=x-b4[k];s4+=d4*d4;
let d5=x-b5[k];s5+=d5*d5;
let d6=x-b6[k];s6+=d6*d6;
let d7=x-b7[k];s7+=d7*d7;
let d8=x-b8[k];s8+=d8*d8;
let d9=x-b9[k];s9+=d9*d9;
let d10=x-b10[k];s10+=d10*d10;
let d11=x-b11[k];s11+=d11*d11;
let d12=x-b12[k];s12+=d12*d12;
let d13=x-b13[k];s13+=d13*d13;
let d14=x-b14[k];s14+=d14*d14;
let d15=x-b15[k];s15+=d15*d15;
}
{let k=base+1;let x=a.0[k];
let d0=x-b0[k];s0+=d0*d0;
let d1=x-b1[k];s1+=d1*d1;
let d2=x-b2[k];s2+=d2*d2;
let d3=x-b3[k];s3+=d3*d3;
let d4=x-b4[k];s4+=d4*d4;
let d5=x-b5[k];s5+=d5*d5;
let d6=x-b6[k];s6+=d6*d6;
let d7=x-b7[k];s7+=d7*d7;
let d8=x-b8[k];s8+=d8*d8;
let d9=x-b9[k];s9+=d9*d9;
let d10=x-b10[k];s10+=d10*d10;
let d11=x-b11[k];s11+=d11*d11;
let d12=x-b12[k];s12+=d12*d12;
let d13=x-b13[k];s13+=d13*d13;
let d14=x-b14[k];s14+=d14*d14;
let d15=x-b15[k];s15+=d15*d15;
}
{let k=base+2;let x=a.0[k];
let d0=x-b0[k];s0+=d0*d0;
let d1=x-b1[k];s1+=d1*d1;
let d2=x-b2[k];s2+=d2*d2;
let d3=x-b3[k];s3+=d3*d3;
let d4=x-b4[k];s4+=d4*d4;
let d5=x-b5[k];s5+=d5*d5;
let d6=x-b6[k];s6+=d6*d6;
let d7=x-b7[k];s7+=d7*d7;
let d8=x-b8[k];s8+=d8*d8;
let d9=x-b9[k];s9+=d9*d9;
let d10=x-b10[k];s10+=d10*d10;
let d11=x-b11[k];s11+=d11*d11;
let d12=x-b12[k];s12+=d12*d12;
let d13=x-b13[k];s13+=d13*d13;
let d14=x-b14[k];s14+=d14*d14;
let d15=x-b15[k];s15+=d15*d15;
}
{let k=base+3;let x=a.0[k];
let d0=x-b0[k];s0+=d0*d0;
let d1=x-b1[k];s1+=d1*d1;
let d2=x-b2[k];s2+=d2*d2;
let d3=x-b3[k];s3+=d3*d3;
let d4=x-b4[k];s4+=d4*d4;
let d5=x-b5[k];s5+=d5*d5;
let d6=x-b6[k];s6+=d6*d6;
let d7=x-b7[k];s7+=d7*d7;
let d8=x-b8[k];s8+=d8*d8;
let d9=x-b9[k];s9+=d9*d9;
let d10=x-b10[k];s10+=d10*d10;
let d11=x-b11[k];s11+=d11*d11;
let d12=x-b12[k];s12+=d12*d12;
let d13=x-b13[k];s13+=d13*d13;
let d14=x-b14[k];s14+=d14*d14;
let d15=x-b15[k];s15+=d15*d15;
}
}
[s0,s1,s2,s3,s4,s5,s6,s7,s8,s9,s10,s11,s12,s13,s14,s15]
}

fn feature(seed:&mut u64)->GradientFeature{let mut d=[0.;128];for v in &mut d{*seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);*v=((*seed>>11)as f64)/((1u64<<53)as f64);}let norm=d.iter().map(|v|v*v).sum::<f64>().sqrt();for v in &mut d{*v/=norm;}GradientFeature{descriptor:GradientDescriptor(d)}}
fn load(path:&str)->Vec<GradientFeature>{let b=std::fs::read(path).unwrap();let n=u64::from_le_bytes(b[..8].try_into().unwrap())as usize;assert_eq!(b.len(),8+n*1040);b[8..].chunks_exact(1040).map(|b|GradientFeature{descriptor:GradientDescriptor(std::array::from_fn(|i|f64::from_le_bytes(b[16+i*8..24+i*8].try_into().unwrap())))}).collect()}
fn expected(a:&GradientDescriptor,b:&[GradientFeature],width:usize)->Vec<u64>{let mut output=vec![0;width];for (block,chunk)in b.chunks(4).enumerate(){let v=four(a,chunk);for (lane,f)in chunk.iter().enumerate(){let scalar=a.0.iter().zip(&f.descriptor.0).map(|(x,y)|(x-y)*(x-y)).sum::<f64>();assert_eq!(v[lane].to_bits(),scalar.to_bits());output[block*4+lane]=v[lane].to_bits();}}output}
fn run4(left:&[GradientFeature],right:&[GradientFeature])->u64{let mut sum=0u64;for _ in 0..2{for a in left{for b in right.chunks(4){let values=four(black_box(&a.descriptor),black_box(b));for v in values.iter().take(b.len()){sum=sum.wrapping_add(v.to_bits());}}}}sum}
fn run8(left:&[GradientFeature],right:&[GradientFeature])->u64{let mut sum=0u64;for _ in 0..2{for a in left{for b in right.chunks(8){let values=lanes8(black_box(&a.descriptor),black_box(b));for v in values.iter().take(b.len()){sum=sum.wrapping_add(v.to_bits());}}}}sum}
fn run16(left:&[GradientFeature],right:&[GradientFeature])->u64{let mut sum=0u64;for _ in 0..2{for a in left{for b in right.chunks(16){let values=lanes16(black_box(&a.descriptor),black_box(b));for v in values.iter().take(b.len()){sum=sum.wrapping_add(v.to_bits());}}}}sum}

fn main(){let args:Vec<_>=std::env::args().skip(1).collect();assert_eq!(args.len(),3);let left=load(&args[1]);let right=load(&args[2]);
if args[0]=="verify"{let mut seed=173;let mut comparisons=0;for _ in 0..1024{let a=feature(&mut seed);let b:[_;16]=std::array::from_fn(|_|feature(&mut seed));for n in 0..=8{assert_eq!(expected(&a.descriptor,&b[..n],8),lanes8(&a.descriptor,&b[..n]).map(f64::to_bits));comparisons+=n;}for n in 0..=16{assert_eq!(expected(&a.descriptor,&b[..n],16),lanes16(&a.descriptor,&b[..n]).map(f64::to_bits));comparisons+=n;}}
for a in &left{for b in right.chunks(16){let expected=expected(&a.descriptor,b,16);assert_eq!(expected,lanes16(&a.descriptor,b).map(f64::to_bits));let mut actual=vec![0;16];for(block,chunk)in b.chunks(8).enumerate(){actual[block*8..block*8+chunk.len()].copy_from_slice(&lanes8(&a.descriptor,chunk)[..chunk.len()].iter().map(|v|v.to_bits()).collect::<Vec<_>>());}assert_eq!(expected,actual);}}
println!("{} {}",comparisons,2*left.len()*right.len());return;}
let sum=match args[0].as_str(){"4"=>run4(&left,&right),"8"=>run8(&left,&right),"16"=>run16(&left,&right),_=>panic!("version")};println!("{}",sum);}
