//! Inspect production RAW transforms without saving sensor pixels.
use rrrah_decode::RawDecoder;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for source in std::env::args().skip(1) {
        let budget = rrrah_core::MemoryBudget::new(2 * 1024 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(&source);
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request)?;
        let metadata = &decoded.mosaic.metadata;
        let xyz = metadata.xyz_to_camera.map(|row| row.map(f64::from));
        let transform = rrrah_core::camera_to_linear_srgb_precise(xyz)?;
        let combined = std::array::from_fn::<_, 3, _>(|r| {
            std::array::from_fn::<_, 3, _>(|c| transform[r][c] * f64::from(metadata.white_balance[c]))
        });
        // Compare in the independent oracle's rounded, white-normalized PCS
        // sRGB space as well as the precise native D65 sRGB output space.
        let d50 = [0.3457 / 0.3585, 1.0, (1.0 - 0.3457 - 0.3585) / 0.3585];
        let adaptation = rrrah_core::bradford_adaptation_f64(d50, rrrah_core::XYZ_WHITE_D65).unwrap();
        let mut camera = rrrah_core::multiply_3x3_f64([xyz[0], xyz[1], xyz[2]], adaptation);
        let white: [f64; 3] = std::array::from_fn(|r| (0..3).map(|c| camera[r][c] * d50[c]).sum());
        let scale = white.into_iter().fold(0.0_f64, f64::max);
        for row in &mut camera {
            for value in row {
                *value /= scale;
            }
        }
        let mut pcs = [
            [0.4361, 0.3851, 0.1431],
            [0.2225, 0.7169, 0.0606],
            [0.0139, 0.0971, 0.7141],
        ];
        for r in 0..3 {
            let sum: f64 = pcs[r].iter().sum();
            for value in &mut pcs[r] {
                *value *= d50[r] / sum;
            }
        }
        let oracle_space = rrrah_core::multiply_3x3_f64(
            rrrah_core::invert_3x3_f64(pcs).unwrap(),
            rrrah_core::invert_3x3_f64(camera).unwrap(),
        );
        let result = serde_json::json!({"source":source,"make":metadata.make,"model":metadata.model,"dimensions":[metadata.width,metadata.height],"white_balance":metadata.white_balance,"xyz_to_camera":metadata.xyz_to_camera,"native_combined":combined,"adobe_space_combined":oracle_space,"managed_peak_bytes":budget.peak()});
        drop(decoded);
        assert_eq!(budget.used(), 0);
        println!("{result}");
    }
    Ok(())
}
