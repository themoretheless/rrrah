#[cfg(test)]
mod controls {
    use super::*;
    use std::collections::VecDeque;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let nonce=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path=std::env::temp_dir().join(format!("rrrah-fddf273-{}-{nonce}",std::process::id()));
            std::fs::create_dir(&path).unwrap();Self(path)
        }
        fn file(&self,name:usize,bytes:&[u8])->PathBuf {
            let path=self.0.join(name.to_string());std::fs::write(&path,bytes).unwrap();path
        }
    }
    impl Drop for Fixture {fn drop(&mut self){std::fs::remove_dir_all(&self.0).unwrap();}}
    fn run(size:u64,paths:Vec<PathBuf>)->Vec<(u64,Vec<PathBuf>)>{
        let(tx,rx)=std::sync::mpsc::channel();compare_files(false,size,paths,tx);rx.into_iter().collect()
    }
    #[test]
    fn healthy_real_files_partition_by_bytes_at_every_block_edge(){
        for size in [0,1,4095,4096,4097,12289]{
            let f=Fixture::new();let a=vec![17;size];let mut b=a.clone();if size>0{b[size/2]^=1;}
            let paths=vec![f.file(0,&a),f.file(1,&a),f.file(2,&b),f.file(3,&b)];let groups=run(size as u64,paths);
            assert_eq!(groups.len(),if size==0{1}else{2});
            assert_eq!(groups.iter().map(|(_,v)|v.len()).sum::<usize>(),4);
            for(_,paths)in groups{let reference=std::fs::read(&paths[0]).unwrap();assert!(paths.iter().all(|p|std::fs::read(p).unwrap()==reference));}
        }
    }
    #[test]
    fn real_truncate_after_equal_setup_publishes_false_group_fast_and_slow(){
        for count in [2,99,100]{
            let f=Fixture::new();let paths:Vec<_>=(0..count).map(|i|f.file(i,b"AAAA")).collect();
            assert!(paths.iter().all(|p|std::fs::metadata(p).unwrap().len()==4));
            std::fs::write(&paths[count-1],[]).unwrap();
            assert_ne!(std::fs::read(&paths[0]).unwrap(),std::fs::read(&paths[count-1]).unwrap());
            let groups=run(4,paths);assert_eq!(groups.len(),1);assert_eq!(groups[0].0,4);assert_eq!(groups[0].1.len(),count);
        }
    }
    #[test]
    fn real_growth_after_equal_setup_returns_no_group(){
        let f=Fixture::new();let paths=vec![f.file(0,b"AAAA"),f.file(1,b"AAAA")];
        std::fs::write(&paths[1],b"AAAAB").unwrap();assert!(run(4,paths).is_empty());
    }
    struct Scripted {path:PathBuf,steps:VecDeque<Result<Vec<u8>,()>>,buffer:Vec<u8>}
    impl Candidate for Scripted {
        fn read_block(&mut self)->Result<usize,()>{self.buffer=self.steps.pop_front().unwrap_or(Ok(Vec::new()))?;Ok(self.buffer.len())}
        fn buf_equal(&self,other:&Self)->bool{self.buffer==other.buffer}
        fn into_path(self)->PathBuf{self.path}
    }
    fn scripted(id:&str,steps:Vec<Result<Vec<u8>,()>>)->Scripted{Scripted{path:id.into(),steps:steps.into(),buffer:Vec::new()}}
    fn fake(values:Vec<Scripted>)->Vec<(u64,Vec<PathBuf>)>{let(tx,rx)=std::sync::mpsc::channel();compare_files_inner(3,values,&tx);drop(tx);rx.into_iter().collect()}
    #[test]
    fn permitted_short_read_framing_misses_identical_logical_bytes(){
        let a=vec![b"A".to_vec(),b"BC".to_vec()];let b=vec![b"ABC".to_vec()];assert_eq!(a.concat(),b.concat());
        assert!(fake(vec![scripted("a",a.into_iter().map(Ok).collect()),scripted("b",b.into_iter().map(Ok).collect())]).is_empty());
    }
    #[test]
    fn error_then_eof_can_publish_singleton_as_duplicate_group(){
        let groups=fake(vec![scripted("empty",vec![Ok(Vec::new())]),scripted("error",vec![Err(())])]);
        assert_eq!(groups.len(),1);assert_eq!(groups[0].1,vec![PathBuf::from("empty")]);
    }
    #[test]
    fn all_reader_errors_publish_no_groups(){assert!(fake(vec![scripted("a",vec![Err(())]),scripted("b",vec![Err(())])]).is_empty());}
}
