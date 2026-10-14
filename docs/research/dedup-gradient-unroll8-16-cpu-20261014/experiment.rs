use std::{hint::black_box,io};
#[derive(Clone,Copy)]struct GradientDescriptor([f64;128]);
#[derive(Clone,Copy)]struct GradientFeature{descriptor:GradientDescriptor}
#[inline(never)]
fn distances_four(a: &GradientDescriptor, b: &[GradientFeature]) -> [f64; 4] {
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
fn unroll8(a: &GradientDescriptor, b: &[GradientFeature]) -> [f64; 4] {
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
    for base in (0..128).step_by(8) {
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
        {
            let k = base + 4;
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
            let k = base + 5;
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
            let k = base + 6;
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
            let k = base + 7;
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
fn unroll16(a: &GradientDescriptor, b: &[GradientFeature]) -> [f64; 4] {
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
    for base in (0..128).step_by(16) {
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
        {
            let k = base + 4;
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
            let k = base + 5;
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
            let k = base + 6;
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
            let k = base + 7;
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
            let k = base + 8;
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
            let k = base + 9;
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
            let k = base + 10;
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
            let k = base + 11;
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
            let k = base + 12;
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
            let k = base + 13;
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
            let k = base + 14;
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
            let k = base + 15;
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

fn feature(seed:&mut u64)->GradientFeature{let mut d=[0.;128];for v in &mut d{*seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);*v=((*seed>>11)as f64)/((1u64<<53)as f64);}let norm=d.iter().map(|v|v*v).sum::<f64>().sqrt();for v in &mut d{*v/=norm;}GradientFeature{descriptor:GradientDescriptor(d)}}
fn load(path:&str)->Vec<GradientFeature>{let bytes=std::fs::read(path).unwrap();let n=u64::from_le_bytes(bytes[..8].try_into().unwrap())as usize;assert_eq!(bytes.len(),8+n*1040);bytes[8..].chunks_exact(1040).map(|b|GradientFeature{descriptor:GradientDescriptor(std::array::from_fn(|i|f64::from_le_bytes(b[16+i*8..24+i*8].try_into().unwrap())))}).collect()}
fn main()->Result<(),io::Error>{let args:Vec<_>=std::env::args().skip(1).collect();assert_eq!(args.len(),3);let left=load(&args[1]);let right=load(&args[2]);
 if args[0]=="verify"{let mut seed=173;let mut synthetic=0;for _ in 0..2048{let a=feature(&mut seed);let b:[_;4]=std::array::from_fn(|_|feature(&mut seed));for n in 0..=4{let original=distances_four(&a.descriptor,&b[..n]).map(f64::to_bits);for f in [unroll8 as fn(&GradientDescriptor,&[GradientFeature])->[f64;4],unroll16]{assert_eq!(original,f(&a.descriptor,&b[..n]).map(f64::to_bits));synthetic+=n;}for lane in 0..n{let scalar=a.descriptor.0.iter().zip(&b[lane].descriptor.0).map(|(x,y)|(x-y)*(x-y)).sum::<f64>();assert_eq!(original[lane],scalar.to_bits());}}}
 for a in &left{for block in right.chunks(4){let expected=distances_four(&a.descriptor,block).map(f64::to_bits);assert_eq!(expected,unroll8(&a.descriptor,block).map(f64::to_bits));assert_eq!(expected,unroll16(&a.descriptor,block).map(f64::to_bits));}}
 println!("{} {}",synthetic,2*left.len()*right.len());return Ok(());}
 let f:fn(&GradientDescriptor,&[GradientFeature])->[f64;4]=match args[0].as_str(){"4"=>distances_four,"8"=>unroll8,"16"=>unroll16,_=>panic!("version")};let mut checksum=0u64;
 for _ in 0..2{for a in &left{for block in right.chunks(4){for value in f(black_box(&a.descriptor),black_box(block)){checksum=checksum.wrapping_add(value.to_bits());}}}}
 println!("{}",checksum);Ok(())}
