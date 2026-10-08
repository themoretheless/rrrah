use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::ProjectiveTransform,
    warp::{
        PhotometricPolicy, ProjectiveRegistrationPolicy, WarpPolicy, refine_projective_pixels_photometric,
    },
};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3);
    let text = std::fs::read_to_string(&args[2]).unwrap();
    let lines: Vec<_> = text.lines().collect();
    assert!(!lines.is_empty() && lines.len() <= 8);
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let decode = AnimationBudget {
        max_frames: 1,
        max_pixels: 6400000,
        max_file_bytes: 5242880,
    };
    let a = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(&args[0]),
        decode,
        &budget,
        || false,
    )
    .unwrap();
    let b = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(&args[1]),
        decode,
        &budget,
        || false,
    )
    .unwrap();
    let av = a.view(|| false).unwrap();
    let bv = b.view(|| false).unwrap();
    let mut rows = Vec::new();
    let fit = PhotometricPolicy {
        residual: WarpPolicy {
            tolerance: 0.03,
            max_source_pixels: 12800000,
        },
        minimum_samples: 16,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.,
        maximum_offset: 0.1,
    };
    for line in lines {
        let v: Vec<f64> = line.split_whitespace().map(|v| v.parse().unwrap()).collect();
        assert_eq!(v.len(), 9);
        assert!(v.iter().all(|v| v.is_finite()));
        let initial = ProjectiveTransform {
            matrix: std::array::from_fn(|i| std::array::from_fn(|j| v[i * 3 + j])),
        };
        let result = refine_projective_pixels_photometric(
            &av,
            &bv,
            initial,
            ProjectiveRegistrationPolicy {
                radius: 8,
                stride: 16,
                rounds: 16,
                max_sample_pairs: 180000000,
            },
            fit,
            || false,
        );
        rows.push(match result {
            Ok(model) => format!(
                "{{\"initial\":{:?},\"refined\":{:?}}}",
                initial.matrix, model.matrix
            ),
            Err(error) => format!("{{\"initial\":{:?},\"error\":\"{error:?}\"}}", initial.matrix),
        });
    }
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"models\":[{}],\"managed_used\":0,\"managed_peak\":{}}}",
        rows.join(","),
        budget.peak()
    );
}
