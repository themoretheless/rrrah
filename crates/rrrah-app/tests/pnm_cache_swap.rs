#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

use rrrah_cache::{CacheLimits, ImageSwapConfig, RasterRamCache, RasterSwapCache};
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};

fn qualify(mut observe: impl FnMut(&DecodedRaster) -> Vec<u8>) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("toe.ppm");
    let levels = [0u16, 1, 5308, 5309, 5326, 5327, 32768, 65535];
    let mut bytes = b"P6\n4 2\n65535\n".to_vec();
    for (index, value) in levels.iter().enumerate() {
        for sample in [*value, levels[7 - index], levels[(index + 3) % 8]] {
            bytes.extend_from_slice(&sample.to_be_bytes());
        }
    }
    std::fs::write(&path, bytes).unwrap();
    // Qualify both the original transfer declaration and prepared float storage.
    for prepared_storage in [false, true] {
        let budget = MemoryBudget::new(4096);
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!(decoded.color_space(), &RasterColorSpace::Bt709);
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&decoded, Some(&budget)).unwrap();
        let RasterPixels::Rgba32Float(values) = prepared.pixels() else {
            panic!()
        };
        let expected: Vec<_> = values.iter().map(|v| v.to_bits()).collect();
        let before = observe(&prepared);
        let source = if prepared_storage {
            drop(decoded);
            prepared
        } else {
            drop(prepared);
            decoded
        };
        let color = source.color_space().clone();
        let weight = source.capacity_bytes();
        let swap = RasterSwapCache::<u8>::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits {
                    max_bytes: 8192,
                    max_entries: Some(1),
                    ttl: None,
                },
                queue_bytes: weight,
                queue_count: 1,
                restore_bytes: weight,
            },
            MemoryBudget::new(weight),
            budget.clone(),
        )
        .unwrap();
        let mut ram = RasterRamCache::new(CacheLimits {
            max_bytes: weight,
            max_entries: Some(1),
            ttl: None,
        });
        ram.enable_swap(swap);
        assert!(ram.insert(1, source));
        let lease = ram.get_lease(&1).unwrap();
        assert!(!ram.set_limits_and_spill(CacheLimits::bytes(0)));
        drop(lease);
        assert!(ram.set_limits_and_spill(CacheLimits::bytes(0)));
        assert!(ram.is_empty());
        let swap = ram.swap().unwrap();
        swap.wait_idle().unwrap();
        assert_eq!(budget.used(), 0);
        let mut allocation_seen = false;
        let cancelled = swap
            .try_get(&1, || {
                allocation_seen |= budget.used() > 0;
                allocation_seen
            })
            .unwrap();
        assert!(
            allocation_seen,
            "cancel after output admission, not before reading"
        );
        assert!(cancelled.is_none());
        assert_eq!(budget.used(), 0);
        assert_eq!(swap.stats().errors, 0);
        assert_eq!(swap.stats().reads, 0);
        let pressure = budget.try_reserve(budget.limit()).unwrap();
        assert!(swap.try_get(&1, || false).is_err());
        assert_eq!(swap.stats().errors, 0);
        drop(pressure);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(restored.color_space(), &color);
        assert_eq!(budget.used(), weight);
        assert!(swap.try_get(&1, || false).is_err());
        let alias = restored.clone();
        let ready = rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
        let RasterPixels::Rgba32Float(values) = ready.pixels() else {
            panic!()
        };
        assert_eq!(values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
        assert_eq!(observe(&ready), before);
        drop(ready);
        drop(restored);
        assert_eq!(budget.used(), weight);
        drop(alias);
        assert_eq!(budget.used(), 0);
        let retry = swap.try_get(&1, || false).unwrap().unwrap();
        drop(retry);
        assert_eq!(budget.used(), 0);
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(swap.stats().reads, 2);
        assert_eq!(swap.stats().errors, 0);
        assert_eq!(swap.stats().queued_bytes, 0);
    }
}

#[test]
fn pnm_ram_swap_pressure_preserves_prepared_bits_and_releases_aliases() {
    qualify(|_| Vec::new());
}

#[test]
#[ignore = "requires an actual GPU for full frame readback"]
fn pnm_ram_swap_preserves_metal_frame() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("PNM swap adapter: {}", gpu.adapter_name());
    qualify(|raster| {
        let pixels = gpu
            .render_raster(
                raster,
                rrrah_gpu::ViewParameters {
                    viewport: [64.0; 2],
                    zoom: 8.0,
                    ..Default::default()
                },
                [64, 64],
            )
            .pixels;
        assert!(pixels.chunks_exact(4).any(|pixel| pixel != &pixels[..4]));
        pixels
    });
}
