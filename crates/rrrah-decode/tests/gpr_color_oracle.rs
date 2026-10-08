//! External corpus regression against independent Adobe DNG SDK output.
use rrrah_core::{MemoryBudget, XYZ_WHITE_D65, bradford_adaptation_f64, invert_3x3_f64, multiply_3x3_f64};
use rrrah_decode::{DecodeRequest, NativeRawDecoder, RawDecoder};

#[test]
#[ignore = "requires the six pinned official GPR sources in RRRAH_GPR_CORPUS"]
fn six_production_gpr_transforms_match_independent_adobe_oracles() {
    let corpus = std::path::PathBuf::from(std::env::var_os("RRRAH_GPR_CORPUS").expect("RRRAH_GPR_CORPUS"));
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/gpr-color-matrices.json")).unwrap();
    let d50 = [0.3457 / 0.3585, 1.0, (1.0 - 0.3457 - 0.3585) / 0.3585];
    let mut pcs = [
        [0.4361, 0.3851, 0.1431],
        [0.2225, 0.7169, 0.0606],
        [0.0139, 0.0971, 0.7141],
    ];
    for r in 0..3 {
        let sum: f64 = pcs[r].iter().sum();
        for v in &mut pcs[r] {
            *v *= d50[r] / sum;
        }
    }
    let pcs_to_srgb = invert_3x3_f64(pcs).unwrap();
    for fixture in fixtures.as_array().unwrap() {
        let name = fixture["camera"].as_str().unwrap();
        let budget = MemoryBudget::new(2 * 1024 * 1024 * 1024);
        let mut request = DecodeRequest::new(corpus.join(format!("{name}.GPR")));
        request.memory_budget = Some(budget.clone());
        let decoded = NativeRawDecoder.decode(&request).unwrap();
        let metadata = &decoded.mosaic.metadata;
        let xyz = metadata.xyz_to_camera.map(|r| r.map(f64::from));
        let matrix = multiply_3x3_f64(
            [xyz[0], xyz[1], xyz[2]],
            bradford_adaptation_f64(d50, XYZ_WHITE_D65).unwrap(),
        );
        let camera_white: [f64; 3] = std::array::from_fn(|r| (0..3).map(|c| matrix[r][c] * d50[c]).sum());
        let scale = camera_white.into_iter().fold(0.0_f64, f64::max);
        let camera_to_pcs = invert_3x3_f64(matrix.map(|r| r.map(|v| v / scale))).unwrap();
        let actual = multiply_3x3_f64(pcs_to_srgb, camera_to_pcs);
        let mut maximum = 0.0_f64;
        for r in 0..3 {
            let neutral = fixture["independent_as_shot_neutral"][r].as_f64().unwrap();
            assert!(
                (f64::from(metadata.white_balance[r]) * neutral - 1.0).abs() < 1e-6,
                "{name} WB channel {r}"
            );
            for c in 0..3 {
                maximum =
                    maximum.max((actual[r][c] - fixture["independent_matrix"][r][c].as_f64().unwrap()).abs());
            }
        }
        assert!(maximum < 1e-6, "{name} independent matrix error {maximum}");
        drop(decoded);
        assert_eq!(budget.used(), 0, "{name} retained managed sensor memory");
        eprintln!("{name}: independent matrix max error {maximum}; WB and managed release passed");
    }
}
