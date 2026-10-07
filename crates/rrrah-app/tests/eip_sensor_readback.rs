#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires authored IIQ/DNG/MRW/local CR3 packages, qualified original RAWs and Metal"]
fn explicit_eip_sensor_preserves_native_frames_ram_disk_and_swap() {
    use rrrah_decode::RawDecoder;
    let iiq = std::path::PathBuf::from(std::env::var("RRRAH_EIP_IIQ_DIR").unwrap());
    let dng = std::path::PathBuf::from(std::env::var("RRRAH_EIP_DNG_DIR").unwrap());
    let cr3 = std::path::PathBuf::from(std::env::var("RRRAH_EIP_CR3_CORPUS").unwrap());
    let mrw = std::path::PathBuf::from(std::env::var("RRRAH_EIP_MRW_CORPUS").unwrap());
    let originals = std::path::PathBuf::from(std::env::var("RRRAH_EIP_ORIGINAL_CORPUS").unwrap());
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = [
        (mrw.join("1826"), "mrw", originals.join("1826.mrw")),
        (mrw.join("4419"), "mrw", originals.join("4419.mrw")),
        (iiq, "iiq", originals.join("4366.iiq")),
        (dng, "dng", originals.join("830.dng")),
        (cr3.join("IMG_9043"), "cr3", repo.join("tests/IMG_9043.CR3")),
        (cr3.join("IMG_9074"), "cr3", repo.join("tests/IMG_9074.CR3")),
    ];
    let gpu = common::qualification_gpu().expect("GPU required for explicit EIP sensor qualification");
    eprintln!("EIP sensor qualification adapter: {}", gpu.adapter_name());
    for (folder, extension, original_path) in cases {
        let reference_budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut original_request = rrrah_decode::DecodeRequest::new(&original_path);
        original_request.memory_budget = Some(reference_budget.clone());
        let reference = rrrah_decode::NativeRawDecoder
            .decode(&original_request)
            .unwrap()
            .mosaic;
        let recipe = rrrah_decode::NativeRawDecoder
            .mosaic_recipe(&original_request)
            .unwrap();
        let expected = gpu.render(&reference, [128, 96]);
        assert!(expected.pixels.chunks_exact(4).all(|p| p[3] == 255));
        assert!(
            expected
                .pixels
                .chunks_exact(4)
                .any(|p| p[0] != p[1] || p[1] != p[2])
        );
        for encoding in ["stored", "deflated"] {
            let path = folder.join(format!("{extension}-{encoding}.eip"));
            let bytes = std::fs::read(&path).unwrap();
            let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
            let mut request = rrrah_decode::DecodeRequest::new("/nonexistent/eip-readback/source.eip");
            request.memory_budget = Some(budget.clone());
            let imported = rrrah_decode::decode_eip_sensor(&bytes, &request).unwrap();
            assert_eq!(imported.raw_recipe, recipe);
            let native = imported.decoded.mosaic;
            assert_eq!(native.metadata, reference.metadata);
            assert_eq!(native.pixels, reference.pixels);
            assert_eq!(gpu.render(&native, [128, 96]).pixels, expected.pixels);
            assert_eq!(native.thumbnail_rgba8(128), reference.thumbnail_rgba8(128));
            let fingerprint = rrrah_cache::SourceFingerprint::from_path(&path).unwrap();
            let key = rrrah_cache::CacheKey::for_mosaic_recipe(&fingerprint, 0, recipe);
            let original_key = rrrah_cache::CacheKey::for_mosaic_recipe(
                &rrrah_cache::SourceFingerprint::from_path(&original_path).unwrap(),
                0,
                recipe,
            );
            assert_ne!(
                key, original_key,
                "package fingerprint must not become inner RAW fingerprint"
            );
            let mut ram = rrrah_cache::MosaicRamCache::new(64 * 1024 * 1024);
            assert!(ram.insert(key, native.clone()));
            let lease = ram.get_lease(&key).unwrap();
            assert!(lease.pixels.ptr_eq(&native.pixels));
            assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
            assert_eq!(gpu.render(&lease, [128, 96]).pixels, expected.pixels);
            drop(lease);
            drop(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).unwrap());
            assert!(ram.is_empty());
            let directory = tempfile::tempdir().unwrap();
            let disk = rrrah_cache::DiskMosaicCache::with_max_bytes(
                directory.path().join("persistent"),
                64 * 1024 * 1024,
            );
            disk.store(key, &native).unwrap();
            let cached = disk.load_with_budget(key, &budget).unwrap().unwrap().mosaic;
            assert_eq!(cached.metadata, reference.metadata);
            assert_eq!(cached.pixels, reference.pixels);
            assert_eq!(gpu.render(&cached, [128, 96]).pixels, expected.pixels);
            drop(cached);
            let swap: rrrah_cache::ImageSwapCache<u8, rrrah_core::DecodedMosaic> =
                rrrah_cache::ImageSwapCache::new_with_budgets(
                    directory.path(),
                    rrrah_cache::ImageSwapConfig {
                        limits: rrrah_cache::CacheLimits {
                            max_bytes: 64 * 1024 * 1024,
                            max_entries: Some(1),
                            ttl: None,
                        },
                        queue_bytes: 64 * 1024 * 1024,
                        queue_count: 1,
                        restore_bytes: 128 * 1024 * 1024,
                    },
                    rrrah_core::MemoryBudget::new(64 * 1024 * 1024),
                    budget.clone(),
                )
                .unwrap();
            assert_eq!(
                swap.try_enqueue(1, native.clone()),
                rrrah_cache::SpillAdmission::Queued
            );
            swap.wait_idle().unwrap();
            drop(native);
            assert_eq!(budget.used(), 0);
            let pressure = budget.try_reserve(128 * 1024 * 1024).unwrap();
            assert!(swap.try_get(&1, || false).is_err());
            assert_eq!(swap.stats().errors, 0);
            drop(pressure);
            let restored = swap.try_get(&1, || false).unwrap().unwrap();
            assert!(restored.pixels.is_managed());
            assert_eq!(restored.metadata, reference.metadata);
            assert_eq!(restored.pixels, reference.pixels);
            assert_eq!(gpu.render(&restored, [128, 96]).pixels, expected.pixels);
            drop(restored);
            assert_eq!(budget.used(), 0);
            assert_eq!(
                (swap.stats().writes, swap.stats().reads, swap.stats().errors),
                (1, 1, 0)
            );
            assert_eq!(swap.stats().queued_bytes, 0);
            eprintln!(
                "{} {}: full sensor/metadata, CPU thumbnail, RAM/disk/swap/Metal frame exact",
                original_path.display(),
                encoding
            );
        }
        drop(reference);
        assert_eq!(reference_budget.used(), 0);
    }
}

