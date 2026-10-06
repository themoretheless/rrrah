//! Warm-OS CPU decode versus real session-swap restore. Excludes GPU/presentation.
#[path = "../src/model_swap.rs"]
mod model_swap;
use rrrah_cache::{CacheLimits, ImageSwapCache, ImageSwapConfig, SwapPayload};
use rrrah_core::MemoryBudget;
use std::{
    error::Error,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    time::Instant,
};
struct Compare(std::io::BufReader<File>);
impl Write for Compare {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let mut block = [0; 4096];
        for part in bytes.chunks(block.len()) {
            self.0.read_exact(&mut block[..part.len()])?;
            if part != &block[..part.len()] {
                return Err(std::io::Error::other("payload mismatch"));
            }
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn verify(payload: &model_swap::ModelPayload, reference: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let mut compare = Compare(std::io::BufReader::with_capacity(
        64 * 1024,
        File::open(reference)?,
    ));
    payload.write_payload(&mut compare)?;
    if compare.0.read(&mut [0])? != 0 {
        return Err("short payload".into());
    }
    Ok(())
}
fn percentile(values: &[f64], percent: usize) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[(sorted.len() * percent).div_ceil(100) - 1]
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().ok_or("usage: model_swap_timing MODEL [ROUNDS]")?);
    let rounds = args
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(8);
    if rounds == 0 || rounds > 1000 || args.next().is_some() {
        return Err("rounds must be 1..1000".into());
    }
    let directory = tempfile::tempdir()?;
    let reference = directory.path().join("reference.bin");
    let root = MemoryBudget::new(256 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&path);
    request.memory_budget = Some(root.clone());
    let original = model_swap::ModelPayload::from_model(rrrah_decode::decode_model(&request)?)
        .ok_or("unsupported swap model")?;
    let mut reference_writer = std::io::BufWriter::with_capacity(64 * 1024, File::create(&reference)?);
    original.write_payload(&mut reference_writer)?;
    reference_writer.flush()?;
    drop(reference_writer);
    let payload_bytes = original.payload_len()?;
    let resident = original.capacity_bytes();
    let cache: ImageSwapCache<u64, model_swap::ModelPayload> = ImageSwapCache::new_with_budgets(
        directory.path(),
        ImageSwapConfig {
            limits: CacheLimits {
                max_bytes: 256 * 1024 * 1024,
                max_entries: Some(1),
                ttl: None,
            },
            queue_bytes: 256 * 1024 * 1024,
            queue_count: 1,
            restore_bytes: 256 * 1024 * 1024,
        },
        MemoryBudget::new(256 * 1024 * 1024),
        root.clone(),
    )?;
    let write_start = Instant::now();
    cache.enqueue(1, original);
    cache.wait_idle()?;
    let write_ms = write_start.elapsed().as_secs_f64() * 1000.;
    if cache.stats().writes != 1 || root.used() != 0 {
        return Err("spill/admission or source-release failure".into());
    }
    // Independently admitted diagnostic input; production swap never buffers
    // the complete payload this way.
    let payload_root = MemoryBudget::new(payload_bytes);
    let mut memory_source = payload_root.try_buffer(usize::try_from(payload_bytes)?, 0_u8)?;
    File::open(&reference)?.read_exact(&mut memory_source)?;
    let memory_source = memory_source.freeze();
    println!("tier,iteration,elapsed_ms,resident_bytes");
    let mut decode = Vec::new();
    let mut swap = Vec::new();
    let mut codec = Vec::new();
    let mut memory_codec = Vec::new();
    let mut buffered_codec = Vec::new();
    // One warmup round; subsequent rounds rotate which of five tiers goes first.
    for iteration in 0..=rounds {
        let tiers = ["decode", "swap", "codec_file", "codec_memory", "codec_buffered"];
        for offset in 0..tiers.len() {
            let tier = tiers[(iteration + offset) % tiers.len()];
            let start = Instant::now();
            let value = if tier == "decode" {
                model_swap::ModelPayload::from_model(rrrah_decode::decode_model(&request)?)
                    .ok_or("unsupported model")?
            } else if tier == "swap" {
                cache.try_get(&1, || false)?.ok_or("swap entry missing")?
            } else if tier == "codec_memory" {
                model_swap::ModelPayload::read_payload(&mut &memory_source[..], payload_bytes, &root)?
            } else if tier == "codec_buffered" {
                let mut reader = std::io::BufReader::with_capacity(64 * 1024, File::open(&reference)?);
                model_swap::ModelPayload::read_payload(&mut reader, payload_bytes, &root)?
            } else {
                // Diagnostic tier excludes store integrity/cancellation and must
                // never substitute for production swap restore.
                model_swap::ModelPayload::read_payload(&mut File::open(&reference)?, payload_bytes, &root)?
            };
            let elapsed = start.elapsed().as_secs_f64() * 1000.;
            let capacity = value.capacity_bytes();
            if root.used() != capacity {
                return Err("retained capacity does not match credit".into());
            }
            // Full byte verification is deliberately outside the timed interval.
            verify(&value, &reference)?;
            drop(value);
            if root.used() != 0 {
                return Err("last-owner release failure".into());
            }
            if iteration > 0 {
                println!("{tier},{iteration},{elapsed:.6},{capacity}");
                if tier == "decode" {
                    decode.push(elapsed)
                } else if tier == "swap" {
                    swap.push(elapsed)
                } else if tier == "codec_file" {
                    codec.push(elapsed)
                } else if tier == "codec_buffered" {
                    buffered_codec.push(elapsed)
                } else {
                    memory_codec.push(elapsed)
                }
            }
        }
    }
    drop(memory_source);
    eprintln!(
        "codec_buffered_p50_ms={:.6},codec_buffered_p95_ms={:.6}",
        percentile(&buffered_codec, 50),
        percentile(&buffered_codec, 95)
    );
    if payload_root.used() != 0 {
        return Err("diagnostic source release failure".into());
    }
    eprintln!(
        "source={},payload_bytes={payload_bytes},decoded_capacity={resident},write_ms={write_ms:.6},root_peak={},root_used={},decode_p50_ms={:.6},decode_p95_ms={:.6},swap_p50_ms={:.6},swap_p95_ms={:.6},codec_file_p50_ms={:.6},codec_file_p95_ms={:.6},codec_memory_p50_ms={:.6},codec_memory_p95_ms={:.6},diagnostic_source_peak={},diagnostic_source_used={},reads={},errors={}",
        path.display(),
        root.peak(),
        root.used(),
        percentile(&decode, 50),
        percentile(&decode, 95),
        percentile(&swap, 50),
        percentile(&swap, 95),
        percentile(&codec, 50),
        percentile(&codec, 95),
        percentile(&memory_codec, 50),
        percentile(&memory_codec, 95),
        payload_root.peak(),
        payload_root.used(),
        cache.stats().reads,
        cache.stats().errors
    );
    if cache.stats().reads != (rounds + 1) as u64 || cache.stats().errors != 0 {
        return Err("unexpected swap statistics".into());
    }
    Ok(())
}
