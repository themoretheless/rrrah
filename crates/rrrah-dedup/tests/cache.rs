use rrrah_dedup::{
    cache::{CacheKey, FingerprintCache},
    pixels::Fingerprint,
};

#[test]
fn roundtrip_requires_content_recipe_and_frame_identity() {
    let key = CacheKey {
        content: [1; 32],
        recipe: [2; 32],
        frame_index: 3,
    };
    let fp = Fingerprint {
        variants: [[123; 4]; 8],
        mean_linear_rgb: [0.1, 0.2, 0.3],
        luminance_stddev: 0.4,
    };
    let mut cache = FingerprintCache::default();
    cache.insert(key, fp.clone(), 1).unwrap();
    let mut bytes = Vec::new();
    cache.write(&mut bytes, || false).unwrap();
    let restored = FingerprintCache::read(bytes.as_slice(), 1, || false).unwrap();
    assert_eq!(restored.get(&key), Some(&fp));
    for other in [
        CacheKey {
            content: [9; 32],
            ..key
        },
        CacheKey {
            recipe: [9; 32],
            ..key
        },
        CacheKey {
            frame_index: 9,
            ..key
        },
    ] {
        assert!(restored.get(&other).is_none());
    }
    assert!(FingerprintCache::read(bytes.as_slice(), 0, || false).is_err());
    assert!(FingerprintCache::read(bytes.as_slice(), 1, || true).is_err());
    for index in [0, 20, 100, bytes.len() - 1] {
        let mut corrupt = bytes.clone();
        corrupt[index] ^= 1;
        assert!(FingerprintCache::read(corrupt.as_slice(), 1, || false).is_err());
    }
    for end in [0, 15, 16, 32, bytes.len() - 1] {
        assert!(FingerprintCache::read(&bytes[..end], 1, || false).is_err());
    }
    bytes.push(0);
    assert!(FingerprintCache::read(bytes.as_slice(), 1, || false).is_err());
}

#[test]
fn disk_persistence_is_deterministic_and_rejects_invalid_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fingerprints.bin");
    let fp = Fingerprint {
        variants: [[0; 4]; 8],
        mean_linear_rgb: [0.0; 3],
        luminance_stddev: 0.0,
    };
    let a = CacheKey {
        content: [1; 32],
        recipe: [2; 32],
        frame_index: 0,
    };
    let b = CacheKey {
        content: [3; 32],
        ..a
    };
    let mut first = FingerprintCache::default();
    let mut second = FingerprintCache::default();
    for key in [a, b] {
        first.insert(key, fp.clone(), 2).unwrap();
    }
    for key in [b, a] {
        second.insert(key, fp.clone(), 2).unwrap();
    }
    let mut one = Vec::new();
    let mut two = Vec::new();
    first.write(&mut one, || false).unwrap();
    second.write(&mut two, || false).unwrap();
    assert_eq!(one, two);
    first
        .write(std::fs::File::create(&path).unwrap(), || false)
        .unwrap();
    let restored = FingerprintCache::read(std::fs::File::open(&path).unwrap(), 2, || false).unwrap();
    assert_eq!(restored.get(&b), Some(&fp));
    assert!(
        first
            .insert(CacheKey { frame_index: 9, ..a }, fp.clone(), 2)
            .is_err()
    );
    first.insert(a, fp.clone(), 2).unwrap();
    for invalid in [
        Fingerprint {
            luminance_stddev: -1.0,
            ..fp.clone()
        },
        Fingerprint {
            mean_linear_rgb: [f64::NAN; 3],
            ..fp.clone()
        },
    ] {
        assert!(first.insert(a, invalid, 2).is_err());
    }
    assert_eq!(first.get(&a), Some(&fp));
}

