use rrrah_cache::{CacheLimits, ImageSwapConfig, MemoryBudget, RasterSwapCache};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::io::{Read, Write};
use std::sync::Arc;

struct ExpandingPayload(rrrah_memory::SharedBuffer<u8>);
impl rrrah_cache::SwapPayload for ExpandingPayload {
    fn capacity_bytes(&self) -> u64 {
        self.0.capacity_bytes()
    }
    fn payload_len(&self) -> std::io::Result<u64> {
        Ok(5 * 1024 * 1024)
    }
    fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()> {
        for _ in 0..80 {
            writer.write_all(&[7; 64 * 1024])?;
        }
        Ok(())
    }
    fn read_payload(
        reader: &mut impl Read,
        _: u64,
        budget: &MemoryBudget,
    ) -> Result<Self, rrrah_cache::SwapPayloadError> {
        let mut block = [0; 64 * 1024];
        // Deliberately allocate the expanded output after consuming wire bytes.
        for _ in 0..2 {
            reader
                .read_exact(&mut block)
                .map_err(rrrah_cache::SwapPayloadError::Invalid)?;
        }
        let values = budget
            .try_buffer(9 * 1024 * 1024, 7)
            .map_err(rrrah_cache::SwapPayloadError::Memory)?;
        while reader
            .read(&mut block)
            .map_err(rrrah_cache::SwapPayloadError::Invalid)?
            != 0
        {}
        Ok(Self(values.freeze()))
    }
}

#[test]
fn custom_expanding_codecs_keep_pixel_admission_priority_by_default() {
    let directory = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(10 * 1024 * 1024);
    let weight = 9 * 1024 * 1024;
    let swap = rrrah_cache::ImageSwapCache::new_with_budgets(
        directory.path(),
        ImageSwapConfig {
            limits: CacheLimits::bytes(5 * 1024 * 1024),
            queue_bytes: weight,
            queue_count: 1,
            restore_bytes: weight,
        },
        MemoryBudget::new(weight),
        root.clone(),
    )
    .unwrap();
    swap.enqueue(
        1u8,
        ExpandingPayload(
            MemoryBudget::new(weight)
                .try_buffer(weight as usize, 7)
                .unwrap()
                .freeze(),
        ),
    );
    swap.wait_idle().unwrap();
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(restored.0.len() as u64, weight);
    assert!(restored.0.iter().all(|v| *v == 7));
    assert_eq!(root.peak(), weight);
    drop(restored);
    assert_eq!(root.used(), 0);
    assert_eq!(swap.stats().errors, 0);
}

#[test]
fn parallel_scratch_preserves_exact_pixel_caps_profiles_and_budget_replacement() {
    let values: Vec<f32> = (0..1024 * 512 * 4)
        .map(|i| match i % 4 {
            0 => -0.0,
            1 => f32::from_bits(0x7fc01234),
            2 => 8.0,
            _ => 1.0,
        })
        .collect();
    // Large opaque profile exercises header/profile reads before pixel admission.
    let source = DecodedRaster::new(
        1024,
        512,
        RasterPixels::Rgba32Float(Arc::new(values).into()),
        RasterColorSpace::Icc(vec![73; 1024 * 1024]),
    )
    .unwrap();
    let weight = source.capacity_bytes();
    let directory = tempfile::tempdir().unwrap();
    let generous = MemoryBudget::new(64 * 1024 * 1024);
    let mut swap = RasterSwapCache::new_with_budgets(
        directory.path(),
        ImageSwapConfig {
            limits: CacheLimits::bytes(weight + 64),
            queue_bytes: weight,
            queue_count: 1,
            restore_bytes: weight,
        },
        MemoryBudget::new(weight),
        generous.clone(),
    )
    .unwrap();
    swap.enqueue(1u8, source.clone());
    swap.wait_idle().unwrap();
    assert_eq!(swap.stats().writes, 1);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(restored.color_space(), source.color_space());
    let RasterPixels::Rgba32Float(expected) = source.pixels() else {
        unreachable!()
    };
    let RasterPixels::Rgba32Float(actual) = restored.pixels() else {
        unreachable!()
    };
    assert!(
        actual
            .iter()
            .zip(expected.iter())
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
    assert_eq!(swap.restore_budget().used(), weight);
    assert_eq!(generous.used(), weight); // Scratch ends before publishing pixels.
    if std::thread::available_parallelism().is_ok_and(|n| n.get() >= 2) {
        assert_eq!(
            generous.peak(),
            weight + rrrah_swap::PARALLEL_HASH_SCRATCH_BYTES as u64
        );
    }
    drop(restored);
    assert_eq!(generous.used(), 0);

    let exact = MemoryBudget::new(weight);
    swap.set_restore_budget(exact.clone());
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(restored.color_space(), source.color_space());
    let RasterPixels::Rgba32Float(actual) = restored.pixels() else {
        unreachable!()
    };
    assert!(
        actual
            .iter()
            .zip(expected.iter())
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
    assert_eq!(exact.peak(), weight);
    assert_eq!(generous.used(), 0);
    drop(restored);
    assert_eq!(exact.used(), 0);
    assert_eq!(swap.stats().reads, 2);
    assert_eq!(swap.stats().errors, 0);
    assert_eq!(swap.stats().queued_bytes, 0);
}
