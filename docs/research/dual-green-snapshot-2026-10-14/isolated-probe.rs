use std::time::Instant;
fn reflect(i: isize,n: usize)->usize {if n==1{return 0;} let p=2*(n as isize-1);let j=i.rem_euclid(p);if j>=n as isize {(p-j) as usize}else{j as usize}}
fn legacy(mut out:Vec<[f64;3]>,smooth:&[[f64;3]],source:&[f64],w:usize,h:usize,phase:usize,threshold:f64)->Vec<[f64;3]>{
 if w==0||h==0{return out;}let copy=out.clone();let threshold=threshold.max(1e-4);
 for y in 0..h {for x in 0..w {let i=y*w+x;let(mut lo,mut hi,mut sum)=(f64::INFINITY,f64::NEG_INFINITY,0.);for dy in -1..=1 {for dx in -1..=1 {let g=copy[reflect(y as isize+dy,h)*w+reflect(x as isize+dx,w)][1];lo=lo.min(g);hi=hi.max(g);sum+=g;}}
 let mean=(sum/9.).abs().max(1e-3);let contrast=(hi-lo)/mean;let t=(contrast/(2.*threshold)).clamp(0.,1.);let alpha=t*t*(3.-2.*t);let a=copy[i];let b=smooth[i];let mut v=[b[0]+alpha*(a[0]-b[0]),b[1]+alpha*(a[1]-b[1]),b[2]+alpha*(a[2]-b[2])];let c=[[0,1],[1,2]][(y+phase/2)%2][(x+phase%2)%2];v[c]=source[i];out[i]=v;}}
 out
}
fn green(mut out:Vec<[f64;3]>,smooth:&[[f64;3]],source:&[f64],w:usize,h:usize,phase:usize,threshold:f64)->Vec<[f64;3]>{
 if w==0||h==0{return out;}let copy:Vec<f64>=out.iter().map(|p|p[1]).collect();let threshold=threshold.max(1e-4);
 for y in 0..h {for x in 0..w {let i=y*w+x;let(mut lo,mut hi,mut sum)=(f64::INFINITY,f64::NEG_INFINITY,0.);for dy in -1..=1 {for dx in -1..=1 {let g=copy[reflect(y as isize+dy,h)*w+reflect(x as isize+dx,w)];lo=lo.min(g);hi=hi.max(g);sum+=g;}}
 let mean=(sum/9.).abs().max(1e-3);let contrast=(hi-lo)/mean;let t=(contrast/(2.*threshold)).clamp(0.,1.);let alpha=t*t*(3.-2.*t);let a=out[i];let b=smooth[i];let mut v=[b[0]+alpha*(a[0]-b[0]),b[1]+alpha*(a[1]-b[1]),b[2]+alpha*(a[2]-b[2])];let c=[[0,1],[1,2]][(y+phase/2)%2][(x+phase%2)%2];v[c]=source[i];out[i]=v;}}
 out
}
fn inputs(w:usize,h:usize)->(Vec<[f64;3]>,Vec<[f64;3]>,Vec<f64>){let a:Vec<_>=(0..w*h).map(|i|{let v=((i*73+i/17)%1021)as f64/127.;[v+0.13,v*0.37-0.5,v*1.7]}).collect();let b=a.iter().map(|p|[p[2]*0.7,p[0]*0.2,p[1]*0.8]).collect();let s=a.iter().map(|p|p[0]).collect();(a,b,s)}
fn main(){let mut cases=0;for(w,h)in[(0,0),(1,1),(1,7),(9,1),(2,2),(17,19),(139,133)]{let(a,b,s)=inputs(w,h);for phase in 0..4{for threshold in[0.,0.04,0.8]{let x=legacy(a.clone(),&b,&s,w,h,phase,threshold);let y=green(a.clone(),&b,&s,w,h,phase,threshold);assert!(x.iter().flatten().zip(y.iter().flatten()).all(|(x,y)|x.to_bits()==y.to_bits()));cases+=1;}}}println!("exact_cases={cases}");
 let(w,h)=(1024,1024);let(a,b,s)=inputs(w,h);for trial in 0..8{let order=if trial%2==0{[false,true]}else{[true,false]};for candidate in order{let input=a.clone();let start=Instant::now();let result=if candidate{green(input,&b,&s,w,h,0,0.04)}else{legacy(input,&b,&s,w,h,0,0.04)};std::hint::black_box(&result);println!("trial={trial} variant={} elapsed_ns={}",if candidate{"green"}else{"legacy"},start.elapsed().as_nanos());}}
 println!("scratch_bytes_1048576 legacy={} green={}",w*h*24,w*h*8);
}
