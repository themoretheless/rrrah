//! Bounded four-plane decode scheduling and deterministic Bayer assembly.
//!
//! EOS R8 CRX stores the four Bayer parities as independent entropy streams:
//! plane 0 is `(even y, even x)`, plane 1 is `(even y, odd x)`, plane 2 is
//! `(odd y, even x)`, and plane 3 is `(odd y, odd x)`.  Entropy decoding is
//! therefore embarrassingly parallel. The production path sends bounded
//! 32-row batches to the caller for overlapped RGGB assembly; a full-plane
//! scheduler remains as the low-parallelism fallback.
//!
//! Neither scheduler has a private thread pool or an async-runtime dependency.
//! Scoped workers borrow the plane chunks and are joined before return. The
//! application serializes foreground and prefetch RAW decoding through its
//! decode gate, so these bounded scopes do not create nested persistent pools.
//!
//! Cancellation is cooperative.  It is checked before a plane is claimed and
//! exposed to the plane decoder as a callback, which the entropy loop should
//! poll at least once per decoded row.  A scoped thread cannot be abandoned:
//! after cancellation or the first error, this function signals every active
//! decoder and joins all workers before returning.

use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{Receiver, Sender, channel, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};

const PLANE_COUNT: usize = 4;
const DEFAULT_STREAM_ROWS_PER_BATCH: usize = 32;
const DEFAULT_STREAM_QUEUE_DEPTH: usize = 1;
const STREAM_ROWS_ENV: &str = "RRRAH_CR3_STREAM_BATCH_ROWS";
const STREAM_QUEUE_DEPTH_ENV: &str = "RRRAH_CR3_STREAM_QUEUE_DEPTH";
const STREAM_ROW_OPTIONS: [usize; 5] = [8, 16, 32, 64, 128];
const STREAM_QUEUE_DEPTH_OPTIONS: [usize; 3] = [1, 2, 4];

/// A generous hard ceiling for the one allocation performed by interleave.
///
/// Native parsing currently narrows the accepted profile to the 25.5 MP EOS
/// R8, but keeping this helper independently bounded prevents a later caller
/// from turning untrusted dimensions into an unbounded allocation.
const MAX_MOSAIC_PIXELS: usize = 128 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct PlaneDecodeBatch {
    pub(crate) planes: [Vec<u16>; PLANE_COUNT],
    pub(crate) plane_elapsed: [Duration; PLANE_COUNT],
    pub(crate) wall_elapsed: Duration,
    pub(crate) worker_count: usize,
}

#[derive(Debug)]
pub(crate) struct StreamingDecodeBatch {
    pub(crate) pixels: Vec<u16>,
    pub(crate) plane_elapsed: [Duration; PLANE_COUNT],
    pub(crate) wall_elapsed: Duration,
    pub(crate) interleave_elapsed: Duration,
    pub(crate) worker_count: usize,
}

#[derive(Debug)]
pub(crate) enum ParallelDecodeError<E> {
    Cancelled,
    Plane {
        plane_index: usize,
        elapsed: Duration,
        source: E,
    },
    PlanePanicked {
        plane_index: usize,
        elapsed: Duration,
    },
    WorkerSpawn {
        kind: std::io::ErrorKind,
    },
    WorkerPanicked,
    Incomplete {
        plane_index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for ParallelDecodeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("parallel CRX plane decode was cancelled"),
            Self::Plane {
                plane_index,
                elapsed,
                source,
            } => write!(
                formatter,
                "CRX plane {plane_index} decode failed after {elapsed:.2?}: {source}"
            ),
            Self::PlanePanicked { plane_index, elapsed } => write!(
                formatter,
                "CRX plane {plane_index} decoder panicked after {elapsed:.2?}"
            ),
            Self::WorkerSpawn { kind } => {
                write!(formatter, "could not start a scoped CRX plane worker: {kind}")
            }
            Self::WorkerPanicked => formatter.write_str("a scoped CRX plane worker panicked"),
            Self::Incomplete { plane_index } => {
                write!(formatter, "CRX plane {plane_index} produced no decode result")
            }
        }
    }
}

impl<E> Error for ParallelDecodeError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Plane { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum StreamingDecodeError<E> {
    Parallel(ParallelDecodeError<E>),
    Interleave(InterleaveError),
    RowChannelClosed {
        plane_index: usize,
        row: usize,
    },
    RowOrder {
        plane_index: usize,
        expected: usize,
        actual: usize,
    },
    RowLength {
        plane_index: usize,
        row: usize,
        expected: usize,
        actual: usize,
    },
    RowCount {
        plane_index: usize,
        expected: usize,
        actual: usize,
    },
}

impl<E: fmt::Display> fmt::Display for StreamingDecodeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parallel(error) => error.fmt(formatter),
            Self::Interleave(error) => error.fmt(formatter),
            Self::RowChannelClosed { plane_index, row } => write!(
                formatter,
                "CRX plane {plane_index} stopped before streamed row {row}"
            ),
            Self::RowOrder {
                plane_index,
                expected,
                actual,
            } => write!(
                formatter,
                "CRX plane {plane_index} streamed row {actual}, expected row {expected}"
            ),
            Self::RowLength {
                plane_index,
                row,
                expected,
                actual,
            } => write!(
                formatter,
                "CRX plane {plane_index} row {row} has {actual} samples, expected {expected}"
            ),
            Self::RowCount {
                plane_index,
                expected,
                actual,
            } => write!(
                formatter,
                "CRX plane {plane_index} streamed {actual} rows, expected {expected}"
            ),
        }
    }
}

impl<E> Error for StreamingDecodeError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Parallel(error) => Some(error),
            Self::Interleave(error) => Some(error),
            _ => None,
        }
    }
}

