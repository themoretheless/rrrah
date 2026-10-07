//! Warm-file synthetic RPF timing. Assertions run outside timed stages.
use rrrah_core::{MemoryBudget, RasterColorSpace};
use rrrah_decode::{DecodeRequest, RlaAlphaMode, RpfDecodeLimits, decode_rpf_file_with_budget};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        (2..=3).contains(&args.len()),
        "usage: rpf_lifecycle_timing FILE [varying]"
    );
    let varying = args.get(2).is_some_and(|value| value == "varying");
    let request = DecodeRequest::new(&args[1]);
    let limits = RpfDecodeLimits {
        max_node_names: 0,
        max_node_name_bytes: 0,
        max_row_layers: 0,
        max_total_layers: 0,
        max_output_bytes: 512 * 1024 * 1024,
    };
    let mut runs = Vec::new();
    for iteration in 0..6 {
        let root = MemoryBudget::new(512 * 1024 * 1024);
        let start = std::time::Instant::now();
        let image = decode_rpf_file_with_budget(&request, &root, limits).unwrap();
        let decode_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = std::time::Instant::now();
        let display = image
            .to_raster_with_interpretation(
                [0, 1, 2],
                Some(0),
                RlaAlphaMode::Straight,
                RasterColorSpace::LinearSrgb,
                &root,
                || false,
            )
            .unwrap();
        let display_ms = start.elapsed().as_secs_f64() * 1000.0;
        let plan = image.plan();
        let pixels = u64::from(plan.width) * u64::from(plan.height);
        for pixel in 0..pixels {
            for (channel, value) in [0.25, 1.0, 4.0, 0.5].into_iter().enumerate() {
                assert_eq!(image.main_sample(channel as u32, pixel), Some(value));
            }
        }
        for (channel, size) in rrrah_decode::RPF_GBUFFER_SAMPLE_BYTES.iter().enumerate() {
            let data = image.gbuffer_channel(channel).unwrap();
            assert_eq!(data.len(), pixels as usize * size);
            for (pixel, sample) in data.chunks_exact(*size).enumerate() {
                for (plane, value) in sample.iter().enumerate() {
                    assert_eq!(
                        *value,
                        ((channel * 13
                            + plane * 17
                            + if varying {
                                (pixel % plan.width as usize) * 37
                            } else {
                                0
                            })
                            & 255) as u8
                    );
                }
            }
        }
        let digest = blake3::hash(image.packed_bytes()).to_hex().to_string();
        let peak = root.peak();
        drop(display);
        drop(image);
        assert_eq!(root.used(), 0);
        runs.push(serde_json::json!({"iteration":iteration,"warmup":iteration==0,"decode_file_ms":decode_ms,"display_raster_ms":display_ms,"peak_managed_bytes":peak,"packed_bytes":plan.total_bytes,"packed_digest":digest,"pixels":pixels,"final_used":root.used()}));
    }
    println!("{}", serde_json::to_string_pretty(&runs).unwrap());
}
