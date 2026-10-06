#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
fn explicit_rla_alpha_survives_ram_swap_pressure_and_metal() {
    use rrrah_cache::{CacheLimits, ImageSwapConfig, RasterRamCache, RasterSwapCache};
    use rrrah_core::{MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("RLA transport adapter: {}", gpu.adapter_name());
    for name in ["rla-8-c3-a8-mixed1.rla", "rla-16-c3-a8-mixed1.rla"] {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster")
            .join(name);
        for offset_window in [false, true] {
            let fixture_directory = tempfile::tempdir().unwrap();
            let path = if offset_window {
                let mut source = std::fs::read(&path).unwrap();
                for (offset, coordinate) in [(0, -2i16), (2, 142), (4, -1), (6, 4)] {
                    source[offset..offset + 2].copy_from_slice(&coordinate.to_be_bytes());
                }
                let altered = fixture_directory.path().join(name);
                std::fs::write(&altered, source).unwrap();
                altered
            } else {
                path.clone()
            };
            let budget = MemoryBudget::new(4 * 1024 * 1024);
            let mut request = rrrah_decode::DecodeRequest::new(path);
            request.memory_budget = Some(budget.clone());
            let decoded = rrrah_decode::decode_rla_with_interpretation(
                &request,
                rrrah_decode::RlaAlphaMode::Straight,
                RasterColorSpace::Srgb,
            )
            .unwrap();
            assert_eq!(budget.used(), decoded.capacity_bytes());
            let mut expected_payload = Vec::new();
            rrrah_cache::write_raster_payload(&mut expected_payload, &decoded).unwrap();
            let ready =
                rrrah_decode::prepare_raster_for_display_with_budget(&decoded, Some(&budget)).unwrap();
            let RasterPixels::Rgba32Float(samples) = ready.pixels() else {
                panic!()
            };
            if offset_window {
                assert_eq!((ready.width(), ready.height()), (145, 6));
                for y in 0..6usize {
                    for x in 0..145usize {
                        if !(2..142).contains(&x) || !(2..5).contains(&y) {
                            assert_eq!(
                                &samples[(y * 145 + x) * 4..(y * 145 + x + 1) * 4],
                                &[0.0, 0.0, 0.0, 0.0]
                            );
                        }
                    }
                }
            }
            let expected_bits: Vec<_> = samples.iter().map(|v| v.to_bits()).collect();
            let view = rrrah_gpu::ViewParameters {
                viewport: [64.0; 2],
                ..Default::default()
            };
            let expected_frame = gpu.render_raster(&ready, view, [64, 64]).pixels;
            drop(ready);
            let weight = decoded.capacity_bytes();
            let directory = tempfile::tempdir().unwrap();
            let swap = RasterSwapCache::<u8>::new_with_budgets(
                directory.path(),
                ImageSwapConfig {
                    limits: CacheLimits {
                        max_bytes: 65536,
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
            let mut ram = RasterRamCache::new(CacheLimits::bytes(weight));
            ram.enable_swap(swap);
            assert!(ram.insert_visible(1, decoded));
            let lease = ram.get_lease(&1).unwrap();
            assert!(!ram.set_limits_and_spill(CacheLimits::bytes(0)));
            drop(lease);
            ram.mark_visible(2);
            assert!(ram.set_limits_and_spill(CacheLimits::bytes(0)));
            let swap = ram.swap().unwrap();
            swap.wait_idle().unwrap();
            assert_eq!(budget.used(), 0);
            let pressure = budget.try_reserve(budget.limit()).unwrap();
            assert!(swap.try_get(&1, || false).is_err());
            drop(pressure);
            let restored = swap.try_get(&1, || false).unwrap().unwrap();
            let mut actual_payload = Vec::new();
            rrrah_cache::write_raster_payload(&mut actual_payload, &restored).unwrap();
            assert_eq!(actual_payload, expected_payload);
            let alias = restored.clone();
            let ready =
                rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
            let RasterPixels::Rgba32Float(samples) = ready.pixels() else {
                panic!()
            };
            assert_eq!(
                samples.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                expected_bits
            );
            assert_eq!(gpu.render_raster(&ready, view, [64, 64]).pixels, expected_frame);
            drop(ready);
            drop(restored);
            assert_eq!(budget.used(), weight);
            drop(alias);
            assert_eq!(budget.used(), 0);
            assert_eq!(swap.stats().writes, 1);
            assert_eq!(swap.stats().reads, 1);
            assert_eq!(swap.stats().errors, 0);
        }
    }
}
