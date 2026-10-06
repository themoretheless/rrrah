use moxcms::{ColorProfile, Layout, TransformOptions};
use std::{hint::black_box, time::Instant};
fn main() {
    let profile =
        ColorProfile::new_from_slice(&std::fs::read(std::env::args().nth(1).unwrap()).unwrap()).unwrap();
    let dest = ColorProfile::new_srgb();
    let mut builds = Vec::new();
    let mut rates = Vec::new();
    let mut input = Vec::with_capacity(262144 * 4);
    let mut state = 0x91827364u32;
    for _ in 0..262144 * 4 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        input.push((state >> 8) as f32 / 16777215.0);
    }
    let mut out = vec![0.0; 262144 * 3];
    let mut checksum = 0.0f64;
    for _ in 0..5 {
        let begin = Instant::now();
        let executor = profile
            .create_transform_f32(Layout::Rgba, &dest, Layout::Rgb, TransformOptions::default())
            .unwrap();
        builds.push(begin.elapsed().as_secs_f64());
        executor.transform(&input, &mut out).unwrap();
        let begin = Instant::now();
        for _ in 0..16 {
            executor
                .transform(black_box(&input), black_box(&mut out))
                .unwrap();
        }
        rates.push((262144.0 * 16.0) / begin.elapsed().as_secs_f64());
        checksum += out.iter().map(|v| f64::from(*v)).sum::<f64>();
    }
    println!(
        "{}",
        serde_json::json!({"build_seconds":builds,"pixels_per_second":rates,"checksum":checksum,"samples":262144,"iterations":16,"repeats":5,"scope":"warm process, deterministic full-domain CMYK, float RGB; no PDF parse/render; macOS arm64"})
    );
}
