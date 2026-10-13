//! Informational 12 MP restore stage profile. No GPU or physical-display claim.
//! Source/reference allocation is outside the managed restore budget. Timer
//! calls add overhead; stream time includes file I/O, buffering and integrity.
//! `--features restore-profile-parallel` adds bounded 2/4-thread hash controls
//! after streaming measurements. It does not enable parallel production swap.
use rrrah_cache::SwapPayload;
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use std::{
    hint::black_box,
    io::Read,
    sync::Arc,
    time::{Duration, Instant},
};

struct TimedRead<'a> {
    inner: &'a mut dyn Read,
    elapsed: Duration,
    calls: usize,
    bytes: usize,
}
impl Read for TimedRead<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let start = Instant::now();
        let result = self.inner.read(output);
        self.elapsed += start.elapsed();
        self.calls += 1;
        if let Ok(count) = result {
            self.bytes += count;
        }
        result
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let samples = 4000 * 3000 * 4;
    let values: Vec<f32> = (0..samples)
        .map(|i| {
            if i % 4 == 3 {
                1.
            } else {
                (i % 1021) as f32 / 255. - 1.
            }
        })
        .collect();
    let source = DecodedRaster::new(
        4000,
        3000,
        RasterPixels::Rgba32Float(Arc::new(values).into()),
        RasterColorSpace::LinearSrgb,
    )?;
    let RasterPixels::Rgba32Float(expected) = source.pixels() else {
        unreachable!()
    };
    let directory = tempfile::tempdir()?;
    let length = source.payload_len()?;
    let store = rrrah_swap::SwapStore::new(
        directory.path(),
        rrrah_swap::SwapLimits {
            max_bytes: length,
            max_objects: Some(1),
        },
    )?;
    let handle = store.write_stream(length, || false, |mut writer| source.write_payload(&mut writer))?;
    let budget = MemoryBudget::new(source.capacity_bytes());
    println!(
        "iteration,total_ms,stream_io_hash_ms,non_stream_ms,zero_init_ms,hash_only_ms,hash_1m_ms,hash_whole_ms,read_calls,bytes"
    );
    for iteration in 0..18 {
        let mut stream_time = Duration::ZERO;
        let mut calls = 0;
        let mut bytes = 0;
        let start = Instant::now();
        let restored = store.read_stream(
            &handle,
            || false,
            |reader| {
                let mut timed = TimedRead {
                    inner: reader,
                    elapsed: Duration::ZERO,
                    calls: 0,
                    bytes: 0,
                };
                let result =
                    DecodedRaster::read_payload(&mut timed, length, &budget).map_err(std::io::Error::other);
                stream_time = timed.elapsed;
                calls = timed.calls;
                bytes = timed.bytes;
                result
            },
        )?;
        let total = start.elapsed();
        let RasterPixels::Rgba32Float(actual) = restored.pixels() else {
            unreachable!()
        };
        // Identity is checked outside every timed interval.
        assert_eq!(actual.as_slice(), expected.as_slice());
        assert_eq!((restored.width(), restored.height()), (4000, 3000));
        assert_eq!(restored.color_space(), &RasterColorSpace::LinearSrgb);
        assert_eq!(bytes as u64, length);
        drop(restored);
        assert_eq!(budget.used(), 0);
        let allocate = Instant::now();
        let zeroed = budget.try_buffer(samples, 0f32)?;
        black_box(&zeroed);
        let allocation_time = allocate.elapsed();
        drop(zeroed);
        assert_eq!(budget.used(), 0);
        let hash = Instant::now();
        let mut hasher = blake3::Hasher::new();
        for chunk in bytemuck::cast_slice::<f32, u8>(expected.as_slice()).chunks(64 * 1024) {
            hasher.update(black_box(chunk));
        }
        let expected_hash = hasher.finalize();
        black_box(expected_hash);
        let hash_time = hash.elapsed();
        // Same digest and input; distinguish update-boundary cost from I/O.
        let hash_large = Instant::now();
        let mut hasher = blake3::Hasher::new();
        for chunk in bytemuck::cast_slice::<f32, u8>(expected.as_slice()).chunks(1024 * 1024) {
            hasher.update(black_box(chunk));
        }
        let large_digest = hasher.finalize();
        let large_time = hash_large.elapsed();
        assert_eq!(large_digest, expected_hash);
        let hash_whole = Instant::now();
        let whole_digest = blake3::hash(black_box(bytemuck::cast_slice::<f32, u8>(expected.as_slice())));
        let whole_time = hash_whole.elapsed();
        assert_eq!(whole_digest, expected_hash);
        if iteration >= 3 {
            let ms = |d: Duration| d.as_secs_f64() * 1000.;
            println!(
                "{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{calls},{bytes}",
                iteration - 3,
                ms(total),
                ms(stream_time),
                ms(total.saturating_sub(stream_time)),
                ms(allocation_time),
                ms(hash_time),
                ms(large_time),
                ms(whole_time)
            );
        }
    }
    eprintln!(
        "18 restores exact; payload_bytes={length}; managed_peak={}; final_managed=0; warm local disk; allocation/hash controls are separate operations",
        budget.peak()
    );
    #[cfg(feature = "restore-profile-parallel")]
    {
        let input = bytemuck::cast_slice::<f32, u8>(expected.as_slice());
        let digest = blake3::hash(input);
        // Dedicated bounded pools; controls do not change production streaming.
        for threads in [2, 4] {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build()?;
            for chunk_bytes in [1024 * 1024, 4 * 1024 * 1024] {
                let mut times = Vec::new();
                for iteration in 0..18 {
                    let start = Instant::now();
                    let actual = pool.install(|| {
                        let mut hasher = blake3::Hasher::new();
                        for chunk in input.chunks(chunk_bytes) {
                            hasher.update_rayon(black_box(chunk));
                        }
                        hasher.finalize()
                    });
                    let elapsed = start.elapsed();
                    assert_eq!(actual, digest);
                    if iteration >= 3 {
                        times.push(elapsed);
                    }
                }
                times.sort();
                eprintln!(
                    "parallel_hash,threads={threads},chunk_bytes={chunk_bytes},p50_ms={:.6},p95_ms={:.6},n=15,digest_exact=true",
                    times[7].as_secs_f64() * 1000.,
                    times[14].as_secs_f64() * 1000.
                );
            }
        }
    }
    Ok(())
}
