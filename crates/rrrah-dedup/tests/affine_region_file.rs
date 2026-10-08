#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    affine_color::AffineColorPolicy,
    affine_region::AffineRegionPolicy,
    affine_region_file::{AffineRegionFileError, AffineRegionFilePolicy, verify_affine_region_files},
    animated::AnimationBudget,
    geometry::ProjectiveTransform,
};
#[test]
fn cancellation_and_cumulative_work_refuse_before_missing_source_io() {
    let request = DecodeRequest::new("/nonexistent/rrrah-affine-region-preflight.png");
    let region = AffineRegionPolicy {
        radius: 1,
        maximum_sites: 256,
        maximum_pixel_reads: 11520,
        color: AffineColorPolicy {
            minimum_samples: 100,
            maximum_samples: 256,
            minimum_variance: 1e-8,
            minimum_relative_pivot: 1e-8,
            maximum_coefficient: 5.,
            maximum_offset: 0.1,
        },
        tolerance: 0.03,
    };
    let policy = AffineRegionFilePolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 4096,
            max_file_bytes: 65536,
        },
        forward: region,
        reverse: region,
        maximum_total_pixel_reads: 23039,
    };
    let model = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    };
    let b = MemoryBudget::new(1048576);
    assert!(matches!(
        verify_affine_region_files(
            &request,
            &request,
            model,
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            policy,
            &b,
            || true
        ),
        Err(AffineRegionFileError::Cancelled)
    ));
    assert!(matches!(
        verify_affine_region_files(
            &request,
            &request,
            model,
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            policy,
            &b,
            || false
        ),
        Err(AffineRegionFileError::Budget)
    ));
    assert_eq!(b.used(), 0);
    assert_eq!(b.peak(), 0);
}