impl<E> From<InterleaveError> for StreamingDecodeError<E> {
    fn from(error: InterleaveError) -> Self {
        Self::Interleave(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InterleaveError {
    EmptyGeometry,
    OddGeometry {
        width: u32,
        height: u32,
    },
    GeometryOverflow,
    PixelLimit {
        pixels: usize,
        limit: usize,
    },
    PlaneLength {
        plane_index: usize,
        expected: usize,
        actual: usize,
    },
    Allocation {
        pixels: usize,
    },
}

impl fmt::Display for InterleaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyGeometry => formatter.write_str("CRX mosaic geometry is empty"),
            Self::OddGeometry { width, height } => write!(
                formatter,
                "CRX four-plane mosaic geometry must be even, got {width}x{height}"
            ),
            Self::GeometryOverflow => formatter.write_str("CRX mosaic geometry overflows this platform"),
            Self::PixelLimit { pixels, limit } => write!(
                formatter,
                "CRX mosaic has {pixels} pixels, above the {limit}-pixel safety limit"
            ),
            Self::PlaneLength {
                plane_index,
                expected,
                actual,
            } => write!(
                formatter,
                "CRX plane {plane_index} has {actual} samples, expected {expected}"
            ),
            Self::Allocation { pixels } => {
                write!(formatter, "could not allocate a {pixels}-sample CRX mosaic")
            }
        }
    }
}

impl Error for InterleaveError {}

/// Decodes four borrowed plane chunks with a bounded scoped worker set.
///
/// `decode_plane` receives the stable plane index, its compressed bytes, and
/// a cancellation callback.  Successful results are always returned in plane
/// index order, independently of scheduling order.
pub(crate) fn decode_four_planes<'a, F, E, C>(
    plane_data: [&'a [u8]; PLANE_COUNT],
    requested_workers: usize,
    cancelled: &C,
    decode_plane: &F,
) -> Result<PlaneDecodeBatch, ParallelDecodeError<E>>
where
    F: Fn(usize, &'a [u8], &dyn Fn() -> bool) -> Result<Vec<u16>, E> + Sync,
    E: Send,
    C: Fn() -> bool + Sync,
{
    let available = thread::available_parallelism().map_or(1, usize::from);
    decode_four_planes_with_parallelism(plane_data, requested_workers, available, cancelled, decode_plane)
}

fn decode_four_planes_with_parallelism<'a, F, E, C>(
    plane_data: [&'a [u8]; PLANE_COUNT],
    requested_workers: usize,
    available_workers: usize,
    cancelled: &C,
    decode_plane: &F,
) -> Result<PlaneDecodeBatch, ParallelDecodeError<E>>
where
    F: Fn(usize, &'a [u8], &dyn Fn() -> bool) -> Result<Vec<u16>, E> + Sync,
    E: Send,
    C: Fn() -> bool + Sync,
{
    if cancelled() {
        return Err(ParallelDecodeError::Cancelled);
    }

    let worker_count = requested_workers
        .max(1)
        .min(available_workers.max(1))
        .min(PLANE_COUNT);
    let started = Instant::now();
    let next_plane = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);

    let worker_results = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count.saturating_sub(1));
        for worker_index in 1..worker_count {
            let handle = thread::Builder::new()
                .name(format!("rrrah-crx-plane-{worker_index}"))
                .spawn_scoped(scope, || {
                    catch_unwind(AssertUnwindSafe(|| {
                        decode_worker(&next_plane, &stop, plane_data, cancelled, decode_plane)
                    }))
                })
                .map_err(|error| {
                    stop.store(true, Ordering::Release);
                    ParallelDecodeError::WorkerSpawn { kind: error.kind() }
                })?;
            handles.push(handle);
        }

        let caller_results = catch_unwind(AssertUnwindSafe(|| {
            decode_worker(&next_plane, &stop, plane_data, cancelled, decode_plane)
        }))
        .map_err(|_| ParallelDecodeError::WorkerPanicked)?;
        let mut results = caller_results;

        for handle in handles {
            let mut worker_results = handle
                .join()
                .map_err(|_| ParallelDecodeError::WorkerPanicked)?
                .map_err(|_| ParallelDecodeError::WorkerPanicked)?;
            results.append(&mut worker_results);
        }
        Ok(results)
    })?;

    if cancelled() {
        return Err(ParallelDecodeError::Cancelled);
    }

    let mut outcomes: [Option<PlaneOutcome<E>>; PLANE_COUNT] = std::array::from_fn(|_| None);
    for (plane_index, outcome) in worker_results {
        if plane_index < PLANE_COUNT {
            outcomes[plane_index] = Some(outcome);
        }
    }

    for (plane_index, outcome) in outcomes.iter_mut().enumerate() {
        if matches!(
            outcome.as_ref(),
            Some(PlaneOutcome::Failed { .. } | PlaneOutcome::Panicked { .. })
        ) {
            return match outcome.take() {
                Some(PlaneOutcome::Failed { source, elapsed }) => Err(ParallelDecodeError::Plane {
                    plane_index,
                    elapsed,
                    source,
                }),
                Some(PlaneOutcome::Panicked { elapsed }) => {
                    Err(ParallelDecodeError::PlanePanicked { plane_index, elapsed })
                }
                _ => Err(ParallelDecodeError::Incomplete { plane_index }),
            };
        }
    }

    let mut planes = Vec::with_capacity(PLANE_COUNT);
    let mut plane_elapsed = [Duration::ZERO; PLANE_COUNT];
    for (plane_index, outcome) in outcomes.into_iter().enumerate() {
        match outcome {
            Some(PlaneOutcome::Decoded { pixels, elapsed }) => {
                planes.push(pixels);
                plane_elapsed[plane_index] = elapsed;
            }
            Some(PlaneOutcome::Failed { .. } | PlaneOutcome::Panicked { .. }) => {
                return Err(ParallelDecodeError::Incomplete { plane_index });
            }
            None => return Err(ParallelDecodeError::Incomplete { plane_index }),
        }
    }

    let planes = planes
        .try_into()
        .map_err(|_| ParallelDecodeError::Incomplete { plane_index: 0 })?;
    Ok(PlaneDecodeBatch {
        planes,
        plane_elapsed,
        wall_elapsed: started.elapsed(),
        worker_count,
    })
}