#[test]
#[ignore = "manual real CR3 file packages and Metal qualification"]
fn file_eip_cr3_ram_disk_swap_preserves_complete_sensor_and_metal_frame() {
    use rrrah_decode::RawDecoder;
    let folder = std::path::PathBuf::from(std::env::var("RRRAH_EIP_FILE_CORPUS").unwrap());
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("EIP file adapter: {}", gpu.adapter_name());
    let original = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
    let reference_root = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let mut original_request = rrrah_decode::DecodeRequest::new(&original);
    original_request.memory_budget = Some(reference_root.clone());
    let reference = rrrah_decode::NativeRawDecoder
        .decode(&original_request)
        .unwrap()
        .mosaic;
    let expected = gpu.render(&reference, [128, 96]).pixels;
    for encoding in ["stored", "deflated"] {
        let path = folder.join(format!("{encoding}.eip"));
        let root = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        let imported = rrrah_decode::decode_eip_file_sensor(&request).unwrap();
        let key = rrrah_cache::CacheKey::for_mosaic_recipe(
            &rrrah_cache::SourceFingerprint::from_path(&path).unwrap(),
            0,
            imported.raw_recipe,
        );
        let native = imported.decoded.mosaic;
        assert_eq!(native.pixels, reference.pixels);
        assert_eq!(native.metadata, reference.metadata);
        assert_eq!(gpu.render(&native, [128, 96]).pixels, expected);
        let mut ram = rrrah_cache::MosaicRamCache::new(64 * 1024 * 1024);
        assert!(ram.insert(key, native.clone()));
        let lease = ram.get_lease(&key).unwrap();
        assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
        assert_eq!(gpu.render(&lease, [128, 96]).pixels, expected);
        drop(lease);
        drop(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).unwrap());
        assert!(ram.is_empty());
        let directory = tempfile::tempdir().unwrap();
        let disk = rrrah_cache::DiskMosaicCache::with_max_bytes(
            directory.path().join("persistent"),
            64 * 1024 * 1024,
        );
        disk.store(key, &native).unwrap();
        let cached = disk.load_with_budget(key, &root).unwrap().unwrap().mosaic;
        assert_eq!(cached.pixels, reference.pixels);
        assert_eq!(cached.metadata, reference.metadata);
        assert_eq!(gpu.render(&cached, [128, 96]).pixels, expected);
        drop(cached);
        std::fs::create_dir(directory.path().join("swap")).unwrap();
        let swap: rrrah_cache::ImageSwapCache<u8, rrrah_core::DecodedMosaic> =
            rrrah_cache::ImageSwapCache::new_with_budgets(
                &directory.path().join("swap"),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits::bytes(64 * 1024 * 1024),
                    queue_bytes: 64 * 1024 * 1024,
                    queue_count: 1,
                    restore_bytes: 64 * 1024 * 1024,
                },
                rrrah_cache::MemoryBudget::new(64 * 1024 * 1024),
                root.clone(),
            )
            .unwrap();
        assert_eq!(
            swap.try_enqueue(1, native.clone()),
            rrrah_cache::SpillAdmission::Queued
        );
        swap.wait_idle().unwrap();
        drop(native);
        assert_eq!(root.used(), 0);
        let pressure = root.try_reserve(root.available_bytes()).unwrap();
        assert!(swap.try_get(&1, || false).is_err());
        assert_eq!(swap.stats().errors, 0);
        drop(pressure);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(restored.pixels, reference.pixels);
        assert_eq!(restored.metadata, reference.metadata);
        assert_eq!(gpu.render(&restored, [128, 96]).pixels, expected);
        drop(restored);
        assert_eq!(root.used(), 0);
        assert_eq!(
            (swap.stats().writes, swap.stats().reads, swap.stats().errors),
            (1, 1, 0)
        );
        eprintln!(
            "EIP file {encoding}: all {} samples, metadata and Metal pixels exact; RAM/disk/swap; final=0",
            reference.pixels.len()
        );
    }
    drop(reference);
    assert_eq!(reference_root.used(), 0);
}
