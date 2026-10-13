// MIT License, Copyright (c) 2020 Piotr Kołaczkowski. Original scan and TLS buffer block from pinned fclones hasher.rs; see source LICENSE.
use std::{cell::RefCell,cmp::{max,min},io::{self,Read}};
#[derive(Clone,Copy)] struct FileLen(u64);
impl From<FileLen> for u64 { fn from(x:FileLen)->Self{x.0} }
thread_local! {
    static BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) }
}

/// Scans up to `len` bytes in a file and sends data to the given consumer.
/// Returns the number of bytes successfully read.
fn scan<F: FnMut(&[u8])>(
    stream: &mut impl Read,
    len: FileLen,
    buf_len: usize,
    mut consumer: F,
) -> io::Result<u64> {
    BUF.with(|buf| {
        let mut buf = buf.borrow_mut();
        let new_len = max(buf.len(), buf_len);
        buf.resize(new_len, 0);
        let mut read: u64 = 0;
        let len = len.into();
        while read < len {
            let remaining = len - read;
            let to_read = min(remaining, buf.len() as u64) as usize;
            let buf = &mut buf[..to_read];
            match stream.read(buf) {
                Ok(0) => break,
                Ok(actual_read) => {
                    read += actual_read as u64;
                    (consumer)(&buf[..actual_read]);
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(read)
    })
}


#[cfg(test)] mod controls {
use super::*;
use std::io::Cursor;
fn collect(data:&[u8],len:u64,buf:usize)->(u64,Vec<u8>){
 let mut out=Vec::new();let n=scan(&mut Cursor::new(data),FileLen(len),buf,|x|out.extend_from_slice(x)).unwrap();(n,out)
}
#[test] fn exact_requested_prefix(){assert_eq!(collect(b"abcdef",3,2),(3,b"abc".to_vec()));}
#[test] fn early_eof_reports_short_success(){assert_eq!(collect(b"abc",9,2),(3,b"abc".to_vec()));}
#[test] fn zero_buffer_returns_no_data(){BUF.with(|b|b.borrow_mut().clear());assert_eq!(collect(b"abc",3,0),(0,vec![]));}
#[test] fn thread_buffer_retains_largest_request(){
 BUF.with(|b|b.borrow_mut().clear());collect(b"x",1,1048576);let before=BUF.with(|b|(b.borrow().len(),b.borrow().capacity()));
 collect(b"x",1,1);let after=BUF.with(|b|(b.borrow().len(),b.borrow().capacity()));assert_eq!(before,after);assert!(after.1>=1048576);
}
struct Interrupted;
impl Read for Interrupted{fn read(&mut self,_:&mut[u8])->io::Result<usize>{Err(io::ErrorKind::Interrupted.into())}}
#[test] fn interrupted_read_is_propagated(){assert_eq!(scan(&mut Interrupted,FileLen(4),2,|_|panic!()).unwrap_err().kind(),io::ErrorKind::Interrupted);}
struct Short(Cursor<Vec<u8>>);
impl Read for Short{fn read(&mut self,b:&mut[u8])->io::Result<usize>{let n=b.len().min(1);self.0.read(&mut b[..n])}}
#[test] fn short_reads_accumulate(){let mut out=Vec::new();let n=scan(&mut Short(Cursor::new(b"abc".to_vec())),FileLen(3),8,|b|out.extend_from_slice(b)).unwrap();assert_eq!((n,out),(3,b"abc".to_vec()));}
}