fn decode_worker<'a, F, E, C>(
    next_plane: &AtomicUsize,
    stop: &AtomicBool,
    plane_data: [&'a [u8]; PLANE_COUNT],
    cancelled: &C,
    decode_plane: &F,
) -> Vec<(usize, PlaneOutcome<E>)>
where
    F: Fn(usize, &'a [u8], &dyn Fn() -> bool) -> Result<Vec<u16>, E> + Sync,
    C: Fn() -> bool + Sync,
{
    let mut results = Vec::new();
    loop {
        if stop.load(Ordering::Acquire) || cancelled() {
            break;
        }

        let plane_index = next_plane.fetch_add(1, Ordering::Relaxed);
        if plane_index >= PLANE_COUNT {
            break;
        }
        if stop.load(Ordering::Acquire) || cancelled() {
            break;
        }

        let plane_started = Instant::now();
        let should_cancel = || stop.load(Ordering::Acquire) || cancelled();
        let result = catch_unwind(AssertUnwindSafe(|| {
            decode_plane(plane_index, plane_data[plane_index], &should_cancel)
        }));
        let elapsed = plane_started.elapsed();

        let outcome = match result {
            Ok(Ok(pixels)) => PlaneOutcome::Decoded { pixels, elapsed },
            Ok(Err(source)) => {
                stop.store(true, Ordering::Release);
                PlaneOutcome::Failed { source, elapsed }
            }
            Err(_) => {
                stop.store(true, Ordering::Release);
                PlaneOutcome::Panicked { elapsed }
            }
        };
        results.push((plane_index, outcome));
    }
    results
}

#[derive(Debug)]
enum PlaneOutcome<E> {
    Decoded { pixels: Vec<u16>, elapsed: Duration },
    Failed { source: E, elapsed: Duration },
    Panicked { elapsed: Duration },
}

#[derive(Debug, Clone, Copy)]
struct MosaicGeometry {
    pixel_count: usize,
    plane_width: usize,
    plane_height: usize,
    plane_pixels: usize,
}

fn checked_mosaic_geometry(sensor_width: u32, sensor_height: u32) -> Result<MosaicGeometry, InterleaveError> {
    if sensor_width == 0 || sensor_height == 0 {
        return Err(InterleaveError::EmptyGeometry);
    }
    if !sensor_width.is_multiple_of(2) || !sensor_height.is_multiple_of(2) {
        return Err(InterleaveError::OddGeometry {
            width: sensor_width,
            height: sensor_height,
        });
    }

    let width = usize::try_from(sensor_width).map_err(|_| InterleaveError::GeometryOverflow)?;
    let height = usize::try_from(sensor_height).map_err(|_| InterleaveError::GeometryOverflow)?;
    let pixel_count = width
        .checked_mul(height)
        .ok_or(InterleaveError::GeometryOverflow)?;
    if pixel_count > MAX_MOSAIC_PIXELS {
        return Err(InterleaveError::PixelLimit {
            pixels: pixel_count,
            limit: MAX_MOSAIC_PIXELS,
        });
    }

    let plane_width = width / 2;
    let plane_height = height / 2;
    let plane_pixels = plane_width
        .checked_mul(plane_height)
        .ok_or(InterleaveError::GeometryOverflow)?;
    Ok(MosaicGeometry {
        pixel_count,
        plane_width,
        plane_height,
        plane_pixels,
    })
}

fn allocate_mosaic(pixel_count: usize) -> Result<Vec<u16>, InterleaveError> {
    let mut mosaic = Vec::new();
    mosaic
        .try_reserve_exact(pixel_count)
        .map_err(|_| InterleaveError::Allocation { pixels: pixel_count })?;
    Ok(mosaic)
}

#[derive(Debug)]
struct StreamedRows {
    first_row: usize,
    row_count: usize,
    samples: Vec<u16>,
}

#[derive(Debug)]
enum StreamPlaneOutcome<E> {
    Decoded {
        elapsed: Duration,
        coordination_wait: Duration,
    },
    Failed {
        source: E,
        elapsed: Duration,
    },
    Panicked {
        elapsed: Duration,
    },
    Protocol(StreamProtocolFailure),
    Stopped,
}

#[derive(Debug)]
enum StreamProtocolFailure {
    Order {
        expected: usize,
        actual: usize,
    },
    Length {
        row: usize,
        expected: usize,
        actual: usize,
    },
    Count {
        expected: usize,
        actual: usize,
    },
}

#[derive(Debug)]
enum AssemblyFailure {
    Cancelled,
    ChannelClosed {
        plane_index: usize,
        row: usize,
    },
    RowOrder {
        plane_index: usize,
        expected: usize,
        actual: usize,
    },
    RowLength {
        plane_index: usize,
        row: usize,
        expected: usize,
        actual: usize,
    },
}

fn stream_tuning_value(name: &str, default: usize, allowed: &[usize]) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| allowed.contains(value))
        .unwrap_or(default)
}