fn color_bmp() -> Vec<u8> {
    let pixels = 64 * 64 * 3u32;
    let mut data = Vec::new();
    data.extend(b"BM");
    data.extend((54 + pixels).to_le_bytes());
    data.extend([0; 4]);
    data.extend(54u32.to_le_bytes());
    data.extend(40u32.to_le_bytes());
    data.extend(64i32.to_le_bytes());
    data.extend(64i32.to_le_bytes());
    data.extend(1u16.to_le_bytes());
    data.extend(24u16.to_le_bytes());
    data.extend([0; 24]);
    for y in 0..64u32 {
        for x in 0..64u32 {
            data.extend([(x * y / 16) as u8, (y * 3) as u8, (x * 3) as u8]);
        }
    }
    data
}
#[test]
fn stable_files_verify_and_mutation_or_replacement_discards_evidence() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.bmp");
    let right = dir.path().join("right.bmp");
    let bytes = color_bmp();
    std::fs::write(&left, &bytes).unwrap();
    std::fs::write(&right, &bytes).unwrap();
    let a = DecodeRequest::new(&left);
    let b = DecodeRequest::new(&right);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let region = AffineRegionPolicy {
        radius: 1,
        maximum_sites: 256,
        maximum_pixel_reads: 11520,
        color: AffineColorPolicy {
            minimum_samples: 100,
            maximum_samples: 256,
            minimum_variance: 1e-8,
            minimum_relative_pivot: 1e-8,
            maximum_coefficient: 5.,
            maximum_offset: 0.1,
        },
        tolerance: 0.001,
    };
    let policy = AffineRegionFilePolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 4096,
            max_file_bytes: 65536,
        },
        forward: region,
        reverse: region,
        maximum_total_pixel_reads: 23040,
    };
    let model = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    };
    let calls = std::cell::Cell::new(0);
    let result = verify_affine_region_files(
        &a,
        &b,
        model,
        [16, 16, 16, 16],
        [16, 16, 16, 16],
        policy,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for e in result {
        assert_eq!(e.heldout.matched, e.heldout.samples);
        assert_eq!(e.heldout.samples, 128);
    }
    assert_eq!(budget.used(), 0);
    let total = calls.get();
    assert!(total > 1000);
    for stop in [total / 2, total] {
        calls.set(0);
        let result = verify_affine_region_files(
            &a,
            &b,
            model,
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            policy,
            &budget,
            || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        );
        assert!(matches!(result, Err(AffineRegionFileError::Cancelled)));
        assert_eq!(budget.used(), 0);
    }
    for replacement in [false, true] {
        std::fs::write(&right, &bytes).unwrap();
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = verify_affine_region_files(
            &a,
            &b,
            model,
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            policy,
            &budget,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == total / 2 {
                    if replacement {
                        let path = dir.path().join("replacement.bmp");
                        std::fs::write(&path, &bytes).unwrap();
                        std::fs::rename(path, &right).unwrap();
                    } else {
                        std::fs::OpenOptions::new()
                            .append(true)
                            .open(&right)
                            .unwrap()
                            .write_all(&[0])
                            .unwrap();
                    }
                    changed.set(true);
                }
                false
            },
        );
        assert!(changed.get());
        assert!(
            matches!(
                result,
                Err(AffineRegionFileError::Source(
                    rrrah_dedup::exact::SnapshotError::Changed
                ))
            ),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn automatic_file_grid_guards_source_and_retains_managed_regions() {
    use rrrah_dedup::{affine_region_file::verify_affine_grid_files, affine_region_grid::AffineGridPolicy};
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.bmp");
    let right = dir.path().join("right.bmp");
    let bytes = color_bmp();
    std::fs::write(&left, &bytes).unwrap();
    std::fs::write(&right, &bytes).unwrap();
    let a = DecodeRequest::new(&left);
    let b = DecodeRequest::new(&right);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let region = AffineRegionPolicy {
        radius: 1,
        maximum_sites: 1024,
        maximum_pixel_reads: 46080,
        color: AffineColorPolicy {
            minimum_samples: 100,
            maximum_samples: 1024,
            minimum_variance: 1e-8,
            minimum_relative_pivot: 1e-8,
            maximum_coefficient: 5.,
            maximum_offset: 0.1,
        },
        tolerance: 0.001,
    };
    let policy = AffineGridPolicy {
        region,
        grid: (2, 2),
        maximum_regions: 4,
        maximum_source_radius: 16,
        maximum_total_pixel_reads: 368640,
    };
    let decode = AnimationBudget {
        max_frames: 1,
        max_pixels: 4096,
        max_file_bytes: 65536,
    };
    let model = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    };
    let calls = std::cell::Cell::new(0);
    let regions = verify_affine_grid_files(&a, &b, model, decode, policy, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(regions.len(), 4);
    for row in regions.iter() {
        for value in row.directions {
            let value = value.unwrap();
            assert_eq!(value.heldout.matched, value.heldout.samples);
        }
    }
    let total = calls.get();
    assert!(budget.used() > 0);
    drop(regions);
    assert_eq!(budget.used(), 0);
    calls.set(0);
    let changed = std::cell::Cell::new(false);
    let result = verify_affine_grid_files(&a, &b, model, decode, policy, &budget, || {
        calls.set(calls.get() + 1);
        if calls.get() == total / 2 {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&right)
                .unwrap()
                .write_all(&[0])
                .unwrap();
            changed.set(true);
        }
        false
    });
    assert!(changed.get());
    assert!(
        matches!(
            result,
            Err(AffineRegionFileError::Source(
                rrrah_dedup::exact::SnapshotError::Changed
            ))
        ),
        "{result:?}"
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn common_footprint_files_preserve_mutation_replacement_and_sticky_cancel() {
    use rrrah_dedup::affine_region::AffineRegionFootprint;
    use rrrah_dedup::affine_region_file::verify_affine_region_files_with_footprints;
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.bmp");
    let right = dir.path().join("right.bmp");
    let bytes = color_bmp();
    std::fs::write(&left, &bytes).unwrap();
    std::fs::write(&right, &bytes).unwrap();
    let a = DecodeRequest::new(&left);
    let b = DecodeRequest::new(&right);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let region = AffineRegionPolicy {
        radius: 1,
        maximum_sites: 256,
        maximum_pixel_reads: 11520,
        color: AffineColorPolicy {
            minimum_samples: 100,
            maximum_samples: 256,
            minimum_variance: 1e-8,
            minimum_relative_pivot: 1e-8,
            maximum_coefficient: 5.,
            maximum_offset: 0.1,
        },
        tolerance: 0.001,
    };
    let policy = AffineRegionFilePolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 4096,
            max_file_bytes: 65536,
        },
        forward: region,
        reverse: AffineRegionPolicy {
            maximum_pixel_reads: 18432,
            ..region
        },
        maximum_total_pixel_reads: 29952,
    };
    let model = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    };
    let calls = std::cell::Cell::new(0);
    let result = verify_affine_region_files_with_footprints(
        &a,
        &b,
        model,
        [16, 16, 16, 16],
        [16, 16, 16, 16],
        policy,
        [AffineRegionFootprint::Target, AffineRegionFootprint::Source],
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for e in result {
        assert_eq!(e.heldout.matched, e.heldout.samples);
        assert_eq!(e.heldout.samples, 128);
    }
    assert_eq!(budget.used(), 0);
    let total = calls.get();
    assert!(total > 1000);
    for stop in [total / 2, total] {
        calls.set(0);
        let result = verify_affine_region_files_with_footprints(
            &a,
            &b,
            model,
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            policy,
            [AffineRegionFootprint::Target, AffineRegionFootprint::Source],
            &budget,
            || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        );
        assert!(matches!(result, Err(AffineRegionFileError::Cancelled)));
        assert_eq!(budget.used(), 0);
    }
    for replacement in [false, true] {
        std::fs::write(&right, &bytes).unwrap();
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = verify_affine_region_files_with_footprints(
            &a,
            &b,
            model,
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            policy,
            [AffineRegionFootprint::Target, AffineRegionFootprint::Source],
            &budget,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == total / 2 {
                    if replacement {
                        let path = dir.path().join("replacement.bmp");
                        std::fs::write(&path, &bytes).unwrap();
                        std::fs::rename(path, &right).unwrap();
                    } else {
                        std::fs::OpenOptions::new()
                            .append(true)
                            .open(&right)
                            .unwrap()
                            .write_all(&[0])
                            .unwrap();
                    }
                    changed.set(true);
                }
                false
            },
        );
        assert!(changed.get());
        assert!(
            matches!(
                result,
                Err(AffineRegionFileError::Source(
                    rrrah_dedup::exact::SnapshotError::Changed
                ))
            ),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn diagnostic_files_preserve_mutation_replacement_and_sticky_cancel() {
    use rrrah_dedup::affine_region::AffineRegionFootprint;
    use rrrah_dedup::affine_region_file::diagnose_affine_region_files_with_footprints;
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.bmp");
    let right = dir.path().join("right.bmp");
    let bytes = color_bmp();
    std::fs::write(&left, &bytes).unwrap();
    std::fs::write(&right, &bytes).unwrap();
    let a = DecodeRequest::new(&left);
    let b = DecodeRequest::new(&right);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let region = AffineRegionPolicy {
        radius: 1,
        maximum_sites: 256,
        maximum_pixel_reads: 11520,
        color: AffineColorPolicy {
            minimum_samples: 100,
            maximum_samples: 256,
            minimum_variance: 1e-8,
            minimum_relative_pivot: 1e-8,
            maximum_coefficient: 5.,
            maximum_offset: 0.1,
        },
        tolerance: 0.001,
    };
    let policy = AffineRegionFilePolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 4096,
            max_file_bytes: 65536,
        },
        forward: region,
        reverse: AffineRegionPolicy {
            maximum_pixel_reads: 18432,
            ..region
        },
        maximum_total_pixel_reads: 29952,
    };
    let model = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    };
    let calls = std::cell::Cell::new(0);
    let result = diagnose_affine_region_files_with_footprints(
        &a,
        &b,
        model,
        [16, 16, 16, 16],
        [16, 16, 16, 16],
        policy,
        [AffineRegionFootprint::Target, AffineRegionFootprint::Source],
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for diagnostic in result {
        assert_eq!(diagnostic.training.samples, 128);
        assert_eq!(diagnostic.training.matched, 128);
        let e = diagnostic.evidence;
        assert_eq!(e.heldout.matched, e.heldout.samples);
        assert_eq!(e.heldout.samples, 128);
    }
    assert_eq!(budget.used(), 0);
    let total = calls.get();
    assert!(total > 1000);
    for stop in [total / 2, total] {
        calls.set(0);
        let result = diagnose_affine_region_files_with_footprints(
            &a,
            &b,
            model,
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            policy,
            [AffineRegionFootprint::Target, AffineRegionFootprint::Source],
            &budget,
            || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        );
        assert!(matches!(result, Err(AffineRegionFileError::Cancelled)));
        assert_eq!(budget.used(), 0);
    }
    for replacement in [false, true] {
        std::fs::write(&right, &bytes).unwrap();
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = diagnose_affine_region_files_with_footprints(
            &a,
            &b,
            model,
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            policy,
            [AffineRegionFootprint::Target, AffineRegionFootprint::Source],
            &budget,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == total / 2 {
                    if replacement {
                        let path = dir.path().join("replacement.bmp");
                        std::fs::write(&path, &bytes).unwrap();
                        std::fs::rename(path, &right).unwrap();
                    } else {
                        std::fs::OpenOptions::new()
                            .append(true)
                            .open(&right)
                            .unwrap()
                            .write_all(&[0])
                            .unwrap();
                    }
                    changed.set(true);
                }
                false
            },
        );
        assert!(changed.get());
        assert!(
            matches!(
                result,
                Err(AffineRegionFileError::Source(
                    rrrah_dedup::exact::SnapshotError::Changed
                ))
            ),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
    }
}
