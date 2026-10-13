#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::{FingerprintPolicy, fingerprint_file},
};

fn no_cancel() -> bool {
    false
}

#[test]
fn pfm_storage_order_cannot_reuse_the_opposite_interpretation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("rows.pfm");
    let mut bytes = b"PF\n2 2\n-1.0\n".to_vec();
    for pixel in [[1.0_f32, 0., 0.], [0., 1., 0.], [0., 0., 1.], [1., 1., 1.]] {
        for channel in pixel {
            bytes.extend_from_slice(&channel.to_le_bytes());
        }
    }
    std::fs::write(&path, bytes).unwrap();
    let mut request = DecodeRequest::new(path);
    request.assume_untagged_linear_srgb = true;
    let policy = FingerprintPolicy {
        recipe: [28; 32],
        max_file_bytes: 1024,
        max_pixels: 4,
        max_frames: 1,
        max_cache_entries: 10,
    };
    let budget = MemoryBudget::new(1024 * 1024);
    let mut cache = FingerprintCache::default();
    let bottom = fingerprint_file(&request, policy, &budget, &mut cache, no_cancel).unwrap();
    assert!(!bottom.cache_hit);
    request.pfm_top_down = true;
    let top = fingerprint_file(&request, policy, &budget, &mut cache, no_cancel).unwrap();
    let fresh = fingerprint_file(
        &request,
        policy,
        &budget,
        &mut FingerprintCache::default(),
        no_cancel,
    )
    .unwrap();
    assert!(!top.cache_hit, "storage order is part of pixel interpretation");
    assert_eq!(top.fingerprint, fresh.fingerprint);
    assert_ne!(bottom.fingerprint, fresh.fingerprint);
    assert!(
        fingerprint_file(&request, policy, &budget, &mut cache, no_cancel)
            .unwrap()
            .cache_hit
    );
    request.pfm_top_down = false;
    let restored = fingerprint_file(&request, policy, &budget, &mut cache, no_cancel).unwrap();
    assert!(restored.cache_hit);
    assert_eq!(restored.fingerprint, bottom.fingerprint);
    assert_eq!(budget.used(), 0);
}

// BMP ignores these format-specific settings. This checks cache separation,
// while the PFM test above checks a setting that actually changes decoded pixels.
#[test]
fn caller_interpretations_have_separate_repeatable_cache_entries() {
    use rrrah_decode::{RlaAlphaMode, RlaColorSpace, RlaFloatByteOrder, RpfRasterInterpretation};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("colors.bmp");
    let mut bytes = vec![0; 62];
    bytes[..2].copy_from_slice(b"BM");
    for (offset, value) in [(2, 62_u32), (10, 54), (14, 40), (18, 2), (22, 1), (34, 8)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24_u16.to_le_bytes());
    bytes[57..60].fill(255);
    std::fs::write(&path, bytes).unwrap();
    let base = DecodeRequest::new(path);
    let mut requests = vec![base.clone()];
    macro_rules! variation {
        ($field:ident, $value:expr) => {{
            let mut request = base.clone();
            request.$field = $value;
            requests.push(request);
        }};
    }
    variation!(rla_alpha_mode, Some(RlaAlphaMode::Straight));
    variation!(rla_alpha_mode, Some(RlaAlphaMode::Premultiplied));
    variation!(rla_float_byte_order, Some(RlaFloatByteOrder::Little));
    variation!(rla_float_byte_order, Some(RlaFloatByteOrder::Big));
    variation!(rla_color_space, Some(RlaColorSpace::Srgb));
    variation!(rla_color_space, Some(RlaColorSpace::LinearSrgb));
    variation!(pq_reference_white_nits, Some(100.));
    variation!(pq_reference_white_nits, Some(203.));
    variation!(hlg_display, Some((100., 1000., 1.2)));
    variation!(hlg_display, Some((203., 1000., 1.2)));
    variation!(hlg_display, Some((100., 2000., 1.2)));
    variation!(hlg_display, Some((100., 1000., 1.3)));
    variation!(assume_untagged_srgb, true);
    variation!(assume_untagged_linear_srgb, true);
    variation!(pfm_top_down, true);
    for (rgb, matte, alpha_mode, color_space) in [
        (
            [0, 1, 2],
            None,
            RlaAlphaMode::Straight,
            rrrah_core::RasterColorSpace::Srgb,
        ),
        (
            [2, 1, 0],
            None,
            RlaAlphaMode::Straight,
            rrrah_core::RasterColorSpace::Srgb,
        ),
        (
            [0, 1, 2],
            Some(3),
            RlaAlphaMode::Straight,
            rrrah_core::RasterColorSpace::Srgb,
        ),
        (
            [0, 1, 2],
            None,
            RlaAlphaMode::Premultiplied,
            rrrah_core::RasterColorSpace::Srgb,
        ),
        (
            [0, 1, 2],
            None,
            RlaAlphaMode::Straight,
            rrrah_core::RasterColorSpace::LinearSrgb,
        ),
    ] {
        variation!(
            rpf_interpretation,
            Some(RpfRasterInterpretation {
                rgb,
                matte,
                alpha_mode,
                color_space
            })
        );
    }
    let policy = FingerprintPolicy {
        recipe: [29; 32],
        max_file_bytes: 1024,
        max_pixels: 2,
        max_frames: 1,
        max_cache_entries: 100,
    };
    let budget = MemoryBudget::new(1024 * 1024);
    let mut cache = FingerprintCache::default();
    for (index, request) in requests.iter().enumerate() {
        let first = fingerprint_file(request, policy, &budget, &mut cache, no_cancel).unwrap();
        assert!(!first.cache_hit, "interpretation {index} reused another entry");
        let repeat = fingerprint_file(request, policy, &budget, &mut cache, no_cancel).unwrap();
        assert!(repeat.cache_hit);
        assert_eq!(first.fingerprint, repeat.fingerprint);
    }
    assert_eq!(budget.used(), 0);
}
