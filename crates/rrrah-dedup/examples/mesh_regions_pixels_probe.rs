//! Diagnostic explicit equal normalized diagonal residuals; no automatic policy.
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_dedup::{
    exact::ContentSnapshot,
    geometry::Correspondence,
    linear::LinearRgbaView,
    mesh_grid::{MeshDirection, MeshGridPolicy, build_mesh_grid},
    mesh_rank::compare_mesh_rank_region,
    piecewise_warp::{PiecewisePolicy, PiecewiseWarp},
    rank_region::RankRegionPolicy,
    triangulation::{DiagonalPolicy, TriangulationPolicy, propose_source_diagonals, triangulate_targets},
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
        return Err("source pixels,target pixels,point file required".into());
    }
    let budget = MemoryBudget::new(512 * 1024 * 1024);
    let first = ContentSnapshot::read(Path::new(&args[0]), 110000000, &|| false)?;
    let second = ContentSnapshot::read(Path::new(&args[1]), 110000000, &|| false)?;
    let (sw, sh, a) = pixels(Path::new(&args[0]), &budget)?;
    let (tw, th, b) = pixels(Path::new(&args[1]), &budget)?;
    let av = LinearRgbaView::new(sw, sh, &a, 6400000, || false)?;
    let bv = LinearRgbaView::new(tw, th, &b, 6400000, || false)?;
    let text = std::fs::read_to_string(&args[2])?;
    let points: Vec<Correspondence> = text
        .lines()
        .map(|line| {
            let v: Vec<f64> = line
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<_, _>>()?;
            if v.len() != 4 {
                return Err("four coordinates required".into());
            }
            Ok(Correspondence {
                source: [v[0], v[1]],
                target: [v[2], v[3]],
            })
        })
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;
    let regions: Vec<u32> = std::fs::read_to_string(&args[3])?
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    if regions.len() != 8 {
        return Err("eight region coordinates required".into());
    }
    let sr: [u32; 4] = std::array::from_fn(|i| regions[i]);
    let tr: [u32; 4] = std::array::from_fn(|i| regions[4 + i]);
    let faces = triangulate_targets(
        &points,
        (tw, th),
        TriangulationPolicy {
            maximum_points: 1000,
            maximum_triangles: 3000,
            maximum_predicate_tests: 10000000,
        },
        &budget,
        || false,
    )?;
    let proposals = propose_source_diagonals(
        &points,
        &faces,
        DiagonalPolicy {
            maximum_points: 1000,
            maximum_triangles: 3000,
            maximum_flips: 64,
            maximum_tests: 10000000,
            minimum_twice_area: 1e-6,
        },
        &budget,
        || false,
    )?;
    drop(faces);
    let faces = proposals;
    let mesh = PiecewiseWarp::from_triangles(
        &points,
        &faces,
        (sw, sh),
        (tw, th),
        PiecewisePolicy {
            maximum_points: 1000,
            maximum_triangles: 3000,
            maximum_pair_tests: 10000000,
            minimum_twice_area: 1e-6,
        },
        &budget,
        || false,
    );
    let mesh = mesh?;
    for (name, direction, source, target, ar, br, w, h) in [
        ("forward", MeshDirection::TargetToSource, &av, &bv, sr, tr, tw, th),
        ("reverse", MeshDirection::SourceToTarget, &bv, &av, tr, sr, sw, sh),
    ] {
        if br[2] == 0
            || br[3] == 0
            || br[0].checked_add(br[2]).is_none_or(|v| v > w)
            || br[1].checked_add(br[3]).is_none_or(|v| v > h)
        {
            return Err("invalid region".into());
        }
        let x = br[0].saturating_sub(16);
        let y = br[1].saturating_sub(16);
        let right = (br[0] + br[2]).saturating_add(16).min(w);
        let bottom = (br[1] + br[3]).saturating_add(16).min(h);
        let grid = build_mesh_grid(
            &mesh,
            [x, y, right - x, bottom - y],
            direction,
            MeshGridPolicy {
                maximum_triangles: 3000,
                maximum_sites: 6400000,
                maximum_face_sites: 100000000,
            },
            &budget,
            || false,
        )?;
        for radius in [0, 3, 8] {
            let e = compare_mesh_rank_region(
                source,
                target,
                &grid,
                ar,
                br,
                RankRegionPolicy {
                    radius: 8,
                    minimum_contrast: 0.005,
                    minimum_pairs: 1000,
                    maximum_sites: 6400000,
                    maximum_pixel_reads: 32000000,
                },
                radius,
                &budget,
                || false,
            )?;
            println!(
                "{{\"direction\":\"{name}\",\"filter_radius\":{radius},\"sites\":{},\"covered\":{},\"valid_sites\":{},\"informative_pairs\":{},\"agreeing_pairs\":{},\"pixel_reads\":{}}}",
                e.sites,
                grid.covered_sites(),
                e.valid_sites,
                e.informative_pairs,
                e.agreeing_pairs,
                e.pixel_reads
            );
        }
    }
    first.verify(&|| false)?;
    second.verify(&|| false)?;
    drop(a);
    drop(b);
    drop(mesh);
    drop(faces);
    assert_eq!(budget.used(), 0);
    Ok(())
}
