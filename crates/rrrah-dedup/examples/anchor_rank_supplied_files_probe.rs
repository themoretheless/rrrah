//! Confirm externally proposed geometry on fresh original files; proposal diagnostic only.
use rrrah_dedup::{
    anchor_rank::AnchorRankPolicy,
    anchor_rank_file::compare_anchor_rank_files,
    animated::AnimationBudget,
    geometry::{Correspondence, ProjectiveTransform},
    local_rank::LocalRankPolicy,
    rank_region::RankRegionPolicy,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 { return Err("source, target, model, points, source tolerance required".into()); }
    for path in &args[2..4] {
        if std::fs::metadata(path)?.len() > 1024 * 1024 { return Err("proposal input cap".into()); }
    }
    let values: Vec<f64> = std::fs::read_to_string(&args[2])?.split_whitespace().map(str::parse).collect::<Result<_, _>>()?;
    if values.len() != 9 { return Err("nine model coefficients required".into()); }
    let model = ProjectiveTransform { matrix: std::array::from_fn(|i| std::array::from_fn(|j| values[3*i+j])) };
    let mut points = Vec::new();
    for line in std::fs::read_to_string(&args[3])?.lines() {
        let v: Vec<f64> = line.split_whitespace().map(str::parse).collect::<Result<_, _>>()?;
        if v.len() != 4 || points.len() >= 28000 { return Err("invalid or excessive proposals".into()); }
        points.push(Correspondence { source: [v[0],v[1]], target: [v[2],v[3]] });
    }
    let policy = AnchorRankPolicy {
        local: LocalRankPolicy {
            rank: RankRegionPolicy { radius: 8, minimum_contrast: 0.005, minimum_pairs: 1000, maximum_sites: 80000000, maximum_pixel_reads: 400000000 },
            filter_radius: 8, minimum_witnesses: 10, maximum_points: 28000,
            maximum_point_checks: 392014000, minimum_point_separation: 2.,
            target_tolerance: 2., source_tolerance: args[4].parse()?, minimum_coverage: 0.3, minimum_agreement: 0.9,
        },
        window_radius: 2, maximum_selection_checks: 1176014000,
    };
    let budget = rrrah_core::MemoryBudget::new(512*1024*1024);
    let result = compare_anchor_rank_files(
        &rrrah_decode::DecodeRequest::new(&args[0]), &rrrah_decode::DecodeRequest::new(&args[1]), model, &points,
        AnimationBudget { max_frames: 1, max_pixels: 12800000, max_file_bytes: 5242880 }, policy, &budget, || false,
    );
    match result {
        Ok(a) => println!("{{\"status\":\"ok\",\"supported\":{},\"anchors\":[{},{}],\"directions\":[{{\"sites\":{},\"valid_sites\":{},\"informative_pairs\":{},\"agreeing_pairs\":{}}},{{\"sites\":{},\"valid_sites\":{},\"informative_pairs\":{},\"agreeing_pairs\":{}}}]}}",a.supported,a.forward_anchors,a.reverse_anchors,a.forward.sites,a.forward.valid_sites,a.forward.informative_pairs,a.forward.agreeing_pairs,a.reverse.sites,a.reverse.valid_sites,a.reverse.informative_pairs,a.reverse.agreeing_pairs),
        Err(e) => println!("{{\"status\":\"refusal:{e:?}\",\"supported\":false}}"),
    }
    assert_eq!(budget.used(), 0);
    eprintln!("memory_used=0 memory_peak={}", budget.peak());
    Ok(())
}
