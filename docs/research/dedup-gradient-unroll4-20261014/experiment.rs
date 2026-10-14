#![allow(dead_code)]
#[derive(Clone,Copy)] struct GradientDescriptor([f64;128]);
#[derive(Clone,Copy)] struct GradientFeature { position:[f64;2], descriptor:GradientDescriptor }
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
    for k in 0..128 {
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
    [s0, s1, s2, s3]
}
#[inline(never)]
fn candidate(a: &GradientDescriptor, b: &[GradientFeature]) -> [f64; 4] {
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
        { let k = base + 0;
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
        { let k = base + 1;
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
        { let k = base + 2;
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
        { let k = base + 3;
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

fn next(seed:&mut u64)->f64 { *seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1); ((*seed>>11) as f64)/((1u64<<53) as f64) }
fn feature(seed:&mut u64)->GradientFeature {
    let mut values=std::array::from_fn(|_| next(seed));
    let norm=values.iter().map(|v|v*v).sum::<f64>().sqrt();
    for value in &mut values { *value/=norm; }
    GradientFeature {position:[0.;2],descriptor:GradientDescriptor(values)}
}
fn run(left:&[GradientFeature],right:&[GradientFeature],f:fn(&GradientDescriptor,&[GradientFeature])->[f64;4])->(u128,u64) {
    let start=std::time::Instant::now();let mut checksum=0u64;
    for a in left {for chunk in right.chunks(4) {for value in f(std::hint::black_box(&a.descriptor),std::hint::black_box(chunk)) {checksum=checksum.wrapping_add(value.to_bits());}}}
    (start.elapsed().as_nanos(),checksum)
}
fn main() {
    let mut seed=173;let left:Vec<_>=(0..256).map(|_|feature(&mut seed)).collect();
    let right:Vec<_>=(0..1024).map(|_|feature(&mut seed)).collect();let mut checked=0;
    for a in &left {for length in 0..=4 {for offset in (0..1020).step_by(17) {
        let chunk=&right[offset..offset+length];
        assert_eq!(distances_four(&a.descriptor,chunk).map(f64::to_bits),candidate(&a.descriptor,chunk).map(f64::to_bits));checked+=1;
    }}}
    println!("checked {}",checked);
    for pair in 0..10 {
        let order=if pair%2==0 {[false,true]} else {[true,false]};
        for changed in order {let (n,sum)=run(&left,&right,if changed {candidate}else{distances_four});println!("{} {} {} {}",pair,changed,n,sum);}
    }
}
