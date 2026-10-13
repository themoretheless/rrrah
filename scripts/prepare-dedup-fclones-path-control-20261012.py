#!/usr/bin/env python3
import hashlib,json,os,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 manifest=D/'fclones-hasher-source-20261012.json';m=json.loads(manifest.read_text());source=Path(m['snapshot'])
 assert all(sha(source/k)==v for k,v in m['file_hashes'].items())
 h=T/'rrrah-fclones-public-path-control-20261012';h.mkdir();(h/'src').mkdir()
 (h/'Cargo.toml').write_text('[package]\nname="rrrah-fclones-public-path-control"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nfclones={path="'+str(source/'fclones')+'"}\n')
 code='''use fclones::Path;
fn main() {
 let pairs=[("/foo", "/foo/bar"), ("/", "/foo"), ("a", "a/b"), ("/foo/bar", "/foo"), ("/foo/bar", "/other")];
 let mut differences=0;
 for (path,base) in pairs {
  let actual=Path::from(path).strip_prefix(&Path::from(base)).map(|p|p.to_path_buf());
  let expected=std::path::Path::new(path).strip_prefix(base).ok().map(|p|if p.as_os_str().is_empty(){std::path::PathBuf::from(".")}else{p.to_path_buf()});
  println!("path={path:?} base={base:?} original={actual:?} standard={expected:?}");
  if actual!=expected {differences+=1;}
 }
 assert_eq!(differences,3);
 #[cfg(unix)] {
  use std::os::unix::ffi::OsStringExt;
  let raw=std::ffi::OsString::from_vec(vec![b'/', b'x', 0xff]);
  assert_eq!(Path::from(std::path::PathBuf::from(raw.clone())).to_path_buf().into_os_string(),raw);
  println!("non_utf8_roundtrip=true");
 }
}
'''
 (h/'src/main.rs').write_text(code)
 lock=subprocess.run(['cargo','+stable','generate-lockfile','--offline','--manifest-path',str(h/'Cargo.toml')],cwd='/tmp',capture_output=True,text=True);assert lock.returncode==0,lock.stderr
 report=D/'fclones-public-path-control-20261012.json';assert not report.exists()
 pins={str(p):sha(p) for p in [manifest,h/'Cargo.toml',h/'Cargo.lock',h/'src/main.rs',Path(__file__)]}
 result={'status':'building','producer_pid':os.getpid(),'pins':pins,'source_commit':m['commit']}
 def save():report.write_text(json.dumps(result,indent=2)+'\n')
 save();log=T/'rrrah-fclones-public-path-control-build-20261012.log'
 with log.open('w') as f:
  child=subprocess.Popen(['cargo','+stable','build','--locked','--offline','--manifest-path',str(h/'Cargo.toml'),'--target-dir',str(T/'rrrah-fclones-target-20261012')],cwd='/tmp',stdout=f,stderr=subprocess.STDOUT);result['build_pid']=child.pid;save();rc=child.wait()
 result['build_returncode']=rc;pins[str(log)]=sha(log);save();assert rc==0
 import shutil
 exe=T/'rrrah-fclones-public-path-control-executable-20261012';shutil.copy2(T/'rrrah-fclones-target-20261012/debug/rrrah-fclones-public-path-control',exe);pins[str(exe)]=sha(exe)
 run=subprocess.run([str(exe)],capture_output=True,text=True,cwd='/tmp');result.update(status='verified_three_public_strip_prefix_counterexamples' if run.returncode==0 else 'failed',returncode=run.returncode,stdout=run.stdout,stderr=run.stderr);save();assert run.returncode==0;print(run.stdout)
if __name__=='__main__':main()