#[test]
fn atomic_replacement_and_cancel_preserve_previous_cache() {
    use std::cell::Cell;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.bin");
    let key = CacheKey {
        content: [1; 32],
        recipe: [2; 32],
        frame_index: 0,
    };
    let fp = Fingerprint {
        variants: [[0; 4]; 8],
        mean_linear_rgb: [0.0; 3],
        luminance_stddev: 0.0,
    };
    let mut cache = FingerprintCache::default();
    cache.insert(key, fp.clone(), 1).unwrap();
    cache.save_atomic(&path, || false).unwrap();
    let original = std::fs::read(&path).unwrap();
    let changed = Fingerprint {
        variants: [[999; 4]; 8],
        ..fp
    };
    cache.insert(key, changed.clone(), 1).unwrap();
    let calls = Cell::new(0);
    // Cancel after records and checksum have been written, before publication.
    assert!(
        cache
            .save_atomic(&path, || {
                calls.set(calls.get() + 1);
                calls.get() >= 4
            })
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    cache.save_atomic(&path, || false).unwrap();
    let restored = FingerprintCache::load_file(&path, 1, || false).unwrap();
    assert_eq!(restored.get(&key), Some(&changed));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

fn process_fixture(writer: u8) -> FingerprintCache {
    let mut cache = FingerprintCache::default();
    for record in 0..64_u8 {
        cache
            .insert(
                CacheKey {
                    content: [record; 32],
                    recipe: [writer; 32],
                    frame_index: 0,
                },
                Fingerprint {
                    variants: [[u64::from(writer); 4]; 8],
                    mean_linear_rgb: [f64::from(writer); 3],
                    luminance_stddev: 0.5,
                },
                64,
            )
            .unwrap();
    }
    cache
}

fn published_writer(cache: &FingerprintCache) -> u8 {
    let writers = (0..=4_u8)
        .filter(|&writer| {
            cache
                .get(&CacheKey {
                    content: [0; 32],
                    recipe: [writer; 32],
                    frame_index: 0,
                })
                .is_some()
        })
        .collect::<Vec<_>>();
    assert_eq!(writers.len(), 1);
    let writer = writers[0];
    for record in 0..64_u8 {
        let fp = cache
            .get(&CacheKey {
                content: [record; 32],
                recipe: [writer; 32],
                frame_index: 0,
            })
            .unwrap();
        assert_eq!(fp.variants, [[u64::from(writer); 4]; 8]);
        for other in 0..=4_u8 {
            if other != writer {
                assert!(
                    cache
                        .get(&CacheKey {
                            content: [record; 32],
                            recipe: [other; 32],
                            frame_index: 0
                        })
                        .is_none()
                );
            }
        }
    }
    writer
}

#[test]
fn cache_publication_child() {
    let Some(path) = std::env::var_os("RRRAH_CACHE_PROCESS_PATH") else {
        return;
    };
    let writer = std::env::var("RRRAH_CACHE_PROCESS_WRITER")
        .unwrap()
        .parse::<u8>()
        .unwrap();
    let cache = process_fixture(writer);
    if let Ok(crash_at) = std::env::var("RRRAH_CACHE_PROCESS_CRASH") {
        let crash_at = crash_at.parse::<usize>().unwrap();
        let calls = std::cell::Cell::new(0);
        let _ = cache.save_atomic(path, || {
            calls.set(calls.get() + 1);
            if calls.get() == crash_at {
                std::process::exit(37);
            }
            false
        });
        panic!("crash checkpoint was not reached");
    }
    for _ in 0..20 {
        cache.save_atomic(&path, || false).unwrap();
    }
}

fn child(path: &std::path::Path, writer: u8, crash_at: usize) -> std::process::Child {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "cache_publication_child", "--nocapture"])
        .env("RRRAH_CACHE_PROCESS_PATH", path)
        .env("RRRAH_CACHE_PROCESS_WRITER", writer.to_string())
        .stdout(std::process::Stdio::null());
    if crash_at > 0 {
        command.env("RRRAH_CACHE_PROCESS_CRASH", crash_at.to_string());
    }
    command.spawn().unwrap()
}

#[test]
fn concurrent_process_writers_publish_only_complete_single_writer_caches() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.bin");
    process_fixture(0).save_atomic(&path, || false).unwrap();
    let mut children = (1..=4).map(|writer| child(&path, writer, 0)).collect::<Vec<_>>();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut reads = 0;
    loop {
        let cache = FingerprintCache::load_file(&path, 64, || false).unwrap();
        published_writer(&cache);
        reads += 1;
        let done = children.iter_mut().all(|process| {
            process.try_wait().unwrap().is_some_and(|status| {
                assert!(status.success());
                true
            })
        });
        if done {
            break;
        }
        if std::time::Instant::now() >= deadline {
            for process in &mut children {
                let _ = process.kill();
                let _ = process.wait();
            }
            panic!("cache writer processes timed out");
        }
        std::thread::yield_now();
    }
    assert!(reads > 0);
    assert!(published_writer(&FingerprintCache::load_file(&path, 64, || false).unwrap()) > 0);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn process_exit_during_temp_write_preserves_previous_publication_and_retry() {
    // 8: partial records; 67: synchronized complete file, before replacement.
    for crash_at in [8, 67] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");
        process_fixture(0).save_atomic(&path, || false).unwrap();
        let original = std::fs::read(&path).unwrap();
        assert_eq!(child(&path, 1, crash_at).wait().unwrap().code(), Some(37));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(
            published_writer(&FingerprintCache::load_file(&path, 64, || false).unwrap()),
            0
        );
        let abandoned = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p != &path)
            .collect::<Vec<_>>();
        assert_eq!(abandoned.len(), 1);
        let orphan = FingerprintCache::load_file(&abandoned[0], 64, || false);
        if crash_at == 8 {
            assert!(orphan.is_err());
        } else {
            assert_eq!(published_writer(&orphan.unwrap()), 1);
        }
        process_fixture(2).save_atomic(&path, || false).unwrap();
        assert_eq!(
            published_writer(&FingerprintCache::load_file(&path, 64, || false).unwrap()),
            2
        );
    }
}

#[cfg(unix)]
#[test]
fn unwritable_parent_refuses_before_publication_and_preserves_retry() {
    use rrrah_dedup::cache::CacheError;
    use std::os::unix::fs::PermissionsExt;
    // This qualification must run without the privileged DAC bypass.
    let identity = std::process::Command::new("id").arg("-u").output().unwrap();
    assert!(identity.status.success());
    assert_ne!(
        String::from_utf8(identity.stdout).unwrap().trim(),
        "0",
        "run cache permission qualification as a non-root user"
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.bin");
    process_fixture(0).save_atomic(&path, || false).unwrap();
    let previous = std::fs::read(&path).unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    let denied = process_fixture(1).save_atomic(&path, || false);
    // Restore before assertions, so failure does not obstruct temp-directory cleanup.
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        matches!(denied, Err(CacheError::Io(error)) if error.kind() == std::io::ErrorKind::PermissionDenied)
    );
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    assert_eq!(
        published_writer(&FingerprintCache::load_file(&path, 64, || false).unwrap()),
        0
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    process_fixture(1).save_atomic(&path, || false).unwrap();
    assert_ne!(std::fs::read(&path).unwrap(), previous);
    assert_eq!(
        published_writer(&FingerprintCache::load_file(&path, 64, || false).unwrap()),
        1
    );
}
