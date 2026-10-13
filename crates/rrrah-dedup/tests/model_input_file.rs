#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::{DecodeRequest, GenerationToken};
use rrrah_dedup::{
    animated::AnimationBudget,
    exact::SnapshotError,
    model_input::ModelInputPolicy,
    model_input_file::{prepare_gray_model_file, ModelFileError},
};
use std::{
    cell::Cell,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
const LIMITS: AnimationBudget = AnimationBudget {
    max_frames: 1,
    max_pixels: 256,
    max_file_bytes: 4096,
};
const POLICY: ModelInputPolicy = ModelInputPolicy {
    max_side: 16,
    max_source_pixels: 256,
    max_output_pixels: 256,
    max_pixel_reads: 1024,
};
fn bmp() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(b"BM");
    b.extend((54u32 + 16 * 16 * 3).to_le_bytes());
    b.extend([0; 4]);
    b.extend(54u32.to_le_bytes());
    b.extend(40u32.to_le_bytes());
    b.extend(16i32.to_le_bytes());
    b.extend(16i32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(24u16.to_le_bytes());
    b.extend([0; 24]);
    for y in 0..16u8 {
        for x in 0..16u8 {
            b.extend([x * 15, y * 15, (x ^ y) * 15]);
        }
    }
    b
}

#[test]
fn file_crop_retains_full_source_identity_and_original_dimensions() {
    use rrrah_dedup::model_input_file::prepare_gray_model_region_file;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crop.bmp");
    std::fs::write(&path, bmp()).unwrap();
    let generation = Arc::new(AtomicU64::new(1));
    let mut request = DecodeRequest::new(&path);
    request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
    let budget = MemoryBudget::new(1024 * 1024);
    let input =
        prepare_gray_model_region_file(&request, [3, 4, 11, 9], LIMITS, POLICY, &budget, || false).unwrap();
    assert_eq!(input.original_dimensions(), [16, 16]);
    assert_eq!(input.rectangle(), [3, 4, 11, 9]);
    assert_eq!(input.input().original, [11, 9]);
    assert_eq!(input.input().dimensions, [8, 8]);
    assert_eq!(budget.used(), 64);
    input.verify_source(|| false).unwrap();
    let mut changed = bmp();
    // Alter an original pixel outside the crop; whole-source protection remains.
    changed[54] ^= 255;
    std::fs::write(&path, changed).unwrap();
    assert!(matches!(
        input.verify_source(|| false),
        Err(ModelFileError::Source(SnapshotError::Changed))
    ));
    std::fs::write(&path, bmp()).unwrap();
    generation.store(2, Ordering::Release);
    assert!(matches!(
        input.verify_source(|| false),
        Err(ModelFileError::Cancelled)
    ));
    drop(input);
    assert_eq!(budget.used(), 0);
    for rectangle in [[0, 0, 0, 1], [u32::MAX, 0, 2, 1]] {
        assert!(matches!(
            prepare_gray_model_region_file(
                &DecodeRequest::new("missing"),
                rectangle,
                LIMITS,
                POLICY,
                &budget,
                || false
            ),
            Err(ModelFileError::Input(_))
        ));
    }
}
#[test]
fn file_input_retains_only_output_and_rechecks_source_and_generation_after_inference() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.bmp");
    std::fs::write(&path, bmp()).unwrap();
    let generation = Arc::new(AtomicU64::new(1));
    let mut request = DecodeRequest::new(&path);
    request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
    let budget = MemoryBudget::new(1024 * 1024);
    let prepared = prepare_gray_model_file(&request, LIMITS, POLICY, &budget, || false).unwrap();
    assert_eq!(prepared.input().dimensions, [16, 16]);
    assert_eq!(budget.used(), 256);
    prepared.verify_source(|| false).unwrap();
    generation.store(2, Ordering::Release);
    assert!(matches!(
        prepared.verify_source(|| false),
        Err(ModelFileError::Cancelled)
    ));
    generation.store(1, Ordering::Release);
    let mut changed = bmp();
    changed.push(0);
    std::fs::write(&path, changed).unwrap();
    assert!(matches!(
        prepared.verify_source(|| false),
        Err(ModelFileError::Source(SnapshotError::Changed))
    ));
    drop(prepared);
    assert_eq!(budget.used(), 0);
}
#[test]
fn every_global_checkpoint_and_mid_final_generation_cancellation_discard_payload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.bmp");
    std::fs::write(&path, bmp()).unwrap();
    let request = DecodeRequest::new(&path);
    let budget = MemoryBudget::new(1024 * 1024);
    let calls = Cell::new(0);
    drop(
        prepare_gray_model_file(&request, LIMITS, POLICY, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    assert_eq!(budget.used(), 0);
    for at in 1..=calls.get() {
        let current = Cell::new(0);
        assert!(
            matches!(
                prepare_gray_model_file(&request, LIMITS, POLICY, &budget, || {
                    current.set(current.get() + 1);
                    current.get() == at
                }),
                Err(ModelFileError::Cancelled)
            ),
            "checkpoint {at}"
        );
        assert_eq!(current.get(), at);
        assert_eq!(budget.used(), 0);
    }
    for at in [0, calls.get() / 2, calls.get()] {
        let generation = Arc::new(AtomicU64::new(if at == 0 { 2 } else { 1 }));
        let mut request = request.clone();
        request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let current = Cell::new(0);
        assert!(matches!(
            prepare_gray_model_file(&request, LIMITS, POLICY, &budget, || {
                current.set(current.get() + 1);
                if current.get() == at {
                    generation.store(2, Ordering::Release);
                }
                false
            }),
            Err(ModelFileError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn invalid_policy_precedes_io_and_mid_preparation_source_changes_are_errors() {
    let budget = MemoryBudget::new(1024 * 1024);
    let missing = DecodeRequest::new("/nonexistent/rrrah-model-input.bmp");
    assert!(matches!(
        prepare_gray_model_file(
            &missing,
            LIMITS,
            ModelInputPolicy {
                max_side: 7,
                ..POLICY
            },
            &budget,
            || false
        ),
        Err(ModelFileError::Policy)
    ));
    assert_eq!(budget.peak(), 0);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.bmp");
    std::fs::write(&path, bmp()).unwrap();
    let request = DecodeRequest::new(&path);
    let calls = Cell::new(0);
    drop(
        prepare_gray_model_file(&request, LIMITS, POLICY, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    let current = Cell::new(0);
    let at = calls.get() / 2;
    let result = prepare_gray_model_file(&request, LIMITS, POLICY, &budget, || {
        current.set(current.get() + 1);
        if current.get() == at {
            let mut changed = bmp();
            changed.push(0);
            std::fs::write(&path, changed).unwrap();
        }
        false
    });
    assert!(
        matches!(result, Err(ModelFileError::Source(SnapshotError::Changed))),
        "{result:?}"
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn external_palette_is_rechecked_after_preparation_and_mutation_discards_input() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("textures")).unwrap();
    std::fs::create_dir(dir.path().join("pics")).unwrap();
    let path = dir.path().join("textures/source.wal");
    let palette = dir.path().join("pics/colormap.pcx");
    // This input recipe is opaque SDR. WAL index 255 declares transparency;
    // replace only mip pixel indices so this fixture exercises dependencies.
    let mut wal = include_bytes!("../../../tests/fixtures/raster/palette-grid.wal").to_vec();
    for index in &mut wal[100..] {
        if *index == 255 {
            *index = 254;
        }
    }
    std::fs::write(&path, wal).unwrap();
    let bytes = include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx");
    std::fs::write(&palette, bytes).unwrap();
    let request = DecodeRequest::new(&path);
    let limits = AnimationBudget {
        max_frames: 1,
        max_pixels: 30000,
        max_file_bytes: 1024 * 1024,
    };
    let policy = ModelInputPolicy {
        max_source_pixels: 30000,
        max_pixel_reads: 120000,
        ..POLICY
    };
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let input = prepare_gray_model_file(&request, limits, policy, &budget, || false).unwrap();
    input.verify_source(|| false).unwrap();
    let mut changed = bytes.to_vec();
    let last = changed.len() - 1;
    changed[last] ^= 0xff;
    std::fs::write(&palette, &changed).unwrap();
    assert!(matches!(
        input.verify_source(|| false),
        Err(ModelFileError::Source(SnapshotError::Changed))
    ));
    drop(input);
    assert_eq!(budget.used(), 0);
    std::fs::write(&palette, bytes).unwrap();
    let did_change = Cell::new(false);
    let result = prepare_gray_model_file(&request, limits, policy, &budget, || {
        if budget.used() > 0 && !did_change.replace(true) {
            std::fs::write(&palette, &changed).unwrap();
        }
        false
    });
    assert!(did_change.get());
    assert!(
        matches!(result, Err(ModelFileError::Source(SnapshotError::Changed))),
        "{result:?}"
    );
    assert_eq!(budget.used(), 0);
    std::fs::write(&palette, bytes).unwrap();
    drop(prepare_gray_model_file(&request, limits, policy, &budget, || false).unwrap());
    assert_eq!(budget.used(), 0);
}

#[test]
fn projected_hdr_file_retains_recipe_and_rejects_equal_length_mutation() {
    use rrrah_dedup::{
        model_input::ModelInputError, model_input_file::prepare_gray_model_file_with_projection,
        model_projection::ModelInputProjection,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hdr.pfm");
    let bytes = |value: f32| {
        let mut data = b"PF\n8 8\n-1.0\n".to_vec();
        for _ in 0..192 {
            data.extend(value.to_le_bytes());
        }
        data
    };
    std::fs::write(&path, bytes(4.)).unwrap();
    let mut request = DecodeRequest::new(&path);
    request.assume_untagged_linear_srgb = true;
    let budget = MemoryBudget::new(32 * 1024);
    assert!(matches!(
        prepare_gray_model_file(&request, LIMITS, POLICY, &budget, || false),
        Err(ModelFileError::Input(ModelInputError::UnsupportedPixels))
    ));
    assert_eq!(budget.used(), 0);
    let projection = ModelInputProjection::CompositeReinhardV1 {
        exposure: 1.,
        background: [0.; 3],
    };
    let prepared =
        prepare_gray_model_file_with_projection(&request, LIMITS, POLICY, projection, &budget, || false)
            .unwrap();
    assert_eq!(prepared.projection(), projection);
    assert_eq!(prepared.original_dimensions(), [8, 8]);
    assert_eq!(prepared.input().dimensions, [8, 8]);
    assert!(prepared.input().pixels.iter().all(|v| *v == 231));
    prepared.verify_source(|| false).unwrap();
    std::fs::write(&path, bytes(4.01)).unwrap();
    assert!(matches!(
        prepared.verify_source(|| false),
        Err(ModelFileError::Source(SnapshotError::Changed))
    ));
    drop(prepared);
    assert_eq!(budget.used(), 0);
}

#[test]
fn projected_file_invalid_configuration_is_refused_before_source_io() {
    use rrrah_dedup::{
        model_input::ModelInputError, model_input_file::prepare_gray_model_file_with_projection,
        model_projection::ModelInputProjection,
    };
    let dir = tempfile::tempdir().unwrap();
    let request = DecodeRequest::new(dir.path().join("missing.pfm"));
    let budget = MemoryBudget::new(32 * 1024);
    let projection = ModelInputProjection::CompositeReinhardV1 {
        exposure: 1.,
        background: [f64::NAN, 0., 0.],
    };
    assert!(matches!(
        prepare_gray_model_file_with_projection(&request, LIMITS, POLICY, projection, &budget, || false),
        Err(ModelFileError::Input(ModelInputError::Invalid))
    ));
    assert_eq!((budget.used(), budget.peak()), (0, 0));
    assert!(matches!(
        prepare_gray_model_file_with_projection(&request, LIMITS, POLICY, projection, &budget, || true),
        Err(ModelFileError::Cancelled)
    ));
    assert_eq!((budget.used(), budget.peak()), (0, 0));
}

#[test]
fn projected_crop_reads_original_rectangle_and_guards_mutation_outside_it() {
    use rrrah_dedup::{
        model_input_file::prepare_gray_model_region_file_with_projection,
        model_projection::ModelInputProjection,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crop-hdr.pfm");
    let bytes = |outside: f32| {
        let mut data = b"PF\n17 11\n-1.0\n".to_vec();
        for y in (0..11).rev() {
            for x in 0..17 {
                let value = if (3..14).contains(&x) && (2..9).contains(&y) {
                    4_f32
                } else {
                    outside
                };
                for _ in 0..3 {
                    data.extend(value.to_le_bytes());
                }
            }
        }
        data
    };
    std::fs::write(&path, bytes(1000.)).unwrap();
    let mut request = DecodeRequest::new(&path);
    request.assume_untagged_linear_srgb = true;
    let budget = MemoryBudget::new(32 * 1024);
    let projection = ModelInputProjection::CompositeReinhardV1 {
        exposure: 1.,
        background: [0.; 3],
    };
    let prepared = prepare_gray_model_region_file_with_projection(
        &request,
        [3, 2, 11, 7],
        LIMITS,
        POLICY,
        projection,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(prepared.original_dimensions(), [17, 11]);
    assert_eq!(prepared.rectangle(), [3, 2, 11, 7]);
    assert_eq!(prepared.input().original, [11, 7]);
    assert_eq!(prepared.input().dimensions, [8, 8]);
    assert!(prepared.input().pixels.iter().all(|v| *v == 231));
    prepared.verify_source(|| false).unwrap();
    std::fs::write(&path, bytes(1001.)).unwrap();
    assert!(matches!(
        prepared.verify_source(|| false),
        Err(ModelFileError::Source(SnapshotError::Changed))
    ));
    drop(prepared);
    assert_eq!(budget.used(), 0);
}

fn late_callback_fixture(
    dir: &std::path::Path,
    dependency: bool,
) -> (
    DecodeRequest,
    std::path::PathBuf,
    AnimationBudget,
    ModelInputPolicy,
) {
    if !dependency {
        let path = dir.join("source.bmp");
        std::fs::write(&path, bmp()).unwrap();
        return (DecodeRequest::new(&path), path, LIMITS, POLICY);
    }
    std::fs::create_dir(dir.join("textures")).unwrap();
    std::fs::create_dir(dir.join("pics")).unwrap();
    let path = dir.join("textures/source.wal");
    let palette = dir.join("pics/colormap.pcx");
    let mut wal = include_bytes!("../../../tests/fixtures/raster/palette-grid.wal").to_vec();
    for index in &mut wal[100..] {
        if *index == 255 {
            *index = 254;
        }
    }
    std::fs::write(&path, wal).unwrap();
    std::fs::write(
        &palette,
        include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx"),
    )
    .unwrap();
    (
        DecodeRequest::new(path),
        palette,
        AnimationBudget {
            max_frames: 1,
            max_pixels: 30000,
            max_file_bytes: 1024 * 1024,
        },
        ModelInputPolicy {
            max_source_pixels: 30000,
            max_pixel_reads: 120000,
            ..POLICY
        },
    )
}

#[test]
fn final_preparation_callback_cannot_publish_mutated_source_or_dependency() {
    use rrrah_dedup::{model_input_file::*, model_projection::ModelInputProjection};
    for dependency in [false, true] {
        for variant in 0..4 {
            let dir = tempfile::tempdir().unwrap();
            let (request, path, limits, policy) = late_callback_fixture(dir.path(), dependency);
            let bytes = std::fs::read(&path).unwrap();
            let budget = MemoryBudget::new(8 * 1024 * 1024);
            let projection = ModelInputProjection::CompositeReinhardV1 {
                exposure: 1.,
                background: [0.; 3],
            };
            let run = |cancel: &dyn Fn() -> bool| match variant {
                0 => prepare_gray_model_file(&request, limits, policy, &budget, cancel),
                1 => prepare_gray_model_region_file(&request, [2, 2, 8, 8], limits, policy, &budget, cancel),
                2 => prepare_gray_model_file_with_projection(
                    &request, limits, policy, projection, &budget, cancel,
                ),
                _ => prepare_gray_model_region_file_with_projection(
                    &request,
                    [2, 2, 8, 8],
                    limits,
                    policy,
                    projection,
                    &budget,
                    cancel,
                ),
            };
            let count = Cell::new(0usize);
            drop(
                run(&|| {
                    count.set(count.get() + 1);
                    false
                })
                .unwrap(),
            );
            assert_eq!(budget.used(), 0);
            let last = count.replace(0);
            let result = run(&|| {
                count.set(count.get() + 1);
                if count.get() == last {
                    let mut changed = bytes.clone();
                    changed.push(0);
                    std::fs::write(&path, changed).unwrap();
                }
                false
            });
            assert!(
                matches!(result, Err(ModelFileError::Source(_))),
                "dependency={dependency}, variant={variant}: stale preparation {result:?}"
            );
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn final_source_verification_callback_cannot_hide_source_or_dependency_mutation() {
    for dependency in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (request, path, limits, policy) = late_callback_fixture(dir.path(), dependency);
        let bytes = std::fs::read(&path).unwrap();
        let budget = MemoryBudget::new(8 * 1024 * 1024);
        let prepared = prepare_gray_model_file(&request, limits, policy, &budget, || false).unwrap();
        let retained = budget.used();
        let count = Cell::new(0usize);
        prepared
            .verify_source(|| {
                count.set(count.get() + 1);
                false
            })
            .unwrap();
        let last = count.replace(0);
        let result = prepared.verify_source(|| {
            count.set(count.get() + 1);
            if count.get() == last {
                let mut changed = bytes.clone();
                changed.push(0);
                std::fs::write(&path, changed).unwrap();
            }
            false
        });
        assert!(
            matches!(result, Err(ModelFileError::Source(_))),
            "dependency={dependency}: stale verification {result:?}"
        );
        assert_eq!(budget.used(), retained);
        drop(prepared);
        assert_eq!(budget.used(), 0);
    }
}
