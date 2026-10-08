//! Geometric anchor pixel diagnostic; no automatic identity policy.
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_dedup::{
    exact::ContentSnapshot,
    geometry::{Correspondence, ProjectiveTransform},
    linear::LinearRgbaView,
    local_rank::LocalRankPolicy,
    anchor_rank::{AnchorRankPolicy, compare_anchor_rank},
    rank_region::RankRegionPolicy,
};
use std::{
    io::{BufReader, Read},
    path::Path,
};
fn pixels(
    path: &Path,
    budget: &MemoryBudget,
) -> Result<(u32, u32, SharedBuffer<f32>), Box<dyn std::error::Error>> {
    let scratch = budget.try_reserve(65536)?;
    let mut file = BufReader::with_capacity(65536, std::fs::File::open(path)?);
    let mut magic = [0; b"RRRAH-RANK-RGBA32-V1\n".len()];
    file.read_exact(&mut magic)?;
    if &magic != b"RRRAH-RANK-RGBA32-V1\n" {
        return Err("invalid pixel header".into());
    }
    let mut b = [0; 4];
    file.read_exact(&mut b)?;
    let w = u32::from_le_bytes(b);
    file.read_exact(&mut b)?;
    let h = u32::from_le_bytes(b);
    let n = u64::from(w) * u64::from(h);
    if n == 0 || n > 6400000 {
        return Err("pixel limit".into());
    }
    let credit = budget.try_reserve(n * 16)?;
    let mut samples = Vec::new();
    samples.try_reserve_exact(usize::try_from(n * 4)?)?;
    for _ in 0..n * 4 {
        file.read_exact(&mut b)?;
        samples.push(f32::from_le_bytes(b));
    }
    if file.read(&mut b)? != 0 {
        return Err("trailing pixels".into());
    }
    drop(file);
    drop(scratch);
    Ok((w, h, credit.try_adopt(samples)?))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("source pixels, target pixels, model/regions and point file required".into());
    }
    let values: Vec<f64> = std::fs::read_to_string(&args[2])?
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    if values.len() != 9 { return Err("invalid model".into()); }
    let model = ProjectiveTransform {
        matrix: std::array::from_fn(|i| std::array::from_fn(|j| values[i * 3 + j])),
    };
    let text = std::fs::read_to_string(&args[3])?;
    let mut points = Vec::new();
    for line in text.lines() {
        let v: Vec<f64> = line
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        if v.len() != 4 {
            return Err("invalid point".into());
        }
        points.push(Correspondence {
            source: [v[0], v[1]],
            target: [v[2], v[3]],
        });
    }
    let budget = MemoryBudget::new(512 * 1024 * 1024);
    let first = ContentSnapshot::read(Path::new(&args[0]), 110000000, &|| false)?;
    let second = ContentSnapshot::read(Path::new(&args[1]), 110000000, &|| false)?;
    let (sw, sh, a) = pixels(Path::new(&args[0]), &budget)?;
    let (tw, th, b) = pixels(Path::new(&args[1]), &budget)?;
    let av = LinearRgbaView::new(sw, sh, &a, 6400000, || false)?;
    let bv = LinearRgbaView::new(tw, th, &b, 6400000, || false)?;
    for radius in [0, 3, 8] {
        let policy = LocalRankPolicy {
            rank: RankRegionPolicy {
                radius: 8,
                minimum_contrast: 0.005,
                minimum_pairs: 1000,
                maximum_sites: 12800000,
                maximum_pixel_reads: 64000000,
            },
            filter_radius: radius,
            minimum_witnesses: 10,
            maximum_points: 28000,
            maximum_point_checks: 392014000,
            minimum_point_separation: 2.,
            target_tolerance: 2.,
            source_tolerance: 2. * f64::from(sw).hypot(f64::from(sh)) / f64::from(tw).hypot(f64::from(th)),
            minimum_coverage: 0.3,
            minimum_agreement: 0.9,
        };
        match compare_anchor_rank(&av, &bv, model, &points, AnchorRankPolicy { local: policy, window_radius: 2, maximum_selection_checks: 1176014000 }, &budget, || false) {
            Ok(e) => println!(
                "{{\"filter_radius\":{radius},\"status\":\"ok\",\"forward_anchors\":{},\"reverse_anchors\":{},\"supported\":{},\"directions\":[{{\"sites\":{},\"valid_sites\":{},\"informative_pairs\":{},\"agreeing_pairs\":{}}},{{\"sites\":{},\"valid_sites\":{},\"informative_pairs\":{},\"agreeing_pairs\":{}}}]}}",
                e.forward_anchors,
                e.reverse_anchors,
                e.supported,
                e.forward.sites,
                e.forward.valid_sites,
                e.forward.informative_pairs,
                e.forward.agreeing_pairs,
                e.reverse.sites,
                e.reverse.valid_sites,
                e.reverse.informative_pairs,
                e.reverse.agreeing_pairs
            ),
            Err(e) => println!("{{\"filter_radius\":{radius},\"status\":\"refusal:{e:?}\"}}"),
        }
    }
    first.verify(&|| false)?;
    second.verify(&|| false)?;
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    Ok(())
}
