use rrrah_dedup::{
    affine_color::AffineColorPolicy,
    affine_region::{AffineRegionFootprint, AffineRegionPolicy},
    affine_region_file::{AffineRegionFilePolicy, verify_affine_region_files_with_footprints},
    animated::AnimationBudget,
    geometry::ProjectiveTransform,
};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3);
    let text = std::fs::read_to_string(&args[2]).unwrap();
    let values: Vec<f64> = text.split_whitespace().map(|v| v.parse().unwrap()).collect();
    assert_eq!(values.len(), 17);
    let model = ProjectiveTransform {
        matrix: std::array::from_fn(|i| std::array::from_fn(|j| values[i * 3 + j])),
    };
    let rectangle = |start: usize| {
        std::array::from_fn(|i| {
            let value = values[start + i];
            assert!(value.is_finite() && value >= 0. && value <= f64::from(u32::MAX) && value.fract() == 0.);
            value as u32
        })
    };
    let left: [u32; 4] = rectangle(9);
    let right: [u32; 4] = rectangle(13);
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let region = |radius| AffineRegionPolicy {
        radius,
        maximum_sites: 100000,
        maximum_pixel_reads: 500000000,
        color: AffineColorPolicy {
            minimum_samples: 1000,
            maximum_samples: 100000,
            minimum_variance: 1e-5,
            minimum_relative_pivot: 1e-6,
            maximum_coefficient: f64::MAX,
            maximum_offset: f64::MAX,
        },
        tolerance: 0.03,
    };
    let result = verify_affine_region_files_with_footprints(
        &rrrah_decode::DecodeRequest::new(&args[0]),
        &rrrah_decode::DecodeRequest::new(&args[1]),
        model,
        left,
        right,
        AffineRegionFilePolicy {
            decode: AnimationBudget {
                max_frames: 1,
                max_pixels: 6400000,
                max_file_bytes: 5242880,
            },
            forward: region(8),
            reverse: region(8),
            maximum_total_pixel_reads: 1000000000,
        },
        [AffineRegionFootprint::Target, AffineRegionFootprint::Source],
        &budget,
        || false,
    );
    let mut directions = Vec::new();
    let status = match result {
        Err(error) => format!("refusal:{error:?}"),
        Ok(evidence) => {
            for e in evidence {
                let admitted = e.model.matrix.iter().flatten().all(|v| v.abs() <= 5.)
                    && e.model.offset.iter().all(|v| v.abs() <= 0.1);
                let maximum_coefficient = e
                    .model
                    .matrix
                    .iter()
                    .flatten()
                    .fold(0.0_f64, |a, v| a.max(v.abs()));
                let maximum_offset = e.model.offset.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
                directions.push(format!("{{\"radius\":8,\"training_samples\":{},\"samples\":{},\"matched\":{},\"pixel_reads\":{},\"matrix\":{:?},\"offset\":{:?},\"maximum_absolute_coefficient\":{maximum_coefficient},\"maximum_absolute_offset\":{maximum_offset},\"original_bounds_admitted\":{admitted}}}", e.training_samples, e.heldout.samples, e.heldout.matched, e.pixel_reads, e.model.matrix, e.model.offset));
            }
            "ok".to_string()
        }
    };
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"status\":\"{status}\",\"directions\":[{}],\"managed_used\":0,\"managed_peak\":{},\"footprints\":[\"target\",\"source\"]}}",
        directions.join(","),
        budget.peak()
    );
}
