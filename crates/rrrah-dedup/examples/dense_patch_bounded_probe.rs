use rrrah_dedup::{
    animated::AnimationBudget,
    gradient::{GradientFeature, describe_gradient_interpolated, gradient_orientation},
};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 2);
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let policy = AnimationBudget {
        max_frames: 1,
        max_pixels: 6400000,
        max_file_bytes: 5242880,
    };
    let mut batches = Vec::new();
    for (path, stride) in args.iter().zip([32u32, 16]) {
        let image = rrrah_dedup::decode::decode_selected_frame_bounded(
            &rrrah_decode::DecodeRequest::new(path),
            policy,
            &budget,
            || false,
        )
        .unwrap();
        let view = image.view(|| false).unwrap();
        let (w, h) = view.dimensions();
        let mut features = Vec::new();
        let mut attempted = 0u64;
        for y in (16..h.saturating_sub(16)).step_by(stride as usize) {
            for x in (16..w.saturating_sub(16)).step_by(stride as usize) {
                attempted += 256;
                assert!(attempted <= 16000000);
                let Some(angle) = gradient_orientation(&view, [x, y], 256, || false).unwrap() else {
                    continue;
                };
                for scale in [1., 2., 4.] {
                    let margin = 16. * scale + 2.;
                    if f64::from(x) < margin
                        || f64::from(y) < margin
                        || f64::from(x) + margin >= f64::from(w)
                        || f64::from(y) + margin >= f64::from(h)
                    {
                        continue;
                    }
                    attempted += 256;
                    assert!(attempted <= 16000000);
                    if let Some(descriptor) = describe_gradient_interpolated(
                        &view,
                        [f64::from(x), f64::from(y)],
                        scale,
                        angle,
                        256,
                        || false,
                    )
                    .unwrap()
                    {
                        assert!(features.len() < 20000);
                        features.push(GradientFeature {
                            position: [f64::from(x), f64::from(y)],
                            descriptor,
                        });
                    }
                }
            }
        }
        batches.push(features);
    }
    let source = &batches[0];
    let target = &batches[1];
    assert!(source.len().checked_mul(target.len()).unwrap() <= 400000000);
    let mut rows = Vec::new();
    let mut radius_pass = 0;
    for a in source {
        let mut distances: Vec<_> = target
            .iter()
            .enumerate()
            .map(|(i, b)| {
                (
                    a.descriptor
                        .0
                        .iter()
                        .zip(b.descriptor.0)
                        .map(|(x, y)| (x - y) * (x - y))
                        .sum::<f64>(),
                    i,
                )
            })
            .collect();
        distances.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        if distances.is_empty() || distances[0].0 > 0.5 {
            continue;
        };
        radius_pass += 1;
        let (first, id) = distances[0];
        let position = target[id].position;
        let second = distances
            .iter()
            .find(|(_, j)| {
                let p = target[*j].position;
                (p[0] - position[0]).hypot(p[1] - position[1]) > 2.
            })
            .map(|v| v.0);
        if second.is_some_and(|d| first < 0.64 * d) {
            rows.push(format!("{{\"source\":{:?},\"target\":{:?},\"squared_distance\":{first},\"second_distinct_squared_distance\":{}}}",a.position,position,second.unwrap()));
        }
    }
    println!(
        "{{\"status\":\"ok\",\"feature_counts\":[{},{}],\"radius_pass\":{radius_pass},\"matches\":[{}],\"scope\":\"Dense stride32/16, explicit scales1/2/4, original linear patches; candidates only, no geometric or pixel acceptance. Feature/output allocations outside managed decode accounting.\"}}",
        source.len(),
        target.len(),
        rows.join(",")
    );
}
