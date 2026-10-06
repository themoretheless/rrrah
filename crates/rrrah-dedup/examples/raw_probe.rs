//! Full-resolution RAW integration probe; sensor dimensions come from an oracle.
use rrrah_core::MemoryBudget;
use rrrah_decode::{DecodeRequest, DecodedImage};
use rrrah_dedup::decode::decode_selected_frame;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if !(3..=5).contains(&arguments.len()) {
        return Err(
            "usage: raw_probe FILE SENSOR_WIDTH SENSOR_HEIGHT [BUDGET_MIB [--verify-failure-retry|--development-only]]".into(),
        );
    }
    let verify_failure_retry = arguments
        .get(4)
        .is_some_and(|flag| flag == "--verify-failure-retry");
    let development_only = arguments.get(4).is_some_and(|flag| flag == "--development-only");
    if arguments.len() == 5 && !verify_failure_retry && !development_only {
        return Err("unknown verification flag".into());
    }
    let expected = (arguments[1].parse::<u32>()?, arguments[2].parse::<u32>()?);
    let budget_mib = arguments
        .get(3)
        .map(|v| v.parse::<u64>())
        .transpose()?
        .unwrap_or(1024);
    let budget_bytes = budget_mib.checked_mul(1024 * 1024).ok_or("budget overflow")?;
    let budget = MemoryBudget::new(budget_bytes);
    let mut request = DecodeRequest::new(&arguments[0]);
    if verify_failure_retry {
        let source_bytes = std::fs::metadata(&request.path)?.len();
        let sensor_bytes = u64::from(expected.0)
            .checked_mul(u64::from(expected.1))
            .and_then(|count| count.checked_mul(2))
            .ok_or("sensor size overflow")?;
        let partial_limit = source_bytes
            .checked_add(sensor_bytes)
            .and_then(|bytes| bytes.checked_add(1024 * 1024))
            .ok_or("partial budget overflow")?;
        for limit in [0, partial_limit] {
            let denied = MemoryBudget::new(limit);
            request.memory_budget = Some(denied.clone());
            let error = decode_selected_frame(&request, 100_000_000, &denied, || false)
                .expect_err("insufficient managed budget must refuse RAW development");
            let reason = error.to_string().to_ascii_lowercase();
            if !reason.contains("budget") && !reason.contains("memory") {
                return Err(format!("expected resource refusal, got {error}").into());
            }
            if denied.used() != 0 {
                return Err("managed reservation leaked after refused RAW development".into());
            }
            if limit > 0 && denied.peak() == 0 {
                return Err("partial-budget check did not admit any managed reservation".into());
            }
        }
    }
    request.memory_budget = Some(budget.clone());
    if verify_failure_retry {
        let error = decode_selected_frame(&request, 100_000_000, &budget, || budget.peak() > 0)
            .expect_err("cancellation after managed admission must refuse RAW development");
        if !error.to_string().to_ascii_lowercase().contains("cancel") {
            return Err(format!("expected cancellation, got {error}").into());
        }
        if budget.peak() == 0 || budget.used() != 0 {
            return Err("cancelled RAW development must admit then release managed memory".into());
        }
    }
    if !development_only {
        let DecodedImage::Sensor(sensor) = rrrah_decode::decode_image(&request)? else {
            return Err("RAW routed as raster".into());
        };
        let actual = (sensor.mosaic.metadata.width, sensor.mosaic.metadata.height);
        if actual != expected {
            return Err(format!("sensor dimensions {actual:?} != oracle {expected:?}").into());
        }
        drop(sensor);
    }
    let start = std::time::Instant::now();
    let developed = decode_selected_frame(&request, 100_000_000, &budget, || false)?;
    let view = developed.view(|| false)?;
    let (width, height) = view.dimensions();
    let digest = view.pixel_digest(|| false)?;
    let fingerprint = view.fingerprint(|| false)?;
    let informative = fingerprint.luminance_stddev > 0.001;
    let digest = blake3::Hash::from(digest).to_hex();
    let milliseconds = start.elapsed().as_millis();
    drop(developed);
    if budget.used() != 0 {
        return Err("managed memory retained after probe".into());
    }
    println!(
        "{{\"sensor_width\":{},\"sensor_height\":{},\"width\":{width},\"height\":{height},\"digest\":\"{digest}\",\"informative\":{informative},\"milliseconds\":{milliseconds},\"managed_peak\":{},\"budget_refusal_verified\":{verify_failure_retry},\"sensor_dimensions_verified\":{}}}",
        expected.0,
        expected.1,
        budget.peak(),
        !development_only
    );
    Ok(())
}
