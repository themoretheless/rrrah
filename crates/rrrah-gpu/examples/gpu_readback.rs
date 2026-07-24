//! Manual headless readback dump: renders a gray ramp through the full color
//! pipeline and prints readback bytes next to the CPU reference curve, plus
//! an optional PPM frame dump for eyeballing.
//!
//! Environment knobs (all optional):
//! - `RRRAH_GPU_READBACK_DUMP=<path.ppm>` — write one rendered mid-gray frame
//!   as binary PPM (P6) for visual inspection.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

// Reuse the integration-test harness instead of duplicating it.
#[path = "../tests/common/mod.rs"]
mod common;

use common::{GpuReadback, cpu_reference_byte, uniform_mosaic};

const WHITE: f32 = 65_535.0;

fn main() {
    let Some(gpu) = GpuReadback::new() else {
        eprintln!("gpu_readback: no GPU adapter available");
        std::process::exit(2);
    };
    eprintln!("gpu_readback: adapter {}", gpu.adapter_name());

    println!("normalized,level_u16,readback_r,readback_g,readback_b,reference,delta_max");
    let mut mid_frame = None;
    for step in 0..=20_u32 {
        let normalized = f64::from(step) / 20.0;
        let level = (f64::from(WHITE) * normalized) as u16;
        let frame = gpu.render(&uniform_mosaic(256, 256, level, WHITE), [256, 256]);
        let [r, g, b, _] = frame.center();
        let reference = cpu_reference_byte(normalized);
        let delta = r
            .abs_diff(reference)
            .max(g.abs_diff(reference))
            .max(b.abs_diff(reference));
        println!("{normalized:.2},{level},{r},{g},{b},{reference},{delta}");
        if step == 10 {
            mid_frame = Some(frame);
        }
    }

    if let Ok(path) = std::env::var("RRRAH_GPU_READBACK_DUMP") {
        let frame = mid_frame.expect("mid-gray frame rendered at step 10");
        let mut ppm = format!("P6\n{} {}\n255\n", frame.width, frame.height).into_bytes();
        for pixel in frame.pixels.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        match std::fs::write(&path, &ppm) {
            Ok(()) => eprintln!("gpu_readback: wrote {path}"),
            Err(error) => eprintln!("gpu_readback: cannot write {path}: {error}"),
        }
    }
}