/// Decodes all four parity planes while assembling completed rows directly
/// into the final mosaic.
///
/// By default each plane owns two reusable batches of at most 32 rows. A
/// capacity-one channel hands one batch to the coordinator while entropy
/// decode fills the other. The benchmark-only environment overrides below
/// expand that fixed pool to `queue depth + 1` buffers per plane so queue-depth
/// experiments do not accidentally benchmark a starved two-buffer pool.
#[allow(clippy::too_many_lines)]
pub(crate) fn decode_four_planes_streaming<'a, F, E, C>(
    plane_data: [&'a [u8]; PLANE_COUNT],
    sensor_width: u32,
    sensor_height: u32,
    cancelled: &C,
    decode_plane_rows: &F,
) -> Result<StreamingDecodeBatch, StreamingDecodeError<E>>
where
    F: Fn(
            usize,
            &'a [u8],
            &dyn Fn() -> bool,
            &mut dyn FnMut(usize, Vec<u16>) -> Option<Vec<u16>>,
        ) -> Result<(), E>
        + Sync,
    E: Send,
    C: Fn() -> bool + Sync,
{
    if cancelled() {
        return Err(StreamingDecodeError::Parallel(ParallelDecodeError::Cancelled));
    }

    let geometry = checked_mosaic_geometry(sensor_width, sensor_height)?;
    let mut mosaic = allocate_mosaic(geometry.pixel_count)?;
    let rows_per_batch = geometry.plane_height.min(stream_tuning_value(
        STREAM_ROWS_ENV,
        DEFAULT_STREAM_ROWS_PER_BATCH,
        &STREAM_ROW_OPTIONS,
    ));
    let queue_depth = stream_tuning_value(
        STREAM_QUEUE_DEPTH_ENV,
        DEFAULT_STREAM_QUEUE_DEPTH,
        &STREAM_QUEUE_DEPTH_OPTIONS,
    );
    let started = Instant::now();
    let stop = AtomicBool::new(false);
    let first_failed_plane = AtomicUsize::new(PLANE_COUNT);

    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(PLANE_COUNT);
        let mut row_receivers: Vec<Receiver<StreamedRows>> = Vec::with_capacity(PLANE_COUNT);
        let mut recycle_senders: Vec<Sender<Vec<u16>>> = Vec::with_capacity(PLANE_COUNT);
        let mut spawn_error = None;
        let Some(batch_capacity) = geometry.plane_width.checked_mul(rows_per_batch) else {
            return Err(StreamingDecodeError::Interleave(
                InterleaveError::GeometryOverflow,
            ));
        };

        'planes: for (plane_index, &plane_bytes) in plane_data.iter().enumerate() {
            let (row_sender, row_receiver) = sync_channel(queue_depth);
            let (recycle_sender, recycle_receiver) = channel();
            let mut current_batch = Vec::new();
            if current_batch.try_reserve_exact(batch_capacity).is_err() {
                stop.store(true, Ordering::Release);
                spawn_error = Some(ParallelDecodeError::WorkerSpawn {
                    kind: std::io::ErrorKind::OutOfMemory,
                });
                break;
            }
            for _ in 0..queue_depth {
                let mut spare_batch = Vec::new();
                if spare_batch.try_reserve_exact(batch_capacity).is_err() {
                    stop.store(true, Ordering::Release);
                    spawn_error = Some(ParallelDecodeError::WorkerSpawn {
                        kind: std::io::ErrorKind::OutOfMemory,
                    });
                    break 'planes;
                }
                if recycle_sender.send(spare_batch).is_err() {
                    stop.store(true, Ordering::Release);
                    spawn_error = Some(ParallelDecodeError::WorkerSpawn {
                        kind: std::io::ErrorKind::BrokenPipe,
                    });
                    break 'planes;
                }
            }
            row_receivers.push(row_receiver);
            recycle_senders.push(recycle_sender);

            let stop = &stop;
            let first_failed_plane = &first_failed_plane;
            let handle = thread::Builder::new()
                .name(format!("rrrah-crx-plane-{plane_index}"))
                .spawn_scoped(scope, move || {
                    let plane_started = Instant::now();
                    let mut coordination_wait = Duration::ZERO;
                    let mut batch_first_row = 0usize;
                    let mut batch_row_count = 0usize;
                    let mut next_expected_row = 0usize;
                    let mut protocol_failure = None;
                    let mut transport_stopped = false;
                    let should_cancel = || stop.load(Ordering::Acquire) || cancelled();
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        let mut emit_row = |row, mut samples: Vec<u16>| {
                            if should_cancel() {
                                transport_stopped = true;
                                return None;
                            }
                            if row != next_expected_row {
                                protocol_failure = Some(StreamProtocolFailure::Order {
                                    expected: next_expected_row,
                                    actual: row,
                                });
                                stop.store(true, Ordering::Release);
                                return None;
                            }
                            if next_expected_row >= geometry.plane_height {
                                protocol_failure = Some(StreamProtocolFailure::Count {
                                    expected: geometry.plane_height,
                                    actual: next_expected_row.saturating_add(1),
                                });
                                stop.store(true, Ordering::Release);
                                return None;
                            }
                            if samples.len() != geometry.plane_width {
                                protocol_failure = Some(StreamProtocolFailure::Length {
                                    row,
                                    expected: geometry.plane_width,
                                    actual: samples.len(),
                                });
                                stop.store(true, Ordering::Release);
                                return None;
                            }
                            if batch_row_count == 0 {
                                batch_first_row = row;
                            }
                            current_batch.extend_from_slice(&samples);
                            samples.clear();
                            batch_row_count += 1;
                            next_expected_row += 1;

                            let final_row = next_expected_row == geometry.plane_height;
                            if batch_row_count == rows_per_batch || final_row {
                                let wait_started = Instant::now();
                                let outgoing = std::mem::take(&mut current_batch);
                                if row_sender
                                    .send(StreamedRows {
                                        first_row: batch_first_row,
                                        row_count: batch_row_count,
                                        samples: outgoing,
                                    })
                                    .is_err()
                                {
                                    coordination_wait =
                                        coordination_wait.saturating_add(wait_started.elapsed());
                                    transport_stopped = true;
                                    return None;
                                }
                                batch_row_count = 0;
                                if !final_row {
                                    let Some(recycled) = recycle_receiver.recv().ok() else {
                                        coordination_wait =
                                            coordination_wait.saturating_add(wait_started.elapsed());
                                        transport_stopped = true;
                                        return None;
                                    };
                                    current_batch = recycled;
                                }
                                coordination_wait = coordination_wait.saturating_add(wait_started.elapsed());
                            }
                            Some(samples)
                        };
                        decode_plane_rows(plane_index, plane_bytes, &should_cancel, &mut emit_row)
                    }));
                    let elapsed = plane_started.elapsed();
                    if let Some(failure) = protocol_failure {
                        stop.store(true, Ordering::Release);
                        return StreamPlaneOutcome::Protocol(failure);
                    }
                    match result {
                        Ok(Ok(())) if next_expected_row != geometry.plane_height || batch_row_count != 0 => {
                            stop.store(true, Ordering::Release);
                            StreamPlaneOutcome::Protocol(StreamProtocolFailure::Count {
                                expected: geometry.plane_height,
                                actual: next_expected_row,
                            })
                        }
                        Ok(Ok(())) => StreamPlaneOutcome::Decoded {
                            elapsed,
                            coordination_wait,
                        },
                        Ok(Err(_)) | Err(_)
                            if transport_stopped || stop.load(Ordering::Acquire) || cancelled() =>
                        {
                            StreamPlaneOutcome::Stopped
                        }
                        Ok(Err(source)) => {
                            let _ = first_failed_plane.compare_exchange(
                                PLANE_COUNT,
                                plane_index,
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            );
                            stop.store(true, Ordering::Release);
                            StreamPlaneOutcome::Failed { source, elapsed }
                        }
                        Err(_) => {
                            let _ = first_failed_plane.compare_exchange(
                                PLANE_COUNT,
                                plane_index,
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            );
                            stop.store(true, Ordering::Release);
                            StreamPlaneOutcome::Panicked { elapsed }
                        }
                    }
                });

            match handle {
                Ok(handle) => handles.push((plane_index, handle)),
                Err(error) => {
                    stop.store(true, Ordering::Release);
                    spawn_error = Some(ParallelDecodeError::WorkerSpawn { kind: error.kind() });
                    break;
                }
            }
        }

        let mut assembly_failure = None;
        let mut interleave_elapsed = Duration::ZERO;
        if spawn_error.is_none() {
            'batches: for expected_first_row in (0..geometry.plane_height).step_by(rows_per_batch) {
                if cancelled() {
                    assembly_failure = Some(AssemblyFailure::Cancelled);
                    break;
                }

                let expected_row_count = (geometry.plane_height - expected_first_row).min(rows_per_batch);
                let expected_samples = geometry
                    .plane_width
                    .checked_mul(expected_row_count)
                    .expect("validated mosaic geometry and bounded batch size");
                let mut batches: [Option<Vec<u16>>; PLANE_COUNT] = std::array::from_fn(|_| None);
                for plane_index in 0..PLANE_COUNT {
                    let Ok(streamed) = row_receivers[plane_index].recv() else {
                        assembly_failure = Some(AssemblyFailure::ChannelClosed {
                            plane_index,
                            row: expected_first_row,
                        });
                        break 'batches;
                    };
                    if streamed.first_row != expected_first_row || streamed.row_count != expected_row_count {
                        assembly_failure = Some(AssemblyFailure::RowOrder {
                            plane_index,
                            expected: expected_first_row,
                            actual: streamed.first_row,
                        });
                        break 'batches;
                    }
                    if streamed.samples.len() != expected_samples {
                        assembly_failure = Some(AssemblyFailure::RowLength {
                            plane_index,
                            row: expected_first_row,
                            expected: expected_samples,
                            actual: streamed.samples.len(),
                        });
                        break 'batches;
                    }
                    batches[plane_index] = Some(streamed.samples);
                }

                let interleave_started = Instant::now();
                for row_offset in 0..expected_row_count {
                    let start = row_offset * geometry.plane_width;
                    let end = start + geometry.plane_width;
                    extend_interleaved_row(
                        &mut mosaic,
                        &batches[0]
                            .as_deref()
                            .expect("all four streamed batches are present")[start..end],
                        &batches[1]
                            .as_deref()
                            .expect("all four streamed batches are present")[start..end],
                    );
                    extend_interleaved_row(
                        &mut mosaic,
                        &batches[2]
                            .as_deref()
                            .expect("all four streamed batches are present")[start..end],
                        &batches[3]
                            .as_deref()
                            .expect("all four streamed batches are present")[start..end],
                    );
                }
                interleave_elapsed = interleave_elapsed.saturating_add(interleave_started.elapsed());

                if expected_first_row + expected_row_count < geometry.plane_height {
                    for plane_index in 0..PLANE_COUNT {
                        let mut batch = batches[plane_index]
                            .take()
                            .expect("all four streamed batches are present");
                        batch.clear();
                        // A producer may have already queued its final batch
                        // and exited; in that case this retired buffer is no
                        // longer needed and can be dropped.
                        let _ = recycle_senders[plane_index].send(batch);
                    }
                }
            }
        }

        if spawn_error.is_some() || assembly_failure.is_some() {
            stop.store(true, Ordering::Release);
        }
        drop(row_receivers);
        drop(recycle_senders);

        let mut outcomes: [Option<StreamPlaneOutcome<E>>; PLANE_COUNT] = std::array::from_fn(|_| None);
        let mut worker_panicked = false;
        for (plane_index, handle) in handles {
            match handle.join() {
                Ok(outcome) => outcomes[plane_index] = Some(outcome),
                Err(_) => worker_panicked = true,
            }
        }

        if let Some(error) = spawn_error {
            return Err(StreamingDecodeError::Parallel(error));
        }
        if worker_panicked {
            return Err(StreamingDecodeError::Parallel(
                ParallelDecodeError::WorkerPanicked,
            ));
        }
        if cancelled() || matches!(assembly_failure, Some(AssemblyFailure::Cancelled)) {
            return Err(StreamingDecodeError::Parallel(ParallelDecodeError::Cancelled));
        }

        if let Some(AssemblyFailure::RowOrder {
            plane_index,
            expected,
            actual,
        }) = assembly_failure
        {
            return Err(StreamingDecodeError::RowOrder {
                plane_index,
                expected,
                actual,
            });
        }
        if let Some(AssemblyFailure::RowLength {
            plane_index,
            row,
            expected,
            actual,
        }) = assembly_failure
        {
            return Err(StreamingDecodeError::RowLength {
                plane_index,
                row,
                expected,
                actual,
            });
        }

        for (plane_index, outcome) in outcomes.iter().enumerate() {
            match outcome {
                Some(StreamPlaneOutcome::Protocol(StreamProtocolFailure::Order { expected, actual })) => {
                    return Err(StreamingDecodeError::RowOrder {
                        plane_index,
                        expected: *expected,
                        actual: *actual,
                    });
                }
                Some(StreamPlaneOutcome::Protocol(StreamProtocolFailure::Length {
                    row,
                    expected,
                    actual,
                })) => {
                    return Err(StreamingDecodeError::RowLength {
                        plane_index,
                        row: *row,
                        expected: *expected,
                        actual: *actual,
                    });
                }
                Some(StreamPlaneOutcome::Protocol(StreamProtocolFailure::Count { expected, actual })) => {
                    return Err(StreamingDecodeError::RowCount {
                        plane_index,
                        expected: *expected,
                        actual: *actual,
                    });
                }
                _ => {}
            }
        }

        let first_failed = first_failed_plane.load(Ordering::Acquire);
        if first_failed < PLANE_COUNT {
            return match outcomes[first_failed].take() {
                Some(StreamPlaneOutcome::Failed { source, elapsed }) => {
                    Err(StreamingDecodeError::Parallel(ParallelDecodeError::Plane {
                        plane_index: first_failed,
                        elapsed,
                        source,
                    }))
                }
                Some(StreamPlaneOutcome::Panicked { elapsed }) => Err(StreamingDecodeError::Parallel(
                    ParallelDecodeError::PlanePanicked {
                        plane_index: first_failed,
                        elapsed,
                    },
                )),
                _ => Err(StreamingDecodeError::Parallel(ParallelDecodeError::Incomplete {
                    plane_index: first_failed,
                })),
            };
        }

        if let Some(AssemblyFailure::ChannelClosed { plane_index, row }) = assembly_failure {
            return Err(StreamingDecodeError::RowChannelClosed { plane_index, row });
        }

        let mut plane_elapsed = [Duration::ZERO; PLANE_COUNT];
        for (plane_index, outcome) in outcomes.into_iter().enumerate() {
            match outcome {
                Some(StreamPlaneOutcome::Decoded {
                    elapsed,
                    coordination_wait,
                }) => {
                    plane_elapsed[plane_index] = elapsed.saturating_sub(coordination_wait);
                }
                _ => {
                    return Err(StreamingDecodeError::Parallel(ParallelDecodeError::Incomplete {
                        plane_index,
                    }));
                }
            }
        }

        debug_assert_eq!(mosaic.len(), geometry.pixel_count);
        Ok(StreamingDecodeBatch {
            pixels: mosaic,
            plane_elapsed,
            wall_elapsed: started.elapsed(),
            interleave_elapsed,
            worker_count: PLANE_COUNT,
        })
    })
}

