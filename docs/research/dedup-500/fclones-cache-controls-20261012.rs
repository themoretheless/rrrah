#[cfg(test)]
mod rrrah_cache_controls {
    use crate::{cache::HashCache, file::{FileChunk, FileLen, FileMetadata, FilePos}, hasher::{FileHasher, HashFn}, log::StdLog, path::Path};
    use std::{fs, time::{Duration, UNIX_EPOCH}};

    fn control(second_nanos: u32) {
        let root = tempfile::tempdir().unwrap();
        let native = root.path().join("input");
        fs::write(&native, b"AAAA").unwrap();
        let initial_time = UNIX_EPOCH + Duration::new(1_700_000_000, 123_400_100);
        fs::File::options().write(true).open(&native).unwrap()
            .set_times(fs::FileTimes::new().set_modified(initial_time)).unwrap();
        let path = Path::from(native.clone());
        let before = FileMetadata::new(&path).unwrap();
        assert_eq!(before.modified().unwrap(), initial_time);
        let cache = HashCache::open(&Path::from(root.path().join("cache")), None, HashFn::Blake3).unwrap();
        let log = StdLog::new();
        let cached = FileHasher { algorithm: HashFn::Blake3, buf_len: 65536, cache: Some(cache), transform: None, log: &log };
        let chunk = FileChunk::new(&path, FilePos(0), FileLen(4));
        let original_hash = cached.hash_file(&chunk, |_| {}).unwrap();
        fs::write(&native, b"BBBB").unwrap();
        let second_time = UNIX_EPOCH + Duration::new(1_700_000_000, second_nanos);
        fs::File::options().write(true).open(&native).unwrap()
            .set_times(fs::FileTimes::new().set_modified(second_time)).unwrap();
        let after = FileMetadata::new(&path).unwrap();
        assert_eq!(before.file_id(), after.file_id());
        assert_eq!(before.len(), after.len());
        assert_eq!(after.modified().unwrap(), second_time);
        assert_eq!(before.modified().unwrap().duration_since(UNIX_EPOCH).unwrap().as_millis(), after.modified().unwrap().duration_since(UNIX_EPOCH).unwrap().as_millis());
        if second_nanos != 123_400_100 { assert_ne!(before.modified().unwrap(), after.modified().unwrap()); }
        let reused = cached.hash_file(&chunk, |_| {}).unwrap();
        let fresh = FileHasher::new(HashFn::Blake3, None, &log).hash_file(&chunk, |_| {}).unwrap();
        assert_eq!(reused, original_hash);
        assert_ne!(fresh, original_hash);
    }

    #[test]
    fn original_cached_hasher_reuses_old_hash_after_restored_mtime() { control(123_400_100); }

    #[test]
    fn original_cached_hasher_reuses_old_hash_after_submillisecond_edit() { control(123_400_900); }
}
