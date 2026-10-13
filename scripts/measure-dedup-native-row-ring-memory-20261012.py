#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-native-row-ring-snapshot-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items());parent=json.loads(Path(m['parent_manifest']).read_text());old=Path(parent['snapshot'])/'crates/rrrah-dedup/src/gradient_scale.rs';assert sha(old)==parent['file_hashes']['crates/rrrah-dedup/src/gradient_scale.rs'];baseline=old.read_text();baseline=baseline[baseline.index('pub fn smooth_gradient_candidates_managed('):].replace('pub fn smooth_gradient_candidates_managed(', 'fn original_full_frame(')
 h=T/'rrrah-native-row-ring-memory-harness-20261012';out=D/'dedup-native-row-ring-memory-20261012.json';assert not h.exists()and not out.exists();(h/'src').mkdir(parents=True)
 cargo=f'''[package]
name="native-row-ring-memory-probe"
version="0.0.0"
edition="2024"
[workspace]
[dependencies]
rrrah-dedup={{path="{s}/crates/rrrah-dedup",features=["raster"]}}
rrrah-core={{path="{s}/crates/rrrah-core"}}
serde_json="1"
blake3="1.8"
[profile.dev]
opt-level=3
''';(h/'Cargo.toml').write_text(cargo)
 (h/'src/main.rs').write_text('''use rrrah_dedup::{linear::LinearRgbaView,local::LocalError};
fn main() {
    let mode=std::env::args().nth(1).expect("baseline or ring");
    let (width,height,radius)=(2048u32,1536u32,4u32);
    let count=width as usize*height as usize;
    let mut input=Vec::with_capacity(count*4);
    for i in 0..count {let v=(i%4093)as f32/4096.;input.extend([v,v*2.-0.5,1.-v,1.]);}
    let view=LinearRgbaView::new(width,height,&input,count as u64,||false).unwrap();
    let budget=rrrah_core::MemoryBudget::new(512*1024*1024);
    let taps=count as u64*(2*radius as u64+1)*2;
    let result=match mode.as_str(){
      "baseline"=>original_full_frame(&view,radius,taps,&budget,||false),
      "ring"=>rrrah_dedup::gradient_scale::smooth_gradient_candidates_managed(&view,radius,taps,&budget,||false),
      _=>panic!("unknown mode"),
    }.unwrap();
    let mut digest=blake3::Hasher::new();
    for value in result.iter(){digest.update(&value.to_bits().to_le_bytes());}
    let retained=budget.used();drop(result);assert_eq!(budget.used(),0);
    println!("{}",serde_json::json!({"mode":mode,"dimensions":[width,height],"radius":radius,"output_digest":digest.finalize().to_hex().to_string(),"managed_peak":budget.peak(),"retained":retained,"after_drop":budget.used()}));
}
'''+baseline)
 pins={str(s/k):v for k,v in m['file_hashes'].items()};pins.update({str(p):sha(p)for p in [mp,Path(m['parent_manifest']),old,h/'Cargo.toml',h/'src/main.rs',Path(__file__)]});r={'status':'building','producer_pid':os.getpid(),'pins':pins,'scope':'Actual original full-frame versus row-ring smoothing on identical 2048x1536 RGB/HDR samples, radius4, separate timed processes. No decode/model/whole-portfolio RSS qualification.'}
 def save():p=out.with_suffix('.tmp');p.write_text(json.dumps(r,indent=2)+'\n');p.replace(out)
 save()
 try:
  log=T/'rrrah-native-row-ring-memory-build-20261012.log';target=T/'rrrah-native-row-ring-memory-target-20261012'
  with log.open('w')as f:
   p=subprocess.Popen(['cargo','build','--manifest-path',str(h/'Cargo.toml'),'--offline','--target-dir',str(target)],cwd='/tmp',stdout=f,stderr=subprocess.STDOUT);r['build_pid']=p.pid;save();code=p.wait()
  r.update(build_returncode=code,build_log=str(log),build_log_sha256=sha(log));save();assert code==0
  b=T/'rrrah-native-row-ring-memory-executable-20261012';assert not b.exists();shutil.copy2(target/'debug/native-row-ring-memory-probe',b);pins[str(b)]=sha(b);pins[str(h/'Cargo.lock')]=sha(h/'Cargo.lock');r.update(status='running',results=[]);save()
  for mode in ['baseline','ring']:
   cmd=['/usr/bin/time','-l',str(b),mode];start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);r['child_pid']=p.pid;save();stdout,stderr=p.communicate();assert p.returncode==0;value=json.loads(stdout);rss=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1]);r['results'].append(dict(mode=mode,returncode=p.returncode,command=cmd,stdout=stdout,stderr=stderr,evidence=value,rss_bytes=rss,elapsed_seconds=time.monotonic()-start));save()
  a,b=r['results'];assert a['evidence']['output_digest']==b['evidence']['output_digest'];assert all(x['evidence']['after_drop']==0 for x in [a,b]);assert all(sha(p)==v for p,v in pins.items());r['status']='complete_identical_smoothing_stage_memory_control';save();print([(x['mode'],x['rss_bytes'],x['evidence']['managed_peak'])for x in r['results']],flush=True)
 except Exception as e:r.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