/// Interleaves CRX parity planes into one row-major RGGB sensor mosaic.
///
/// This is deliberately sequential: after parallel entropy decode, interleave
/// is a contiguous, memory-bandwidth-bound write.  A second worker wave would
/// add synchronization and memory contention without reducing entropy work.
pub(crate) fn interleave_rggb(
    planes: [&[u16]; PLANE_COUNT],
    sensor_width: u32,
    sensor_height: u32,
) -> Result<Vec<u16>, InterleaveError> {
    let geometry = checked_mosaic_geometry(sensor_width, sensor_height)?;
    for (plane_index, plane) in planes.iter().enumerate() {
        if plane.len() != geometry.plane_pixels {
            return Err(InterleaveError::PlaneLength {
                plane_index,
                expected: geometry.plane_pixels,
                actual: plane.len(),
            });
        }
    }

    let mut mosaic = allocate_mosaic(geometry.pixel_count)?;

    for plane_y in 0..geometry.plane_height {
        let plane_start = plane_y * geometry.plane_width;
        let plane_end = plane_start + geometry.plane_width;
        extend_interleaved_row(
            &mut mosaic,
            &planes[0][plane_start..plane_end],
            &planes[1][plane_start..plane_end],
        );
        extend_interleaved_row(
            &mut mosaic,
            &planes[2][plane_start..plane_end],
            &planes[3][plane_start..plane_end],
        );
    }

    debug_assert_eq!(mosaic.len(), geometry.pixel_count);
    Ok(mosaic)
}

