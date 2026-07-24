//! Bounded scoped-worker executor for independent DNG segments.
//!
//! Tiles and strips are independent coded units: every segment carries its
//! own lossless-JPEG stream (own Huffman tables, predictor reset at the
//! segment start) or its own packed byte range, so segments decode
//! concurrently. Determinism is structural — every unit writes only into its
//! own precomputed output band, and failures are reported at the lowest unit
//! index — so the decoded mosaic is bit-identical for any worker count.

use std::{
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use super::DngError;

/// Runtime knob mirroring `RRRAH_CR3_PLANE_WORKERS`: overrides the DNG
/// segment-decode worker count. Any positive integer; clamped to the
/// available parallelism and the unit count at decode time.
pub(super) const WORKERS_ENV: &str = "RRRAH_DNG_DECODE_WORKERS";

/// Below this sample count the thread-spawn cost outweighs the parallel win.
const MIN_PARALLEL_SAMPLES: usize = 131_072;

/// Worker count from `RRRAH_DNG_DECODE_WORKERS`; defaults to the available
/// parallelism. Unlike the fixed four-plane CR3 batch, DNG segments number
/// in the hundreds, so scaling to all cores pays off.
pub(super) fn env_workers() -> usize {
    std::env::var(WORKERS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|workers| *workers >= 1)
        .unwrap_or_else(default_workers)
}

fn default_workers() -> usize {
    thread::available_parallelism().map_or(1, usize::from)
}

/// Resolves the effective worker count: single-threaded when the job is too
/// small to amortize spawn costs, otherwise the request clamped to hardware
/// and to the number of independent units.
pub(super) fn effective_workers(requested: usize, units: usize, samples: usize) -> usize {
    if units < 2 || samples < MIN_PARALLEL_SAMPLES {
        return 1;
    }
    let available = thread::available_parallelism().map_or(1, usize::from);
    requested.max(1).min(available).min(units)
}

/// Splits the output mosaic into one contiguous row band per strip (band
/// height = rows per strip) or per tile row (band height = tile height).
/// Band `index` covers rows `[index * band_height, ..)` clamped to the image
/// height, so each band is an exclusive `&mut` region.
pub(super) fn split_row_bands(
    output: &mut [u16],
    width: usize,
    height: usize,
    band_height: usize,
) -> Vec<&mut [u16]> {
    debug_assert!(band_height > 0);
    debug_assert_eq!(output.len(), width * height);
    let mut units = Vec::new();
    let mut rest = output;
    let mut first_row = 0;
    while first_row < height {
        let rows = band_height.min(height - first_row);
        let (band, tail) = rest.split_at_mut(rows * width);
        units.push(band);
        rest = tail;
        first_row += rows;
    }
    units
}

/// Unit processor shared by all workers: `(unit index, band)`.
#[allow(clippy::type_complexity)]
type UnitProcessor<'a> = dyn Fn(usize, &mut [u16]) -> Result<(), DngError> + Sync + 'a;

/// Runs `process(index, band)` for every unit on a bounded scoped worker set
/// (including the calling thread). With `workers <= 1` or a single unit the
/// loop runs inline — no threads are spawned for small files.
///
/// Every unit is processed exactly once even after failures, so the reported
/// error is the one at the lowest unit index, matching the sequential order
/// independently of scheduling.
pub(super) fn run_units(
    units: Vec<&mut [u16]>,
    workers: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
    process: &UnitProcessor<'_>,
) -> Result<(), DngError> {
    let unit_count = units.len();
    if workers <= 1 || unit_count < 2 {
        for (index, unit) in units.into_iter().enumerate() {
            process(index, unit)?;
        }
        if cancelled() {
            // No unit observed the cancellation; surface it deterministically.
            return Err(DngError::Cancelled { row: 0 });
        }
        return Ok(());
    }

    let slots: Vec<Mutex<Option<&mut [u16]>>> =
        units.into_iter().map(|unit| Mutex::new(Some(unit))).collect();
    let next = AtomicUsize::new(0);
    let failure: Mutex<Option<(usize, DngError)>> = Mutex::new(None);

    let worker = || loop {
        if cancelled() {
            break;
        }
        let index = next.fetch_add(1, Ordering::Relaxed);
        if index >= unit_count {
            break;
        }
        let unit = slots[index]
            .lock()
            .expect("unit slot lock is never poisoned")
            .take()
            .expect("each unit is claimed exactly once");
        if let Err(error) = process(index, unit) {
            record_failure(&failure, index, error);
        }
    };

    thread::scope(|scope| {
        // A spawn failure degrades to fewer workers, never to a failed
        // decode: the caller thread claims the remaining units itself.
        let shared_worker = &worker;
        for worker_index in 1..workers {
            let spawned = thread::Builder::new()
                .name(format!("rrrah-dng-segment-{worker_index}"))
                .spawn_scoped(scope, shared_worker);
            if spawned.is_err() {
                break;
            }
        }
        worker();
    });

    if let Some((_, error)) = failure.lock().expect("failure lock is never poisoned").take() {
        return Err(error);
    }
    if cancelled() {
        // Cancellation noticed between units, before any unit observed it.
        return Err(DngError::Cancelled { row: 0 });
    }
    Ok(())
}

fn record_failure(failure: &Mutex<Option<(usize, DngError)>>, index: usize, error: DngError) {
    let mut guard = failure.lock().expect("failure lock is never poisoned");
    if guard.as_ref().is_none_or(|(existing, _)| index < *existing) {
        *guard = Some((index, error));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_workers_stays_sequential_for_small_jobs() {
        assert_eq!(effective_workers(8, 1, 1_000_000), 1);
        assert_eq!(effective_workers(8, 64, MIN_PARALLEL_SAMPLES - 1), 1);
    }

    #[test]
    fn effective_workers_clamps_to_hardware_and_units() {
        let available = thread::available_parallelism().map_or(1, usize::from);
        assert_eq!(effective_workers(1024, 3, 1_000_000), 3.min(available));
        assert_eq!(effective_workers(0, 64, 1_000_000), 1);
    }

    #[test]
    fn split_row_bands_covers_the_output_exactly_once() {
        let mut output: Vec<u16> = (0..7 * 11).collect();
        let bands = split_row_bands(&mut output, 11, 7, 3);
        assert_eq!(bands.len(), 3);
        assert_eq!(bands[0].len(), 33);
        assert_eq!(bands[1].len(), 33);
        assert_eq!(bands[2].len(), 11);
        let mut expected = 0;
        for band in bands {
            for value in band.iter() {
                assert_eq!(*value, expected);
                expected += 1;
            }
        }
    }

    #[test]
    fn run_units_processes_every_unit_once_at_any_worker_count() {
        for workers in [1, 2, 4, 16] {
            let mut output = vec![0_u16; 40];
            let units = split_row_bands(&mut output, 4, 10, 1);
            let process = |index: usize, unit: &mut [u16]| -> Result<(), DngError> {
                unit.fill(u16::try_from(index).unwrap());
                Ok(())
            };
            run_units(units, workers, &|| false, &process).unwrap();
            let expected: Vec<u16> = (0..10).flat_map(|index| [index; 4]).collect();
            assert_eq!(output, expected, "workers={workers}");
        }
    }

    #[test]
    fn run_units_reports_the_lowest_index_failure() {
        let mut output = vec![0_u16; 40];
        let units = split_row_bands(&mut output, 4, 10, 1);
        let process = |index: usize, _unit: &mut [u16]| -> Result<(), DngError> {
            if index >= 3 {
                return Err(DngError::Cancelled { row: index });
            }
            Ok(())
        };
        let error = run_units(units, 4, &|| false, &process).unwrap_err();
        assert!(matches!(error, DngError::Cancelled { row: 3 }));
    }

    #[test]
    fn run_units_surfaces_cancellation_without_units_observing_it() {
        let mut output = vec![0_u16; 4];
        let units = split_row_bands(&mut output, 4, 1, 1);
        let process = |_index: usize, _unit: &mut [u16]| -> Result<(), DngError> { Ok(()) };
        let error = run_units(units, 2, &|| true, &process).unwrap_err();
        assert!(matches!(error, DngError::Cancelled { row: 0 }));
    }
}
