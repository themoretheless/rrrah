//! Bounded synthetic A/B probes for RR-0013/RR-0019; no image decode claims.
use std::{
    fs,
    hint::black_box,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

fn cover(root: &Path, cached_type: bool) -> Option<PathBuf> {
    fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let supported = path.extension().is_some_and(|e| e == "jpg");
            let file = if cached_type {
                supported && entry.file_type().unwrap().is_file()
            } else {
                fs::symlink_metadata(&path).unwrap().file_type().is_file() && supported
            };
            file.then_some(path)
        })
        .min()
}
fn prefetch(paths: Option<Vec<PathBuf>>, optimized: bool, duplicate: bool) {
    let identity = Arc::new(Mutex::new(if optimized { None } else { paths.clone() }));
    let slot = if duplicate {
        Arc::new(Mutex::new(None))
    } else {
        identity.clone()
    };
    *slot.lock().unwrap() = if duplicate && !optimized {
        paths.clone()
    } else {
        paths
    };
    black_box((identity, slot));
}
fn main() {
    let root = std::env::temp_dir().join(format!("rrrah-first30-probe-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    println!(
        "{{\"scope\":\"synthetic filesystem and prefetch ownership operations; no end-to-end viewer result\",\"pairs\":["
    );
    let mut comma = "";
    for count in [1000, 10000] {
        for i in 0..count {
            fs::write(root.join(format!("{i:05}.jpg")), []).unwrap();
            fs::write(root.join(format!("{i:05}.junk")), []).unwrap();
        }
        fs::create_dir(root.join("000-directory.jpg")).ok();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("00000.jpg"), root.join("000-link.jpg")).ok();
        }
        for _ in 0..3 {
            assert_eq!(cover(&root, false), cover(&root, true));
        }
        for pair in 0..10 {
            let mut times = [0u128; 2];
            for mode in [pair % 2, 1 - pair % 2] {
                let start = Instant::now();
                let path = cover(&root, mode == 1).unwrap();
                assert_eq!(path.file_name().unwrap(), "00000.jpg");
                black_box(path);
                times[mode] = start.elapsed().as_nanos();
            }
            print!(
                "{comma}{{\"task\":\"RR-0019\",\"files\":{count},\"pair\":{pair},\"baseline_ns\":{},\"candidate_ns\":{}}}",
                times[0], times[1]
            );
            comma = ",\n";
        }
    }
    let paths: Vec<_> = (0..7)
        .map(|i| PathBuf::from(format!("/synthetic/{i}/{}", "a".repeat(512))))
        .collect();
    for duplicate in [false, true] {
        for _ in 0..3 {
            prefetch(Some(paths.clone()), false, duplicate);
            prefetch(Some(paths.clone()), true, duplicate);
        }
        for pair in 0..10 {
            let mut times = [0u128; 2];
            for mode in [pair % 2, 1 - pair % 2] {
                for _ in 0..10000 {
                    let input = Some(paths.clone()); // Caller preparation excluded from operation timing.
                    let start = Instant::now();
                    prefetch(black_box(input), mode == 1, duplicate);
                    times[mode] += start.elapsed().as_nanos();
                }
            }
            print!(
                "{comma}{{\"task\":\"RR-0013\",\"duplicate\":{duplicate},\"iterations\":10000,\"pair\":{pair},\"baseline_ns\":{},\"candidate_ns\":{}}}",
                times[0], times[1]
            );
            comma = ",\n";
        }
    }
    println!("\n]}}");
}
