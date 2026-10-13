use rrrah_dedup::{exposure::{measure_rank_exposure, ExposureError}, linear::LinearRgbaView};
use std::cell::Cell;

#[test]
fn calibrated_rank_recovers_uniform_darkening_and_rejects_inversion() {
    use rrrah_dedup::{exposure::calibrate_rank_exposure_into, geometry::ProjectiveTransform,
        rank_region::{compare_rank_region, RankRegionPolicy, RankRegionError}};
    let original: Vec<f32> = (0..1024).flat_map(|i| {
        let v = 0.2 + ((i % 32 * 13 + i / 32 * 7) % 29) as f32 / 50.0;
        [v, v, v, 1.0]
    }).collect();
    let source = LinearRgbaView::new(32, 32, &original, 1024, || false).unwrap();
    let policy = RankRegionPolicy { radius: 2, minimum_contrast: 0.005,
        minimum_pairs: 1000, maximum_sites: 256, maximum_pixel_reads: 11520 };
    let model = ProjectiveTransform { matrix: [[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]] };
    for inverted in [false, true] {
        let dark: Vec<f32> = original.chunks_exact(4).flat_map(|p| {
            let v = if inverted {1.0 - p[0]} else {p[0]} * 0.0001;
            [v,v,v,1.0]
        }).collect();
        let target = LinearRgbaView::new(32,32,&dark,1024,||false).unwrap();
        assert_eq!(compare_rank_region(&source,&target,model,[0,0,32,32],[8,8,16,16],policy,||false),Err(RankRegionError::Uninformative));
        let mut a = vec![0.0;4096]; let mut b = vec![0.0;4096];
        let (source, _) = calibrate_rank_exposure_into(&source,&mut a,3072,||false).unwrap();
        let (target, _) = calibrate_rank_exposure_into(&target,&mut b,3072,||false).unwrap();
        let e = compare_rank_region(&source,&target,model,[0,0,32,32],[8,8,16,16],policy,||false).unwrap();
        assert_eq!(e.agreeing_pairs,if inverted {0} else {e.informative_pairs});
    }
}

#[test]
fn calibration_scratch_and_all_cancellation_boundaries_are_guarded() {
    use rrrah_dedup::exposure::calibrate_rank_exposure_into;
    let pixels = [2.0,1.0,0.5,1.0];
    let image = LinearRgbaView::new(1,1,&pixels,1,||false).unwrap();
    assert_eq!(calibrate_rank_exposure_into(&image,&mut [],3,||false).unwrap_err(),ExposureError::Layout);
    let mut scratch = [9.0;4];
    assert_eq!(calibrate_rank_exposure_into(&image,&mut scratch,2,||false).unwrap_err(),ExposureError::Budget);
    assert_eq!(scratch,[9.0;4]);
    let calls = Cell::new(0);
    let (_,e) = calibrate_rank_exposure_into(&image,&mut scratch,3,||{calls.set(calls.get()+1);false}).unwrap();
    assert_eq!(e.pixel_reads,3);
    for stop in 1..=calls.get() {
        let n=Cell::new(0);
        assert_eq!(calibrate_rank_exposure_into(&image,&mut scratch,3,||{n.set(n.get()+1);n.get()==stop}).unwrap_err(),ExposureError::Cancelled);
    }
}

#[test]
fn uniform_exposure_calibrates_dark_and_hdr_without_clipping() {
    for scale in [0.0001_f32, 1.0, 16.0, f32::MIN_POSITIVE] {
        let pixels = [scale / 4.0, scale / 2.0, scale, 1.0];
        let image = LinearRgbaView::new(1, 1, &pixels, 1, || false).unwrap();
        let measured = measure_rank_exposure(&image, 1, || false).unwrap();
        assert_eq!(measured.peak_rgb, f64::from(scale));
        assert_eq!(measured.pixel_reads, 1);
        assert!((measured.gain * f64::from(scale) - 1.0).abs() < 1e-15);
        assert_eq!(image.rgba(0, 0).unwrap(), pixels);
    }
}

#[test]
fn zero_negative_and_transparent_inputs_refuse_calibration() {
    for (pixels, expected) in [
        ([0.0, 0.0, 0.0, 1.0], ExposureError::Uninformative),
        ([-0.1, 0.5, 1.0, 1.0], ExposureError::UnsupportedPixels),
        ([0.1, 0.5, 1.0, 0.5], ExposureError::UnsupportedPixels),
        ([f32::NAN, 0.0, 0.0, 0.0], ExposureError::UnsupportedPixels),
    ] {
        let image = LinearRgbaView::new(1, 1, &pixels, 1, || false).unwrap();
        assert_eq!(measure_rank_exposure(&image, 1, || false), Err(expected));
    }
}

#[test]
fn budget_precedes_reads_and_late_cancellation_discards_measurement() {
    let pixels = [-1.0, 0.0, 0.0, 1.0];
    let image = LinearRgbaView::new(1, 1, &pixels, 1, || false).unwrap();
    assert_eq!(measure_rank_exposure(&image, 0, || false), Err(ExposureError::Budget));
    assert_eq!(measure_rank_exposure(&image, 0, || true), Err(ExposureError::Cancelled));
    let pixels = [1.0; 8];
    let image = LinearRgbaView::new(2, 1, &pixels, 2, || false).unwrap();
    for stop in 1..=4 {
        let calls = Cell::new(0);
        assert_eq!(measure_rank_exposure(&image, 2, || {
            calls.set(calls.get() + 1); calls.get() == stop
        }), Err(ExposureError::Cancelled));
    }
}
