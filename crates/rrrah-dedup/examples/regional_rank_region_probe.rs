use rrrah_dedup::{
    animated::AnimationBudget,
    decode::decode_selected_frame_bounded,
    exact::ContentSnapshot,
    geometry::ProjectiveTransform,
    rank_region::{RankRegionPolicy, compare_filtered_rank_region},
};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 4);
    let filter: u32 = args[3].parse().unwrap();
    assert!(filter <= 8);
    let v: Vec<f64> = std::fs::read_to_string(&args[2])
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(v.len(), 17);
    let h = ProjectiveTransform {
        matrix: std::array::from_fn(|i| std::array::from_fn(|j| v[i * 3 + j])),
    };
    let rectangle = |begin: usize| {
        std::array::from_fn(|i| {
            let value = v[begin + i];
            assert!(value.is_finite() && value >= 0. && value <= f64::from(u32::MAX) && value.fract() == 0.);
            value as u32
        })
    };
    let left = rrrah_decode::DecodeRequest::new(&args[0]);
    let right = rrrah_decode::DecodeRequest::new(&args[1]);
    let first = ContentSnapshot::read(&left.path, 5242880, &|| false).unwrap();
    let second = ContentSnapshot::read(&right.path, 5242880, &|| false).unwrap();
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let decode = AnimationBudget {
        max_frames: 1,
        max_pixels: 6400000,
        max_file_bytes: 5242880,
    };
    let a = decode_selected_frame_bounded(&left, decode, &budget, || false).unwrap();
    let b = decode_selected_frame_bounded(&right, decode, &budget, || false).unwrap();
    let av = a.view(|| false).unwrap();
    let bv = b.view(|| false).unwrap();
    let policy = RankRegionPolicy {
        radius: 8,
        minimum_contrast: 0.005,
        minimum_pairs: 1000,
        maximum_sites: 2000000,
        maximum_pixel_reads: 90000000 * (u64::from(filter) * 2 + 1).pow(2),
    };
    let f = compare_filtered_rank_region(&av, &bv, h, rectangle(9), rectangle(13), policy, filter, || false);
    let r = compare_filtered_rank_region(
        &bv,
        &av,
        h.inverse().unwrap(),
        rectangle(13),
        rectangle(9),
        policy,
        filter,
        || false,
    );
    first.verify(&|| false).unwrap();
    second.verify(&|| false).unwrap();
    let format = |result: Result<
        rrrah_dedup::rank_region::RankRegionEvidence,
        rrrah_dedup::rank_region::RankRegionError,
    >| match result {
        Ok(e) => format!(
            "{{\"status\":\"ok\",\"sites\":{},\"valid_sites\":{},\"informative_pairs\":{},\"agreeing_pairs\":{},\"pixel_reads\":{}}}",
            e.sites, e.valid_sites, e.informative_pairs, e.agreeing_pairs, e.pixel_reads
        ),
        Err(e) => format!("{{\"status\":\"refusal:{e:?}\"}}"),
    };
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"directions\":[{},{}],\"managed_used\":0,\"managed_peak\":{}}}",
        format(f),
        format(r),
        budget.peak()
    );
}
