use rrrah_dedup::{animated::AnimationBudget, geometry::ProjectiveTransform};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3);
    let text = std::fs::read_to_string(&args[2]).unwrap();
    let v: Vec<f64> = text.split_whitespace().map(|v| v.parse().unwrap()).collect();
    assert_eq!(v.len(), 9);
    let model = ProjectiveTransform {
        matrix: std::array::from_fn(|i| std::array::from_fn(|j| v[i * 3 + j])),
    };
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let policy = AnimationBudget {
        max_frames: 1,
        max_pixels: 6400000,
        max_file_bytes: 5242880,
    };
    let a = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(&args[0]),
        policy,
        &budget,
        || false,
    )
    .unwrap();
    let b = rrrah_dedup::decode::decode_selected_frame_bounded(
        &rrrah_decode::DecodeRequest::new(&args[1]),
        policy,
        &budget,
        || false,
    )
    .unwrap();
    let av = a.view(|| false).unwrap();
    let bv = b.view(|| false).unwrap();
    let mut rows = Vec::new();
    for (label, source, target, transform, domain) in [
        ("forward", &av, &bv, model, [240, 60, 80, 60]),
        (
            "reverse",
            &bv,
            &av,
            model.inverse().unwrap(),
            [2115, 922, 103, 133],
        ),
    ] {
        let source_domain = if label == "forward" {
            [2115, 922, 103, 133]
        } else {
            [240, 60, 80, 60]
        };
        let radius = if label == "forward" {
            8
        } else {
            rrrah_dedup::region_footprint::select_filter_footprint(model, [2166.5, 988.5], 8, 16, || false)
                .unwrap()
                .source_radius
        };
        let result = rrrah_dedup::affine_region::verify_affine_region(
            source,
            target,
            transform,
            source_domain,
            domain,
            rrrah_dedup::affine_region::AffineRegionPolicy {
                radius,
                maximum_sites: 20000,
                maximum_pixel_reads: 64000000,
                color: rrrah_dedup::affine_color::AffineColorPolicy {
                    minimum_samples: 1000,
                    maximum_samples: 20000,
                    minimum_variance: 1e-5,
                    minimum_relative_pivot: 1e-6,
                    maximum_coefficient: 5.,
                    maximum_offset: 0.1,
                },
                tolerance: 0.03,
            },
            &budget,
            || false,
        )
        .unwrap();
        rows.push(format!("{{\"direction\":\"{label}\",\"radius\":{radius},\"training_samples\":{},\"heldout_samples\":{},\"matched\":{},\"pixel_reads\":{}}}",result.training_samples,result.heldout.samples,result.heldout.matched,result.pixel_reads));
    }
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"ok\",\"directions\":[{}],\"managed_used\":0,\"managed_peak\":{},\"scope\":\"One fixed ROI pair, native opaque display-range decoder and mapped box forward radius8, reverse geometry-derived radius; separate checker training/heldout, coefficients5/offset0.1, tolerance0.03. Sample vector payloads use the shared memory budget; allocator overhead excluded. Centers restricted to opposite ROI; filter taps retain whole-image support. Diagnostic only, no copy decision.\"}}",
        rows.join(","),
        budget.peak()
    );
}
