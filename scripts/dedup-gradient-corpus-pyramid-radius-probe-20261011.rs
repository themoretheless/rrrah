//! Independent scalar descriptor distances; proposal radius calibration only.
use std::{error::Error,io::{Read,Write}};
fn main()->Result<(),Box<dyn Error>> {
 let args:Vec<_>=std::env::args().skip(1).collect();if args.len()!=3{return Err("usage: descriptors.bin file_count distances.tsv".into());}
 let files:usize=args[1].parse()?;if files>386{return Err("file cap".into());}
 let mut bytes=Vec::new();std::fs::File::open(&args[0])?.take(386*128*1040+1).read_to_end(&mut bytes)?;
 if bytes.len()%1040!=0 || bytes.len()>386*128*1040{return Err("descriptor size cap".into());}
 let mut entries=Vec::new();for (i,row) in bytes.chunks_exact(1040).enumerate(){
  let id=u64::from_le_bytes(row[..8].try_into()?);let owner=u64::from_le_bytes(row[8..16].try_into()?) as usize;
  if id!=i as u64 || owner>=files{return Err("descriptor identity".into());}
  let mut descriptor=[0.;128];for(k,word)in row[16..].chunks_exact(8).enumerate(){descriptor[k]=f64::from_le_bytes(word.try_into()?);}
  if descriptor.iter().any(|x|!x.is_finite()||*x<0.) || (descriptor.iter().map(|x|x*x).sum::<f64>()-1.).abs()>1e-6{return Err("descriptor validity".into());}
  entries.push((owner,descriptor));
 }
 let mut minima=vec![f64::INFINITY;files*files];let mut comparisons=0u64;
 for i in 0..entries.len(){let (a,da)=&entries[i];for (b,db) in &entries[i+1..]{if a==b{continue;}
  let mut d=0.;for k in 0..128{let delta=da[k]-db[k];d+=delta*delta;}
  let key=a.min(b)*files+a.max(b);minima[key]=minima[key].min(d);comparisons+=1;
 }}
 let mut out=std::io::BufWriter::new(std::fs::File::create(&args[2])?);
 for a in 0..files{for b in a+1..files{writeln!(out,"{a}\t{b}\t{}",minima[a*files+b])?;}}
 out.flush()?;println!("files={files} descriptors={} comparisons={comparisons}",entries.len());Ok(())
}
