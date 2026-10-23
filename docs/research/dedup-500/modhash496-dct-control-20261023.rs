use std::f64::consts::{FRAC_1_SQRT_2,PI};
const DCT:usize=32;
fn dct2_2d(block: &mut [f64; DCT * DCT]) {
    let n = DCT;
    let scale = (2.0 / n as f64).sqrt();
    let kernel = |x: &[f64], out: &mut [f64]| {
        for (k, slot) in out.iter_mut().enumerate() {
            let mut acc = 0.0;
            for (i, &xi) in x.iter().enumerate() {
                acc += xi * (PI * ((2 * i + 1) * k) as f64 / (2.0 * n as f64)).cos();
            }
            let c_k = if k == 0 { FRAC_1_SQRT_2 } else { 1.0 };
            *slot = c_k * scale * acc;
        }
    };
    let mut row_out = [0.0f64; DCT];
    for row in block.chunks_exact_mut(DCT) {
        kernel(row, &mut row_out);
        row.copy_from_slice(&row_out);
    }
    let mut gather = [0.0f64; DCT];
    let mut col_out = [0.0f64; DCT];
    for x in 0..DCT {
        for (y, g) in gather.iter_mut().enumerate() {
            *g = block[y * DCT + x];
        }
        kernel(&gather, &mut col_out);
        for (y, g) in col_out.iter().enumerate() {
            block[y * DCT + x] = *g;
        }
    }
}
fn median63(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[(v.len() - 1) / 2]
}
fn main(){
 let mut maximum=0f64;let mut checks=0;
 for case in 0..4 {
  let mut input=[0f64;1024];
  for i in 0..1024 {input[i]=match case{0=>0.,1=>128.,2=>if i==137{255.}else{0.},_=>((i*73+i/32*19)%256) as f64};}
  let mut actual=input;dct2_2d(&mut actual);
  for v in 0..8 {for u in 0..8 {
   let mut sum=0.;for y in 0..32 {for x in 0..32 {sum+=input[y*32+x]*(PI*((2*x+1)*u) as f64/64.).cos()*(PI*((2*y+1)*v) as f64/64.).cos();}}
   let expected=sum*(if u==0{FRAC_1_SQRT_2}else{1.})*(if v==0{FRAC_1_SQRT_2}else{1.})/16.;
   let error=(expected-actual[v*32+u]).abs();maximum=maximum.max(error);assert!(error<1e-8);checks+=1;
  }}
  if case==1 {let mut rest=Vec::new();for y in 0..8{for x in 0..8{if x!=0||y!=0{rest.push(actual[y*32+x]);}}}let median=median63(&mut rest);let mut hash=0u64;for y in 0..8{for x in 0..8{if actual[y*32+x]>median{hash|=1u64<<(63-(8*y+x));}}}println!("flat128 low-block hash={hash:016x}, set_bits={}",hash.count_ones());}
 }
 println!("{checks} low-block coefficients checked; maximum absolute error={maximum:e}");
}
