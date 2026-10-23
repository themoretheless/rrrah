// Isolated original distance function; Digest storage adapter only, not full package.
mod digest {
 pub struct Digest<const N: usize>(pub [u8;N]);
 impl<const N:usize> Digest<N> { pub fn as_bytes(&self)->&[u8;N] { &self.0 } }
}
#[path="evidence/n24q02m--modhash/04-distance.rs"] mod original;
fn check<const N:usize>() {
 let mut seed=0x123456789abcdef0_u64;
 for case in 0..128 {
  let mut a=[0u8;N];let mut b=[0u8;N];
  for i in 0..N {
   seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;a[i]=seed as u8;
   seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;b[i]=seed as u8;
  }
  if case==0 { b=a; }
  if case==1 { a.fill(0);b.fill(255); }
  let expected=(0..N).map(|i|(0..8).filter(|bit|((a[i]>>bit)&1)!=((b[i]>>bit)&1)).count() as u32).sum::<u32>();
  let aa=digest::Digest(a);let bb=digest::Digest(b);
  assert_eq!(original::hamming(&aa,&bb),expected,"N={N}, case={case}");
  assert_eq!(original::hamming(&bb,&aa),expected);
 }
}
fn main(){
check::<0>();
check::<1>();
check::<2>();
check::<3>();
check::<4>();
check::<5>();
check::<6>();
check::<7>();
check::<8>();
check::<9>();
check::<10>();
check::<11>();
check::<12>();
check::<13>();
check::<14>();
check::<15>();
check::<16>();
check::<17>();
check::<18>();
check::<19>();
check::<20>();
check::<21>();
check::<22>();
check::<23>();
check::<24>();
check::<25>();
check::<26>();
check::<27>();
check::<28>();
check::<29>();
check::<30>();
check::<31>();
check::<32>();
check::<33>();
check::<34>();
check::<35>();
check::<36>();
check::<37>();
check::<38>();
check::<39>();
check::<40>();
check::<41>();
check::<42>();
check::<43>();
check::<44>();
check::<45>();
check::<46>();
check::<47>();
check::<48>();
check::<49>();
check::<50>();
check::<51>();
check::<52>();
check::<53>();
check::<54>();
check::<55>();
check::<56>();
check::<57>();
check::<58>();
check::<59>();
check::<60>();
check::<61>();
check::<62>();
check::<63>();
check::<64>();
check::<65>();
println!("8448 independent bit-loop comparisons plus symmetry passed for digest lengths0..65");}
