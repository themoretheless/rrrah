//! Diagnostic: exhaustive projective hypotheses from a small supplied point set.
use rrrah_dedup::geometry::{Correspondence, GeometryPolicy, verify_projective_for_domains};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("point file and source/target dimensions required".into());
    }
    let path = &args[0];
    let dimensions: Vec<u32> = args[1..].iter().map(|v| v.parse()).collect::<Result<_, _>>()?;
    let text = std::fs::read_to_string(path)?;
    let points: Vec<_> = text
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
    let result = verify_projective_for_domains(
        &points,
        GeometryPolicy {
            tolerance: 2.,
            min_inliers: 10,
            max_points: 42,
            max_hypotheses: 200000,
        },
        (dimensions[0], dimensions[1]),
        (dimensions[2], dimensions[3]),
        || false,
    )?;
    match result {
        Some(e) => println!(
            "{{\"status\":\"model\",\"points\":{},\"matrix\":{:?},\"inliers\":{:?},\"squared_error\":{}}}",
            points.len(),
            e.transform.matrix,
            e.inliers,
            e.squared_error
        ),
        None => println!("{{\"status\":\"no_model\",\"points\":{}}}", points.len()),
    }
    Ok(())
}
