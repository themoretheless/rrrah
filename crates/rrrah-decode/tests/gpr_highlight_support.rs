//! Real sensor oracle bounds the area affected by highlight reconstruction.
use rrrah_core::{MemoryBudget, Orientation, RasterPixels};
use rrrah_decode::{DecodeRequest, NativeRawDecoder, RawDecoder};

#[test]
#[ignore = "requires pinned HERO9 GPR and independent GoPro SDK sensor.u16le"]
fn hero9_recovery_changes_only_sensor_saturation_neighbourhood_and_preserves_hdr() {
    let source = std::env::var_os("RRRAH_GPR_SOURCE").expect("RRRAH_GPR_SOURCE");
    let oracle =
        std::fs::read(std::env::var_os("RRRAH_GPR_SENSOR_ORACLE").expect("RRRAH_GPR_SENSOR_ORACLE")).unwrap();
    let budget = MemoryBudget::new(2 * 1024 * 1024 * 1024);
    let mut request = DecodeRequest::new(source);
    request.memory_budget = Some(budget.clone());
    let mosaic = NativeRawDecoder.decode(&request).unwrap().mosaic;
    let metadata = &mosaic.metadata;
    assert_eq!(metadata.orientation, Orientation::Normal);
    let (w, h) = (metadata.width as usize, metadata.height as usize);
    assert_eq!((w, h), (5568, 4176));
    assert_eq!(oracle.len(), w * h * 2);
    let mut support = vec![false; w * h];
    let mut saturated = 0;
    for (i, bytes) in oracle.chunks_exact(2).enumerate() {
        let sensor = u16::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(mosaic.pixels[i], sensor, "independent sensor {i}");
        if f64::from(sensor) >= 16383.0 * 0.999 {
            saturated += 1;
            let (x, y) = (i % w, i / w);
            for sy in y.saturating_sub(2)..=(y + 2).min(h - 1) {
                for sx in x.saturating_sub(2)..=(x + 2).min(w - 1) {
                    support[sy * w + sx] = true;
                }
            }
        }
    }
    let lists = rrrah_decode::raw_development_opcodes(&request).unwrap();
    let enabled =
        rrrah_core::develop::develop_raw(&mosaic, &Default::default(), &lists, Some(&budget), &|| false)
            .unwrap();
    let options = rrrah_core::develop::DevelopOptions {
        recover_highlights: false,
        ..Default::default()
    };
    let disabled =
        rrrah_core::develop::develop_raw(&mosaic, &options, &lists, Some(&budget), &|| false).unwrap();
    assert_eq!((enabled.width(), enabled.height()), (w as u32, h as u32));
    let (RasterPixels::Rgba32Float(on), RasterPixels::Rgba32Float(off)) =
        (enabled.pixels(), disabled.pixels())
    else {
        panic!("scene-linear output required")
    };
    let (mut changed, mut hdr) = (0, 0);
    for (i, (a, b)) in on.chunks_exact(4).zip(off.chunks_exact(4)).enumerate() {
        assert!(a.iter().chain(b).all(|v| v.is_finite()));
        assert_eq!(a[3], 1.0);
        assert_eq!(b[3], 1.0);
        if a.iter().zip(b).any(|(a, b)| a.to_bits() != b.to_bits()) {
            changed += 1;
            assert!(support[i], "recovery changed unsaturated neighbourhood pixel {i}");
        }
        hdr += usize::from(a[..3].iter().any(|v| *v > 1.0));
    }
    assert!(saturated > 0 && changed > 0 && hdr > 0);
    drop(enabled);
    drop(disabled);
    drop(mosaic);
    assert_eq!(budget.used(), 0);
    eprintln!(
        "HERO9: {saturated} independent saturated photosites, {changed} changed pixels within support, {hdr} HDR pixels; managed memory released"
    );
}
