//! Inspect and decode arbitrary RPF files without assuming display color meaning.
use rrrah_core::MemoryBudget;
use rrrah_decode::{DecodeRequest, RpfDecodeLimits, decode_rpf_file_with_budget};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 2, "usage: rpf_probe FILE");
    let root = MemoryBudget::new(512 * 1024 * 1024);
    let limits = RpfDecodeLimits {
        max_node_names: 65536,
        max_node_name_bytes: 4 * 1024 * 1024,
        max_row_layers: 1_000_000,
        max_total_layers: 8_000_000,
        max_output_bytes: 256 * 1024 * 1024,
    };
    let started = std::time::Instant::now();
    let image = match decode_rpf_file_with_budget(&DecodeRequest::new(&args[1]), &root, limits) {
        Ok(image) => image,
        Err(error) => {
            assert_eq!(root.used(), 0);
            eprintln!("{error:?}; peak_managed_bytes={}", root.peak());
            std::process::exit(1);
        }
    };
    let decode_ms = started.elapsed().as_secs_f64() * 1000.0;
    let plan = image.plan();
    let header = image.header();
    let channels: Vec<_> = (0..14)
        .filter_map(|index| {
            image.gbuffer_channel(index).map(|bytes| {
                serde_json::json!({"index":index,"bytes":bytes.len(),"blake3":blake3::hash(bytes).to_hex().as_str()})
            })
        })
        .collect();
    let report = serde_json::json!({
        "path":args[1], "decode_ms":decode_ms,
        "width":plan.width, "height":plan.height,
        "color_channels":header.color_channels, "matte_channels":header.matte_channels,
        "auxiliary_channels":header.auxiliary_channels, "main_channels":plan.main_channel_count,
        "main_float_bytes":plan.main_float_bytes, "gbuffer_bytes":plan.gbuffer_bytes,
        "layer_records":plan.layer_records, "layer_bytes":plan.layer_bytes,
        "metadata_bytes":plan.metadata_bytes, "packed_bytes":plan.total_bytes,
        "packed_blake3":blake3::hash(image.packed_bytes()).to_hex().as_str(),
        "channels":channels, "producer_pixel_aspect":format!("{:?}",image.producer_pixel_aspect()),
        "retained_managed_bytes":root.used(), "peak_managed_bytes":root.peak(),
        "color_interpretation":"unspecified; no inferred display conversion"
    });
    drop(image);
    assert_eq!(root.used(), 0);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"decode":report,"final_managed_bytes":root.used()}))
            .unwrap()
    );
}
