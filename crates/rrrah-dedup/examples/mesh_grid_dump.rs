use rrrah_core::MemoryBudget;
use rrrah_dedup::mesh_grid::{MeshDirection, MeshGridPolicy, build_mesh_grid};
use rrrah_dedup::{
    geometry::Correspondence,
    piecewise_warp::{PiecewisePolicy, PiecewiseWarp},
    triangulation::{DiagonalPolicy, TriangulationPolicy, propose_source_diagonals, triangulate_targets},
};
use std::io::Write;
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
    let budget = MemoryBudget::new(128 * 1024 * 1024);
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
    let prefix = std::env::args().nth(2).ok_or("output prefix required")?;
    for (name, direction, w, h) in [
        ("forward", MeshDirection::TargetToSource, 800, 600),
        ("reverse", MeshDirection::SourceToTarget, 2048, 1536),
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
        let path = format!("{prefix}-{name}.xy64");
        let mut file = std::io::BufWriter::new(std::fs::File::create_new(&path)?);
        file.write_all(b"RRRAH-MESH-XY64-V1\n")?;
        file.write_all(&w.to_le_bytes())?;
        file.write_all(&h.to_le_bytes())?;
        for y in 0..h {
            for x in 0..w {
                let p = grid.coordinate(x, y).unwrap_or([f64::NAN; 2]);
                for v in p {
                    file.write_all(&v.to_le_bytes())?;
                }
            }
        }
        file.flush()?;
        println!(
            "{{\"direction\":\"{name}\",\"width\":{w},\"height\":{h},\"covered\":{},\"managed_bytes\":{}}}",
            grid.covered_sites(),
            budget.used()
        );
        drop(grid);
    }
    drop(mesh);
    drop(faces);
    assert_eq!(budget.used(), 0);
    Ok(())
}
