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
    let policy = rrrah_dedup::affine_region_grid::AffineGridPolicy {
        region: rrrah_dedup::affine_region::AffineRegionPolicy {
            radius: 8,
            maximum_sites: 100000,
            maximum_pixel_reads: 500000000,
            color: rrrah_dedup::affine_color::AffineColorPolicy {
                minimum_samples: 1000,
                maximum_samples: 100000,
                minimum_variance: 1e-5,
                minimum_relative_pivot: 1e-6,
                maximum_coefficient: 5.,
                maximum_offset: 0.1,
            },
            tolerance: 0.03,
        },
        grid: (8, 8),
        maximum_regions: 64,
        maximum_source_radius: 16,
        maximum_total_pixel_reads: 32000000000,
    };
    let evidence =
        rrrah_dedup::affine_region_grid::verify_affine_region_grid(&av, &bv, model, policy, &budget, || {
            false
        });
    let mut rows = Vec::new();
    let status = match evidence {
        Ok(regions) => {
            for region in regions.iter() {
                let domains = region.domains.map(|v| [v.x, v.y, v.width, v.height]);
                let directions = region.directions.map(|value| match value {
                    Ok(e) => format!(
                        "{{\"training_samples\":{},\"samples\":{},\"matched\":{},\"pixel_reads\":{}}}",
                        e.training_samples, e.heldout.samples, e.heldout.matched, e.pixel_reads
                    ),
                    Err(error) => format!("{{\"error\":\"{error:?}\"}}"),
                });
                rows.push(format!(
                    "{{\"domains\":{domains:?},\"source_radius\":{},\"directions\":[{},{}]}}",
                    region.source_radius, directions[0], directions[1]
                ));
            }
            "ok".to_string()
        }
        Err(error) => format!("refusal:{error:?}"),
    };
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"{status}\",\"regions\":[{}],\"managed_used\":0,\"managed_peak\":{}}}",
        rows.join(","),
        budget.peak()
    );
}
