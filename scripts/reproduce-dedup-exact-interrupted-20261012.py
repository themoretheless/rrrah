#!/usr/bin/env python3
import hashlib,json,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 before=T/'rrrah-exact-interrupted-before-20261012.rs';code=before.read_text();start=code.index('fn read_block(');end=code.index('\nfn allocate_read_buffer',start);original=code[start:end];assert original.count('file: &mut File')==1
 adapted=original.replace('file: &mut File','file: &mut impl Read');src=T/'rrrah-exact-interrupted-original-control-20261012.rs';assert not src.exists();src.write_text('''use std::{io::{self,Read},cell::Cell};
#[derive(Debug)] enum Failure { Io(io::Error) }
'''+adapted+'''
struct Reader<'a>(&'a Cell<usize>);
impl Read for Reader<'_> { fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
 self.0.set(self.0.get()+1);
 if self.0.get()<=3 { return Err(io::ErrorKind::Interrupted.into()); }
 buffer[0]=42; Ok(1)
}}
fn main(){ let calls=Cell::new(0);let mut reader=Reader(&calls);let mut buffer=[0];
 assert_eq!(read_block(&mut reader,&mut buffer).unwrap(),1);
 assert_eq!(calls.get(),4);assert_eq!(buffer,[42]);
 // Cancellation requested by the third interruption cannot be observed by the original helper.
 println!("Original helper continued reading after third Interrupted:4 attempts, byte42");
}
''');exe=T/'rrrah-exact-interrupted-original-control-20261012';assert not exe.exists();subprocess.run(['rustc','--edition=2024',str(src),'-o',str(exe)],check=True,cwd='/tmp');result=subprocess.run([str(exe)],capture_output=True,text=True,check=True);log=T/'rrrah-exact-interrupted-original-control-20261012.log';log.write_text(result.stdout+result.stderr);out=D/'dedup-exact-interrupted-original-control-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'original_read_loop_continues_after_repeated_interruption','pins':{str(p):sha(p) for p in [before,src,exe,log,Path(__file__)]},'scope':'Original read_block body extracted unchanged, except File argument generalized to impl Read to inject deterministic interrupts. Three Interrupted responses followed by success; helper makes4 attempts. Original helper has no callback checkpoint. Not a physical filesystem/signal test or kernel-call preemption proof.'},indent=2)+'\n');print(result.stdout)
if __name__=='__main__':main()
