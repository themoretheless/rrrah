use blockhash::{Image,blockhash16,blockhash64,blockhash144,blockhash256};
struct Grid{w:u32,h:u32}
impl Image for Grid{const MAX_BRIGHTNESS:u32=255;fn dimensions(&self)->(u32,u32){(self.w,self.h)}fn brightness(&self,x:u32,y:u32)->u32{(x*37+y*19+x*y*11+13)%256}}
fn oracle(g:&Grid,n:u32)->Vec<u8>{
 let mut v=vec![0u64;(n*n)as usize];
 for by in 0..n{for bx in 0..n{for y in 0..g.h{for x in 0..g.w{
 let dx=((bx+1)*g.w).min((x+1)*n).saturating_sub((bx*g.w).max(x*n));
 let dy=((by+1)*g.h).min((y+1)*n).saturating_sub((by*g.h).max(y*n));
 v[(by*n+bx)as usize]+=u64::from(dx)*u64::from(dy)*u64::from(g.brightness(x,y));
 }}}}
 let mut out=vec![0u8;(n*n/8)as usize];let band=(n*n/4)as usize;
 for b in 0..4{let mut s=v[b*band..(b+1)*band].to_vec();s.sort();let median=(s[band/2-1]+s[band/2])/2;
 for i in b*band..(b+1)*band{if v[i]>median||(v[i]==median&&v[i]>255*u64::from(g.w)*u64::from(g.h)/2){out[i/8]|=1<<(7-i%8);}}}
 out
}
fn main(){let mut cases=0;for w in 1..=19{for h in 1..=19{let g=Grid{w,h};let actual=[<[u8;2]>::from(blockhash16(&g)).to_vec(),<[u8;8]>::from(blockhash64(&g)).to_vec(),<[u8;18]>::from(blockhash144(&g)).to_vec(),<[u8;32]>::from(blockhash256(&g)).to_vec()];for(i,n)in[4,8,12,16].into_iter().enumerate(){assert_eq!(actual[i],oracle(&g,n),"{w}x{h} grid{n}");cases+=1;}}}println!("independent_area_cases={cases}; all matched");}
