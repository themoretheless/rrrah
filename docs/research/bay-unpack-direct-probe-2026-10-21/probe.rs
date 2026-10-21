use std::{hint::black_box,time::Instant};
#[inline(never)]
fn decode<const DIRECT:bool>(input:&[u8], output:&mut[u16]) {
 for (row,pixels) in input.chunks_exact(3232).zip(output.chunks_exact_mut(2585)) {
  for (g,p) in row[..3231].chunks_exact(5).zip(pixels.chunks_exact_mut(4)) {
   if DIRECT { p[0]=(u16::from(g[0])<<2)|(u16::from(g[1])>>6);p[1]=((u16::from(g[1])&63)<<4)|(u16::from(g[2])>>4);p[2]=((u16::from(g[2])&15)<<6)|(u16::from(g[3])>>2);p[3]=((u16::from(g[3])&3)<<8)|u16::from(g[4]); }
   else {let packed=u64::from_be_bytes([0,0,0,g[0],g[1],g[2],g[3],g[4]]);for (i,s) in p.iter_mut().enumerate(){*s=((packed>>(30-i*10))&1023)as u16;}}
  }
  pixels[2584]=u16::from_be_bytes([row[3230],row[3231]])>>6;
 }
}
fn time(input:&[u8],output:&mut[u16],direct:bool)->u128 {let t=Instant::now();for _ in 0..12 {if direct {decode::<true>(black_box(input),black_box(output));} else {decode::<false>(black_box(input),black_box(output));}}black_box(&output);t.elapsed().as_nanos()}
fn main(){let input:Vec<_>=(0..3232*1924).map(|i|((i as u64*1664525+1013904223)^(i as u64>>3))as u8).collect();let mut a=vec![0;2585*1924];let mut b=a.clone();decode::<false>(&input,&mut a);decode::<true>(&input,&mut b);assert_eq!(a,b);println!("exact_samples={}",a.len());for _ in 0..12 {time(&input,&mut a,false);time(&input,&mut a,true);}for i in 0..6 {let x=time(&input,&mut a,false);let y=time(&input,&mut a,false);println!("AA {i} {x} {y}");}for i in 0..16 {let(x,y)=if i%2==0{let x=time(&input,&mut a,false);let y=time(&input,&mut a,true);(x,y)}else{let y=time(&input,&mut a,true);let x=time(&input,&mut a,false);(x,y)};println!("AB {i} {x} {y}");}}
