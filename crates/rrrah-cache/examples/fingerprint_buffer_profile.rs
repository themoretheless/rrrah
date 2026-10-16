//! Alternating paired experiment; production fingerprint logic is unchanged.
use rrrah_cache::SourceFingerprint;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::{Instant, SystemTime},
};
fn fingerprint(path: &Path, mode: usize) -> SourceFingerprint {
    let metadata = std::fs::metadata(path).unwrap();
    let size = metadata.len();
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let mut file = File::open(path).unwrap();
    let mut hash = blake3::Hasher::new();
    hash.update(b"rrrah/source-sample/v1\0");
    hash.update(&size.to_le_bytes());
    let mut offsets = if size <= 65536 {
        vec![0]
    } else {
        vec![0, (size - 65536) / 2, size - 65536]
    };
    offsets.sort_unstable();
    offsets.dedup();
    let reuse = mode != 0;
    let coalesce = mode == 2 && size <= 131072;
    let mut whole = if coalesce {
        vec![0u8; size as usize]
    } else {
        Vec::new()
    };
    if coalesce {
        file.read_exact(&mut whole).unwrap();
    }
    let mut shared = if reuse {
        vec![0u8; size.min(65536) as usize]
    } else {
        Vec::new()
    };
    for offset in offsets {
        if !coalesce {
            file.seek(SeekFrom::Start(offset)).unwrap();
        }
        hash.update(&offset.to_le_bytes());
        let len = (size - offset).min(65536) as usize;
        if coalesce {
            hash.update(&whole[offset as usize..offset as usize + len]);
            continue;
        }
        let mut owned = if reuse { Vec::new() } else { vec![0u8; len] };
        let sample = if reuse {
            &mut shared[..len]
        } else {
            owned.as_mut_slice()
        };
        file.read_exact(sample).unwrap();
        hash.update(sample);
    }
    SourceFingerprint {
        file_size: size,
        modified_ns,
        sampled_blake3: *hash.finalize().as_bytes(),
    }
}
fn main() {
    let path = std::path::PathBuf::from(std::env::args_os().nth(1).expect("source path"));
    let expected = SourceFingerprint::from_path(&path).unwrap();
    for mode in 0..3 {
        assert_eq!(fingerprint(&path, mode), expected);
    }
    let mut pairs = Vec::new();
    for round in 0..11 {
        let mut times = [0u128; 3];
        for mode in if round % 2 == 0 { [0, 1, 2] } else { [2, 1, 0] } {
            let start = Instant::now();
            for _ in 0..256 {
                let value = fingerprint(std::hint::black_box(&path), mode);
                std::hint::black_box(value);
            }
            times[mode] = start.elapsed().as_nanos() / 256;
        }
        pairs.push(times);
    }
    println!(
        "{}",
        serde_json::json!({"source":path,"bytes":expected.file_size,"iterations_per_interval":256,"order":"alternating baseline/reuse/coalesce","pairs_ns":pairs,"equal_production_fingerprint":true})
    );
}
