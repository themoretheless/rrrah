use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::Correspondence,
    piecewise_warp::{PiecewisePolicy, PiecewiseWarp},
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
    let budget = MemoryBudget::new(16 * 1024 * 1024);
    let face_path = std::env::args().nth(2).ok_or("face file required")?;
    let text = std::fs::read_to_string(face_path)?;
    let faces: Vec<[usize; 3]> = text
        .lines()
        .map(|line| {
            let v: Vec<usize> = line
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<_, _>>()?;
            if v.len() != 3 {
                return Err("three indices required".into());
            }
            Ok([v[0], v[1], v[2]])
        })
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;
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
    let status = match &mesh {
        Ok(_) => "accepted".to_string(),
        Err(e) => format!("{e:?}"),
    };
    println!(
        "{{\"landmarks\":{},\"triangles\":{:?},\"source_mesh_status\":\"{}\",\"managed_bytes\":{}}}",
        points.len(),
        faces.iter().collect::<Vec<_>>(),
        status,
        budget.used()
    );
    drop(mesh);
    drop(faces);
    assert_eq!(budget.used(), 0);
    Ok(())
}
