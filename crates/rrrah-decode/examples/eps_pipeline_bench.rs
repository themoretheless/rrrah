//! Reproducible authored EPS pipeline measurements; no external renderer.
use rrrah_core::{MemoryBudget, RasterPixels};
use rrrah_decode::{DecodeRequest, DecodedImage, decode_image, prepare_raster_for_display_with_budget};
use std::{fmt::Write, time::Instant};

fn source(kind: &str) -> String {
    let mut code = String::from(
        "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 512 512\n%%EndComments\n1 0 0 setrgbcolor 3 setlinewidth\n",
    );
    for i in 0..20 {
        let y = 10 + i * 24;
        match kind {
            "fills" => writeln!(code, "10 {y} 480 12 rectfill").unwrap(),
            "strokes" => writeln!(code, "10 {y} 480 12 rectstroke").unwrap(),
            "curves" => writeln!(
                code,
                "10 {y} moveto 100 {} 400 {} 500 {y} curveto stroke",
                y + 16,
                y - 8
            )
            .unwrap(),
            _ => unreachable!(),
        }
    }
    code.push_str("%%EOF\n");
    code
}

fn main() {
    let directory = std::env::temp_dir().join(format!("rrrah-eps-pipeline-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let mut cases = Vec::new();
    for kind in ["fills", "strokes", "curves"] {
        let source = source(kind);
        let path = directory.join(format!("{kind}.eps"));
        std::fs::write(&path, &source).unwrap();
        let mut samples = Vec::new();
        let mut expected_hash = None;
        // First run warms code/pages; retain it separately from measured repeats.
        for iteration in 0..6 {
            let root = MemoryBudget::new(128 * 1024 * 1024);
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(root.clone());
            let start = Instant::now();
            let DecodedImage::Raster(frame) = decode_image(&request).unwrap() else {
                panic!("EPS routed to sensor")
            };
            let decode_ns = start.elapsed().as_nanos() as u64;
            assert_eq!((frame.width(), frame.height()), (512, 512));
            let RasterPixels::Rgba8(pixels) = frame.pixels() else {
                panic!("unexpected native pixels")
            };
            assert!(pixels.chunks_exact(4).any(|p| p[3] != 0));
            let hash = blake3::hash(pixels).to_hex().to_string();
            if let Some(expected) = &expected_hash {
                assert_eq!(&hash, expected);
            } else {
                expected_hash = Some(hash.clone());
            }
            let start = Instant::now();
            let prepared = prepare_raster_for_display_with_budget(&frame, Some(&root)).unwrap();
            let prepare_ns = start.elapsed().as_nanos() as u64;
            let RasterPixels::Rgba32Float(values) = prepared.pixels() else {
                panic!("unexpected display representation")
            };
            assert!(values.iter().all(|v| v.is_finite()));
            let peak = root.peak();
            drop(prepared);
            drop(frame);
            assert_eq!(root.used(), 0, "pipeline leaked root admission");
            samples.push(serde_json::json!({"warmup": iteration == 0, "decode_ns": decode_ns, "display_prepare_ns": prepare_ns, "managed_peak_bytes": peak, "root_after_drop": root.used(), "pixel_blake3": hash}));
        }
        cases.push(serde_json::json!({"case":kind,"source_blake3":blake3::hash(source.as_bytes()).to_hex().to_string(),"samples":samples}));
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"scope":"authored 512x512 EPS, 20 paints per case", "samples_per_case":5,"warmup_per_case":1,"clock":"monotonic wall time","memory":"managed root peak, excludes untracked runtime allocations","hardware_backend":"CPU decode and display preparation; no GPU upload/render measurement","cache_state":"repeated file reads, no image RAM/swap cache", "cases":cases})).unwrap());
}
