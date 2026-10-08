use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_dedup::{
    exact::ContentSnapshot,
    geometry::Correspondence,
    linear::LinearRgbaView,
    mesh_grid::{MeshDirection, MeshGridPolicy, build_mesh_grid},
    mesh_rank::compare_mesh_rank,
    mesh_refine::{
        LandmarkAdmissionPolicy, MeshRefinePolicy, admit_mesh_landmarks, propose_mesh_translations,
    },
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
        return Err("source pixels, target pixels, landmark file, seed file required".into());
    }
    let budget = MemoryBudget::new(512 * 1024 * 1024);
    let first = ContentSnapshot::read(Path::new(&args[0]), 110000000, &|| false)?;
    let second = ContentSnapshot::read(Path::new(&args[1]), 110000000, &|| false)?;
    let (sw, sh, a) = pixels(Path::new(&args[0]), &budget)?;
    let (tw, th, b) = pixels(Path::new(&args[1]), &budget)?;
    let source = LinearRgbaView::new(sw, sh, &a, 6400000, || false)?;
    let target = LinearRgbaView::new(tw, th, &b, 6400000, || false)?;
    let points: Vec<Correspondence> = std::fs::read_to_string(&args[2])?
        .lines()
        .map(|line| {
            let v: Vec<f64> = line
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<_, _>>()?;
            if v.len() != 4 {
                return Err("four landmark coordinates required".into());
            }
            Ok(Correspondence {
                source: [v[0], v[1]],
                target: [v[2], v[3]],
            })
        })
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;
    let seeds: Vec<[u32; 2]> = std::fs::read_to_string(&args[3])?
        .lines()
        .map(|line| {
            let v: Vec<u32> = line
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<_, _>>()?;
            if v.len() != 2 {
                return Err("two seed coordinates required".into());
            }
            Ok([v[0], v[1]])
        })
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;
    let faces = triangulate_targets(
        &points,
        (800, 600),
        TriangulationPolicy {
            maximum_points: 1000,
            maximum_triangles: 3000,
            maximum_predicate_tests: 10000000,
        },
        &budget,
        || false,
    )?;
    let fixed = propose_source_diagonals(
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
    let mesh = PiecewiseWarp::from_triangles(
        &points,
        &fixed,
        (2048, 1536),
        (800, 600),
        PiecewisePolicy {
            maximum_points: 1000,
            maximum_triangles: 3000,
            maximum_pair_tests: 10000000,
            minimum_twice_area: 1e-6,
        },
        &budget,
        || false,
    )?;
    let grid = build_mesh_grid(
        &mesh,
        [0, 0, 800, 600],
        MeshDirection::TargetToSource,
        MeshGridPolicy {
            maximum_triangles: 3000,
            maximum_sites: 480000,
            maximum_face_sites: 100000000,
        },
        &budget,
        || false,
    )?;
    let results = propose_mesh_translations(
        &source,
        &target,
        &grid,
        &seeds,
        MeshRefinePolicy {
            offset_radius: 24,
            offset_step: 2,
            minimum_contrast: 0.005,
            minimum_pairs: 64,
            minimum_training_fraction: 0.9,
            minimum_validation_fraction: 0.9,
            maximum_seeds: 2000,
            maximum_offsets: 625,
            maximum_pixel_reads: 5000000000,
        },
        &budget,
        || false,
    )?;
    let admitted = admit_mesh_landmarks(
        &points,
        &seeds,
        &results,
        (sw, sh),
        (tw, th),
        LandmarkAdmissionPolicy {
            maximum_points: 1000,
            maximum_proposals: 2000,
            maximum_distance_tests: 10000000,
            minimum_separation: 2.,
        },
        &budget,
        || false,
    )?;
    println!(
        "{{\"kind\":\"admission\",\"original_points\":{},\"eligible\":{},\"duplicates\":{},\"points\":{}}}",
        points.len(),
        admitted.eligible_proposals,
        admitted.duplicate_proposals,
        admitted.points.len()
    );
    for p in admitted.points.iter() {
        println!(
            "{{\"kind\":\"point\",\"source\":{:?},\"target\":{:?}}}",
            p.source, p.target
        );
    }
    drop(grid);
    drop(mesh);
    drop(fixed);
    drop(faces);
    let points = &admitted.points;
    let faces = triangulate_targets(
        &points,
        (800, 600),
        TriangulationPolicy {
            maximum_points: 1000,
            maximum_triangles: 3000,
            maximum_predicate_tests: 10000000,
        },
        &budget,
        || false,
    )?;
    let fixed = propose_source_diagonals(
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
    let mesh = PiecewiseWarp::from_triangles(
        &points,
        &fixed,
        (2048, 1536),
        (800, 600),
        PiecewisePolicy {
            maximum_points: 1000,
            maximum_triangles: 3000,
            maximum_pair_tests: 10000000,
            minimum_twice_area: 1e-6,
        },
        &budget,
        || false,
    )?;
    for (name, direction, src, dst, w, h) in [
        ("forward", MeshDirection::TargetToSource, &source, &target, tw, th),
        ("reverse", MeshDirection::SourceToTarget, &target, &source, sw, sh),
    ] {
        let grid = build_mesh_grid(
            &mesh,
            [0, 0, w, h],
            direction,
            MeshGridPolicy {
                maximum_triangles: 3000,
                maximum_sites: 4000000,
                maximum_face_sites: 100000000,
            },
            &budget,
            || false,
        )?;
        for radius in [0, 3, 8] {
            let evidence = compare_mesh_rank(
                src,
                dst,
                &grid,
                RankRegionPolicy {
                    radius: 8,
                    minimum_contrast: 0.005,
                    minimum_pairs: 1000,
                    maximum_sites: 4000000,
                    maximum_pixel_reads: 20000000,
                },
                radius,
                &budget,
                || false,
            )?;
            println!(
                "{{\"direction\":\"{name}\",\"filter_radius\":{radius},\"sites\":{},\"covered\":{},\"valid_sites\":{},\"informative_pairs\":{},\"agreeing_pairs\":{},\"pixel_reads\":{}}}",
                evidence.sites,
                grid.covered_sites(),
                evidence.valid_sites,
                evidence.informative_pairs,
                evidence.agreeing_pairs,
                evidence.pixel_reads
            );
        }
    }
    first.verify(&|| false)?;
    second.verify(&|| false)?;
    drop(results);
    drop(mesh);
    drop(fixed);
    drop(faces);
    drop(admitted);
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    Ok(())
}
