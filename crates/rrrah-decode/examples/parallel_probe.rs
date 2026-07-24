//! Temporary scaling probe for the parallel DNG segment decoder.
//! Measures parse / pixel-unpack breakdown across worker counts.

use std::hint::black_box;
use std::time::{Duration, Instant};

use rrrah_decode::dng::bench_support::{
    SegmentLayout, SyntheticCompression, build_segmented_dng, decode_dng_pixels_timed,
};

fn gradient_samples(width: usize, height: usize) -> Vec<u16> {
    let mut state = 0x1234_5678_9abc_def0_u64;
    let mut next = move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_f491_4f6c_dd1d)
    };
    let mut samples = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let base = 2048 + i32::try_from((x * 3 + y * 5) % 1024).unwrap();
            let noise = (next() & 0x3f) as i32 - 32;
            samples.push(u16::try_from((base + noise).clamp(0, 4095)).unwrap());
        }
    }
    samples
}

fn main() {
    let (width, height) = (6000_u32, 4000_u32);
    let samples = gradient_samples(width as usize, height as usize);
    let dng = build_segmented_dng(
        width,
        height,
        SegmentLayout::Tiles {
            tile_width: 256,
            tile_height: 256,
        },
        SyntheticCompression::LosslessJpeg12,
        &samples,
    );
    println!("dng bytes: {}", dng.len());

    let rounds = 5;
    let mut reference = 0_u64;
    for workers in [1_usize, 2, 3, 4, 6, 8, 10] {
        let mut parse_times = Vec::new();
        let mut unpack_times = Vec::new();
        let mut totals = Vec::new();
        for _ in 0..rounds {
            let started = Instant::now();
            let (checksum, timing) = decode_dng_pixels_timed(black_box(&dng), workers).unwrap();
            totals.push(started.elapsed());
            parse_times.push(timing.parse);
            unpack_times.push(timing.pixel_unpack);
            if workers == 1 {
                reference = checksum;
            } else {
                assert_eq!(checksum, reference, "bit-identity at {workers} workers");
            }
        }
        let min = |times: &[Duration]| times.iter().min().copied().unwrap().as_secs_f64() * 1e3;
        println!(
            "workers={workers:2}  total_min={:.1} ms  parse_min={:.2} ms  unpack_min={:.1} ms",
            min(&totals),
            min(&parse_times),
            min(&unpack_times),
        );
    }
}