fn extend_interleaved_row(output: &mut Vec<u16>, even: &[u16], odd: &[u16]) {
    const CHUNK_PAIRS: usize = 16;

    debug_assert_eq!(even.len(), odd.len());
    let mut even_chunks = even.chunks_exact(CHUNK_PAIRS);
    let mut odd_chunks = odd.chunks_exact(CHUNK_PAIRS);
    for (even, odd) in even_chunks.by_ref().zip(odd_chunks.by_ref()) {
        output.extend_from_slice(&[
            even[0], odd[0], even[1], odd[1], even[2], odd[2], even[3], odd[3], even[4], odd[4], even[5],
            odd[5], even[6], odd[6], even[7], odd[7], even[8], odd[8], even[9], odd[9], even[10], odd[10],
            even[11], odd[11], even[12], odd[12], even[13], odd[13], even[14], odd[14], even[15], odd[15],
        ]);
    }
    debug_assert_eq!(even_chunks.remainder().len(), odd_chunks.remainder().len());
    for (&even_sample, &odd_sample) in even_chunks.remainder().iter().zip(odd_chunks.remainder()) {
        output.push(even_sample);
        output.push(odd_sample);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fmt,
        sync::{
            Arc, Barrier,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
    };

    use super::{
        InterleaveError, ParallelDecodeError, StreamingDecodeError, decode_four_planes_streaming,
        decode_four_planes_with_parallelism, interleave_rggb,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct SyntheticError(usize);

    impl fmt::Display for SyntheticError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "synthetic plane {} failure", self.0)
        }
    }

    impl std::error::Error for SyntheticError {}

    #[test]
    fn interleaves_four_parities_in_rggb_sensor_order() {
        let p0 = [0, 1, 2, 3];
        let p1 = [10, 11, 12, 13];
        let p2 = [20, 21, 22, 23];
        let p3 = [30, 31, 32, 33];

        let mosaic = interleave_rggb([&p0, &p1, &p2, &p3], 4, 4).unwrap();

        assert_eq!(
            mosaic,
            [
                0, 10, 1, 11, //
                20, 30, 21, 31, //
                2, 12, 3, 13, //
                22, 32, 23, 33,
            ]
        );
    }

    #[test]
    fn interleaves_each_plane_row_directly_into_the_final_mosaic() {
        let p0 = [0, 1, 2, 3, 4, 5];
        let p1 = [10, 11, 12, 13, 14, 15];
        let p2 = [20, 21, 22, 23, 24, 25];
        let p3 = [30, 31, 32, 33, 34, 35];

        let mosaic = interleave_rggb([&p0, &p1, &p2, &p3], 6, 4).unwrap();

        assert_eq!(
            mosaic,
            [
                0, 10, 1, 11, 2, 12, //
                20, 30, 21, 31, 22, 32, //
                3, 13, 4, 14, 5, 15, //
                23, 33, 24, 34, 25, 35,
            ]
        );
    }

    #[test]
    fn interleave_rejects_invalid_geometry_before_allocating() {
        let empty: &[u16] = &[];
        assert_eq!(
            interleave_rggb([empty; 4], 0, 4),
            Err(InterleaveError::EmptyGeometry)
        );
        assert_eq!(
            interleave_rggb([empty; 4], 3, 4),
            Err(InterleaveError::OddGeometry { width: 3, height: 4 })
        );
        assert!(matches!(
            interleave_rggb([empty; 4], 32_768, 16_384),
            Err(InterleaveError::PixelLimit { .. })
        ));
    }

    #[test]
    fn interleave_rejects_each_wrong_plane_length() {
        let full = [1_u16; 4];
        let short = [1_u16; 3];
        for wrong_plane in 0..4 {
            let mut planes: [&[u16]; 4] = [&full; 4];
            planes[wrong_plane] = &short;
            assert_eq!(
                interleave_rggb(planes, 4, 4),
                Err(InterleaveError::PlaneLength {
                    plane_index: wrong_plane,
                    expected: 4,
                    actual: 3,
                })
            );
        }
    }

    #[test]
    fn streaming_decode_matches_four_plane_interleave() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let result = decode_four_planes_streaming(chunks, 4, 4, &|| false, &|plane_index, _, _, emit_row| {
            let mut row = Vec::with_capacity(2);
            for row_index in 0..2 {
                let base = u16::try_from(plane_index * 20 + row_index * 2).unwrap();
                row.extend_from_slice(&[base, base + 1]);
                row = emit_row(row_index, row).ok_or(SyntheticError(100 + plane_index))?;
                assert!(row.is_empty());
            }
            Ok::<_, SyntheticError>(())
        });
        let batch = result.unwrap();

        assert_eq!(
            batch.pixels,
            [
                0, 20, 1, 21, //
                40, 60, 41, 61, //
                2, 22, 3, 23, //
                42, 62, 43, 63,
            ]
        );
        assert_eq!(batch.worker_count, 4);
        assert!(batch.interleave_elapsed <= batch.wall_elapsed);
        assert!(
            batch
                .plane_elapsed
                .into_iter()
                .all(|elapsed| elapsed <= batch.wall_elapsed)
        );
    }

    #[test]
    fn streaming_decode_attributes_a_plane_failure_without_hanging() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let result = decode_four_planes_streaming(chunks, 4, 4, &|| false, &|plane_index, _, _, emit_row| {
            let row = vec![u16::try_from(plane_index).unwrap(); 2];
            let _ = emit_row(0, row).ok_or(SyntheticError(100 + plane_index))?;
            if plane_index == 2 {
                return Err(SyntheticError(2));
            }
            let row = vec![u16::try_from(plane_index).unwrap(); 2];
            let _ = emit_row(1, row).ok_or(SyntheticError(100 + plane_index))?;
            Ok::<(), SyntheticError>(())
        });

        assert!(matches!(
            result,
            Err(StreamingDecodeError::Parallel(ParallelDecodeError::Plane {
                plane_index: 2,
                source: SyntheticError(2),
                ..
            }))
        ));
    }

    #[test]
    fn streaming_decode_rejects_out_of_order_rows() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let result = decode_four_planes_streaming(chunks, 4, 2, &|| false, &|plane_index, _, _, emit_row| {
            let row_index = usize::from(plane_index == 1);
            let row = vec![u16::try_from(plane_index).unwrap(); 2];
            let _ = emit_row(row_index, row).ok_or(SyntheticError(100 + plane_index))?;
            Ok::<(), SyntheticError>(())
        });

        assert!(matches!(
            result,
            Err(StreamingDecodeError::RowOrder {
                plane_index: 1,
                expected: 0,
                actual: 1,
            })
        ));
    }

    #[test]
    fn streaming_decode_rejects_reordering_inside_a_batch() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let result =
            decode_four_planes_streaming(chunks, 4, 66, &|| false, &|plane_index, _, _, emit_row| {
                for expected_row in 0..33 {
                    let actual_row = if plane_index == 1 && expected_row == 10 {
                        11
                    } else {
                        expected_row
                    };
                    let row = vec![u16::try_from(plane_index).unwrap(); 2];
                    let _ = emit_row(actual_row, row).ok_or(SyntheticError(100 + plane_index))?;
                }
                Ok::<(), SyntheticError>(())
            });

        assert!(matches!(
            result,
            Err(StreamingDecodeError::RowOrder {
                plane_index: 1,
                expected: 10,
                actual: 11,
            })
        ));
    }

    #[test]
    fn streaming_decode_rejects_an_individual_wrong_row_width() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let result =
            decode_four_planes_streaming(chunks, 4, 66, &|| false, &|plane_index, _, _, emit_row| {
                for row_index in 0..33 {
                    let width = usize::from(!(plane_index == 2 && row_index == 5)) + 1;
                    let row = vec![u16::try_from(plane_index).unwrap(); width];
                    let _ = emit_row(row_index, row).ok_or(SyntheticError(100 + plane_index))?;
                }
                Ok::<(), SyntheticError>(())
            });

        assert!(matches!(
            result,
            Err(StreamingDecodeError::RowLength {
                plane_index: 2,
                row: 5,
                expected: 2,
                actual: 1,
            })
        ));
    }

    #[test]
    fn streaming_decode_rejects_early_success_from_one_plane() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let result =
            decode_four_planes_streaming(chunks, 4, 66, &|| false, &|plane_index, _, _, emit_row| {
                let rows = if plane_index == 1 { 32 } else { 33 };
                for row_index in 0..rows {
                    let row = vec![u16::try_from(plane_index).unwrap(); 2];
                    let _ = emit_row(row_index, row).ok_or(SyntheticError(100 + plane_index))?;
                }
                Ok::<(), SyntheticError>(())
            });

        assert!(matches!(
            result,
            Err(StreamingDecodeError::RowCount {
                plane_index: 1,
                expected: 33,
                actual: 32,
            })
        ));
    }

    #[test]
    fn streaming_decode_rejects_an_extra_row() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let result = decode_four_planes_streaming(chunks, 4, 2, &|| false, &|plane_index, _, _, emit_row| {
            let rows = if plane_index == 2 { 2 } else { 1 };
            for row_index in 0..rows {
                let row = vec![u16::try_from(plane_index).unwrap(); 2];
                let _ = emit_row(row_index, row).ok_or(SyntheticError(100 + plane_index))?;
            }
            Ok::<(), SyntheticError>(())
        });

        assert!(matches!(
            result,
            Err(StreamingDecodeError::RowCount {
                plane_index: 2,
                expected: 1,
                actual: 2,
            })
        ));
    }

    #[test]
    fn streaming_decode_attributes_a_plane_panic() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let result = decode_four_planes_streaming(chunks, 4, 2, &|| false, &|plane_index, _, _, emit_row| {
            assert_ne!(plane_index, 3, "synthetic streaming panic");
            let row = vec![u16::try_from(plane_index).unwrap(); 2];
            let _ = emit_row(0, row).ok_or(SyntheticError(100 + plane_index))?;
            Ok::<(), SyntheticError>(())
        });

        assert!(matches!(
            result,
            Err(StreamingDecodeError::Parallel(
                ParallelDecodeError::PlanePanicked { plane_index: 3, .. }
            ))
        ));
    }

    #[test]
    fn streaming_decode_external_cancellation_unblocks_all_workers() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let cancelled = AtomicBool::new(false);
        let result = decode_four_planes_streaming(
            chunks,
            4,
            2,
            &|| cancelled.load(Ordering::Acquire),
            &|plane_index, _, _, emit_row| {
                if plane_index == 0 {
                    cancelled.store(true, Ordering::Release);
                }
                let row = vec![u16::try_from(plane_index).unwrap(); 2];
                let _ = emit_row(0, row).ok_or(SyntheticError(100 + plane_index))?;
                Ok::<(), SyntheticError>(())
            },
        );

        assert!(matches!(
            result,
            Err(StreamingDecodeError::Parallel(ParallelDecodeError::Cancelled))
        ));
    }

    #[test]
    fn parallel_decode_is_bounded_and_returns_stable_plane_order() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let barrier = Arc::new(Barrier::new(2));
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let first_wave = AtomicUsize::new(0);

        let batch = decode_four_planes_with_parallelism(
            chunks,
            4,
            2,
            &|| false,
            &|plane_index, bytes, _cancelled| {
                let now = active.fetch_add(1, Ordering::AcqRel) + 1;
                peak.fetch_max(now, Ordering::AcqRel);
                if first_wave.fetch_add(1, Ordering::AcqRel) < 2 {
                    barrier.wait();
                }
                active.fetch_sub(1, Ordering::AcqRel);
                Ok::<_, SyntheticError>(vec![u16::from(bytes[0]), u16::try_from(plane_index).unwrap()])
            },
        )
        .unwrap();

        assert_eq!(batch.worker_count, 2);
        assert_eq!(peak.load(Ordering::Acquire), 2);
        assert_eq!(batch.planes, [vec![0, 0], vec![1, 1], vec![2, 2], vec![3, 3]]);
        assert!(batch.wall_elapsed >= batch.plane_elapsed.into_iter().min().unwrap());
    }

    #[test]
    fn zero_worker_request_still_uses_one_caller_worker() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let batch = decode_four_planes_with_parallelism(chunks, 0, 8, &|| false, &|plane_index, _, _| {
            let now = active.fetch_add(1, Ordering::AcqRel) + 1;
            peak.fetch_max(now, Ordering::AcqRel);
            active.fetch_sub(1, Ordering::AcqRel);
            Ok::<_, SyntheticError>(vec![u16::try_from(plane_index).unwrap()])
        })
        .unwrap();

        assert_eq!(batch.worker_count, 1);
        assert_eq!(peak.load(Ordering::Acquire), 1);
    }

    #[test]
    fn cancellation_prevents_any_plane_from_starting() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let calls = AtomicUsize::new(0);
        let result = decode_four_planes_with_parallelism(chunks, 4, 4, &|| true, &|_, _, _| {
            calls.fetch_add(1, Ordering::AcqRel);
            Ok::<_, SyntheticError>(Vec::new())
        });

        assert!(matches!(result, Err(ParallelDecodeError::Cancelled)));
        assert_eq!(calls.load(Ordering::Acquire), 0);
    }

    #[test]
    fn an_error_signals_inflight_decoders_and_is_attributed() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let barrier = Arc::new(Barrier::new(2));
        let observed_cancel = AtomicBool::new(false);
        let result =
            decode_four_planes_with_parallelism(chunks, 4, 2, &|| false, &|plane_index, _, cancelled| {
                barrier.wait();
                if plane_index == 0 {
                    return Err(SyntheticError(0));
                }
                while !cancelled() {
                    thread::yield_now();
                }
                observed_cancel.store(true, Ordering::Release);
                Ok(Vec::new())
            });

        assert!(matches!(
            result,
            Err(ParallelDecodeError::Plane {
                plane_index: 0,
                source: SyntheticError(0),
                ..
            })
        ));
        assert!(observed_cancel.load(Ordering::Acquire));
    }

    #[test]
    fn a_plane_panic_is_contained_until_every_worker_joins() {
        let chunks: [&[u8]; 4] = [&[0], &[1], &[2], &[3]];
        let result = decode_four_planes_with_parallelism(chunks, 1, 1, &|| false, &|plane_index,
                                                                                    _,
                                                                                    _|
         -> Result<
            Vec<u16>,
            SyntheticError,
        > {
            assert_ne!(plane_index, 0, "synthetic decoder panic");
            Ok(Vec::new())
        });

        assert!(matches!(
            result,
            Err(ParallelDecodeError::PlanePanicked { plane_index: 0, .. })
        ));
    }
}
