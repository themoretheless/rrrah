use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    animated::AnimationBudget,
    decode::decode_selected_frame_bounded,
    exact::ContentSnapshot,
    mesh_grid::{MeshDirection, MeshGridPolicy, build_mesh_grid},
    mesh_rank::compare_mesh_rank,
    rank_region::RankRegionPolicy,
};
use rrrah_dedup::{
    geometry::Correspondence,
    piecewise_warp::{PiecewisePolicy, PiecewiseWarp},
    triangulation::{DiagonalPolicy, TriangulationPolicy, propose_source_diagonals, triangulate_targets},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("landmark JSON path required")?;
    let text = std::fs::read_to_string(path)?;
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
    let budget = MemoryBudget::new(512 * 1024 * 1024);
    let source = rrrah_decode::DecodeRequest::new(std::env::args().nth(2).ok_or("source required")?);
    let target = rrrah_decode::DecodeRequest::new(std::env::args().nth(3).ok_or("query required")?);
    let first = ContentSnapshot::read(&source.path, 5242880, &|| false)?;
    let second = ContentSnapshot::read(&target.path, 5242880, &|| false)?;
    let decode = AnimationBudget {
        max_frames: 1,
        max_pixels: 6400000,
        max_file_bytes: 5242880,
    };
    let a = decode_selected_frame_bounded(&source, decode, &budget, || false)?;
    let b = decode_selected_frame_bounded(&target, decode, &budget, || false)?;
    let av = a.view(|| false)?;
    let bv = b.view(|| false)?;
    if av.dimensions() != (2048, 1536) || bv.dimensions() != (800, 600) {
        return Err("unexpected image dimensions".into());
    }
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
    );
    let mesh = mesh?;
    for (name, direction, source, target, w, h) in [
        ("forward", MeshDirection::TargetToSource, &av, &bv, 800, 600),
        ("reverse", MeshDirection::SourceToTarget, &bv, &av, 2048, 1536),
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
                source,
                target,
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
        drop(grid);
    }
    first.verify(&|| false)?;
    second.verify(&|| false)?;
    drop(av);
    drop(bv);
    drop(a);
    drop(b);
    drop(mesh);
    drop(faces);
    assert_eq!(budget.used(), 0);
    Ok(())
}
