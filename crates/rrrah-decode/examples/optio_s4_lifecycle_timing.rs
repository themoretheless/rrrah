//! Warm-file sensor-only lifecycle timing with complete independent verification
//! outside each timed interval. Does not measure display or cold storage latency.
use rrrah_core::MemoryBudget;
use rrrah_decode::{DecodeRequest, read_optio_s4_sensor_with_budget, unpack_optio_s4_sensor};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: optio_s4_lifecycle_timing CAMERA_FILE ORACLE_U16LE".into());
    }
    let source = std::fs::read(&args[1])?;
    let oracle = std::fs::read(&args[2])?;
    let sensor_bytes =
        u64::from(rrrah_decode::OPTIO_S4_SENSOR_SIZE.0) * u64::from(rrrah_decode::OPTIO_S4_SENSOR_SIZE.1) * 2;
    assert_eq!(oracle.len() as u64, sensor_bytes);
    let verify = |samples: &[u16]| {
        assert_eq!(samples.len() * 2, oracle.len());
        assert!(
            samples
                .iter()
                .zip(oracle.chunks_exact(2))
                .all(|(&sample, pair)| { sample == u16::from_le_bytes([pair[0], pair[1]]) })
        );
    };
    let request = DecodeRequest::new(&args[1]);
    let mut runs = Vec::new();
    for iteration in 0..22 {
        let mut unpack_ms = 0.;
        let mut std_read_ms = 0.;
        let mut bounded_file_import_ms = 0.;
        for offset in 0..3 {
            match (iteration + offset) % 3 {
                0 => {
                    let root = MemoryBudget::new(sensor_bytes);
                    let start = std::time::Instant::now();
                    let sensor = unpack_optio_s4_sensor(&source, &root, || false)?;
                    unpack_ms = start.elapsed().as_secs_f64() * 1000.;
                    verify(&sensor);
                    assert_eq!(root.used(), sensor_bytes);
                    drop(sensor);
                    assert_eq!(root.used(), 0);
                }
                1 => {
                    let start = std::time::Instant::now();
                    let read = std::fs::read(&args[1])?;
                    std_read_ms = start.elapsed().as_secs_f64() * 1000.;
                    assert_eq!(read, source);
                }
                _ => {
                    let root = MemoryBudget::new(source.len() as u64 + sensor_bytes);
                    let start = std::time::Instant::now();
                    let sensor = read_optio_s4_sensor_with_budget(&request, &root)?;
                    bounded_file_import_ms = start.elapsed().as_secs_f64() * 1000.;
                    verify(&sensor);
                    assert_eq!(root.used(), sensor_bytes);
                    assert_eq!(root.peak(), source.len() as u64 + sensor_bytes);
                    drop(sensor);
                    assert_eq!(root.used(), 0);
                }
            }
        }
        runs.push(serde_json::json!({"iteration":iteration,"warmup":iteration<2,
            "borrowed_unpack_ms":unpack_ms,"unmanaged_std_read_ms":std_read_ms,
            "bounded_file_import_ms":bounded_file_import_ms,"root_peak_bytes":source.len() as u64 + sensor_bytes,
            "retained_sensor_bytes":sensor_bytes,"final_root_bytes":0}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "source_blake3":blake3::hash(&source).to_hex().to_string(),
            "oracle_blake3":blake3::hash(&oracle).to_hex().to_string(),
            "runs":runs
        }))?
    );
    Ok(())
}
