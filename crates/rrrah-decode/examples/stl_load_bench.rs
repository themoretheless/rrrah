//! Paired current whole-file and streaming STL API timings, excluding GPU.
use rrrah_decode::{DecodeRequest, DecodedModel, StlMesh, decode_model, decode_stl};
use std::{sync::Arc, time::Instant};
fn digest(mesh: &StlMesh) -> String {
    let mut hash = blake3::Hasher::new();
    for facet in &mesh.facets {
        for value in facet.normal.iter().chain(facet.vertices.iter().flatten()) {
            hash.update(&value.to_bits().to_le_bytes());
        }
        hash.update(&facet.attribute.to_le_bytes());
    }
    if let Some(bounds) = mesh.bounds {
        hash.update(&[1]);
        for value in bounds.into_iter().flatten() { hash.update(&value.to_bits().to_le_bytes()); }
    } else { hash.update(&[0]); }
    hash.finalize().to_hex().to_string()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 { return Err("usage: stl_load_bench FILE.stl".into()); }
    let root = rrrah_core::MemoryBudget::new(1024 * 1024 * 1024);
    let mut request = DecodeRequest::new(&args[1]);
    request.memory_budget = Some(root.clone());
    let mut expected = None;
    let mut rows = Vec::new();
    // ABBA pairs reverse order to reduce one-sided warm-cache scheduling bias.
    for block in 0..12 {
        for streaming in [false, true, true, false] {
            let started = Instant::now();
            let model = if streaming { decode_model(&request)? } else {
                DecodedModel::Stl(Arc::new(decode_stl(&request)?).into()).try_manage(&root)?
            };
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.;
            let DecodedModel::Stl(mesh) = &model else { return Err("requires STL".into()); };
            let current = digest(mesh);
            if let Some(previous) = &expected { assert_eq!(previous, &current); }
            expected = Some(current);
            let facets = mesh.facets.len();
            let retained_bytes = model.capacity_bytes();
            drop(model);
            assert_eq!(root.used(), 0);
            if block >= 2 {
                rows.push(serde_json::json!({"streaming": streaming, "block": block,
                    "elapsed_ms": elapsed_ms, "facets": facets, "retained_bytes": retained_bytes}));
            }
        }
    }
    println!("{}", serde_json::json!({"file": args[1], "warmup_blocks":2,
        "samples": rows, "geometry_blake3": expected, "managed_final_bytes":root.used(),
        "scope":"current whole-file low-level API plus output management versus shared streaming model API; warm filesystem; no GPU"}));
    Ok(())
}
