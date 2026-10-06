#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
fn managed_model_decoding_preserves_gpu_pixels_and_releases_cpu_credit() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let paths = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
    for name in [
        "triangle.stl",
        "obj-negative-quad.obj",
        "ply-offset64-be.ply",
        "off-concave-u.off",
    ] {
        let mut request = rrrah_decode::DecodeRequest::new(paths.join(name));
        let legacy = rrrah_decode::decode_model(&request).unwrap();
        let reference = gpu.render_model(&legacy.triangles().collect::<Vec<_>>(), [64, 64], 0., 0.);
        let budget = rrrah_core::MemoryBudget::new(128 * 1024);
        request.memory_budget = Some(budget.clone());
        let model = rrrah_decode::decode_model(&request).unwrap();
        assert_eq!(budget.used(), model.capacity_bytes(), "{name}");
        let actual = gpu.render_model(&model.triangles().collect::<Vec<_>>(), [64, 64], 0., 0.);
        assert_eq!(actual.pixels, reference.pixels, "{name}");
        drop(model);
        assert_eq!(budget.used(), 0, "{name}");
    }
}

#[test]
fn decoded_obj_notch_survives_common_model_and_gpu_upload() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("OBJ readback adapter: {}", gpu.adapter_name());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
    let model = rrrah_decode::decode_model(&rrrah_decode::DecodeRequest::new(root.join("obj-concave-u.obj")))
        .unwrap();
    let decoded: Vec<_> = model.triangles().collect();
    assert_eq!(decoded.len(), 6);
    let bytes = std::fs::read(root.join("obj-concave-u.obj.triangles.f32le")).unwrap();
    let authored: Vec<[[f32; 3]; 3]> = bytes
        .chunks_exact(36)
        .map(|v| {
            std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    f32::from_le_bytes(v[(i * 3 + j) * 4..(i * 3 + j + 1) * 4].try_into().unwrap())
                })
            })
        })
        .collect();
    let frame = gpu.render_model(&decoded, [64, 64], 0., 0.);
    let reference = gpu.render_model(&authored, [64, 64], 0., 0.);
    assert_eq!(
        frame.pixels, reference.pixels,
        "triangulation changed visible surface"
    );
    assert!(frame.pixel(32, 19)[0] < 40, "notch filled");
    for [x, y] in [[19, 19], [45, 19], [32, 45]] {
        assert!(frame.pixel(x, y)[0] > 100, "missing surface at {x},{y}");
    }
}

#[test]
fn native_float64_ply_reaches_gpu_before_precision_reduction() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("PLY readback adapter: {}", gpu.adapter_name());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
    let origin =
        rrrah_decode::decode_model(&rrrah_decode::DecodeRequest::new(root.join("ply-64-le.ply"))).unwrap();
    let reference: Vec<_> = origin.triangles().collect();
    let expected = gpu.render_model(&reference, [64, 64], 0., 0.);
    for suffix in ["ascii", "le", "be"] {
        let path = root.join(format!("ply-offset64-{suffix}.ply"));
        let model = rrrah_decode::decode_model(&rrrah_decode::DecodeRequest::new(&path)).unwrap();
        let vertices: Vec<_> = model.triangles().collect();
        assert_eq!(
            expected.pixels,
            gpu.render_model(&vertices, [64, 64], 0., 0.).pixels,
            "{}",
            path.display()
        );
        let path = root.join(format!("ply-u64-{suffix}.ply"));
        let model = rrrah_decode::decode_model(&rrrah_decode::DecodeRequest::new(&path)).unwrap();
        let vertices: Vec<_> = model.triangles().collect();
        let bytes = std::fs::read(root.join(format!("ply-u64-{suffix}.ply.triangles.f64le"))).unwrap();
        let oracle: Vec<[[f64; 3]; 3]> = bytes
            .chunks_exact(72)
            .map(|v| {
                std::array::from_fn(|i| {
                    std::array::from_fn(|j| {
                        f64::from_le_bytes(v[(i * 3 + j) * 8..(i * 3 + j + 1) * 8].try_into().unwrap())
                    })
                })
            })
            .collect();
        assert_eq!(
            gpu.render_model(&vertices, [64, 64], 0., 0.).pixels,
            gpu.render_model(&oracle, [64, 64], 0., 0.).pixels
        );
    }
}

#[test]
fn off_binary_attributes_float64_and_concave_surface_reach_gpu() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("OFF readback adapter: {}", gpu.adapter_name());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
    for name in [
        "off-attributes.off",
        "off-attributes-binary.off",
        "off-offset64.off",
        "off-concave-u.off",
    ] {
        let model = rrrah_decode::decode_model(&rrrah_decode::DecodeRequest::new(root.join(name))).unwrap();
        let decoded: Vec<_> = model.triangles().collect();
        let bytes = std::fs::read(root.join(format!("{name}.triangles.f64le"))).unwrap();
        let oracle: Vec<[[f64; 3]; 3]> = bytes
            .chunks_exact(72)
            .map(|v| {
                std::array::from_fn(|i| {
                    std::array::from_fn(|j| {
                        f64::from_le_bytes(v[(i * 3 + j) * 8..(i * 3 + j + 1) * 8].try_into().unwrap())
                    })
                })
            })
            .collect();
        assert_eq!(
            gpu.render_model(&decoded, [64, 64], 0., 0.).pixels,
            gpu.render_model(&oracle, [64, 64], 0., 0.).pixels,
            "{name}"
        );
    }
}

#[path = "../src/model_swap.rs"]
mod model_swap;

#[test]
fn real_model_swap_preserves_gpu_pixels_and_releases_restored_geometry() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("Model swap readback adapter: {}", gpu.adapter_name());
    let parent = tempfile::tempdir().unwrap();
    let root = rrrah_core::MemoryBudget::new(128 * 1024);
    let cache: rrrah_cache::ImageSwapCache<usize, model_swap::ModelPayload> =
        rrrah_cache::ImageSwapCache::new_with_budgets(
            parent.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 1024 * 1024,
                    max_entries: Some(8),
                    ttl: None,
                },
                queue_bytes: 128 * 1024,
                queue_count: 4,
                restore_bytes: 128 * 1024,
            },
            rrrah_core::MemoryBudget::new(128 * 1024),
            root.clone(),
        )
        .unwrap();
    let paths = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
    for (key, name) in [
        "triangle.stl",
        "obj-shared-metadata.obj",
        "off-concave-u.off",
        "ply-32-ascii.ply",
        "ply-offset64-be.ply",
        "ply-u64-le.ply",
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = rrrah_decode::DecodeRequest::new(paths.join(name));
        request.memory_budget = Some(root.clone());
        let original = rrrah_decode::decode_model(&request).unwrap();
        let reference = gpu.render_model(&original.triangles().collect::<Vec<_>>(), [64, 64], 0., 0.);
        cache.enqueue(key, model_swap::ModelPayload::from_model(original).unwrap());
        cache.wait_idle().unwrap();
        assert_eq!(root.used(), 0, "source release: {name}");
        let restored = cache.try_get(&key, || false).unwrap().unwrap();
        assert_eq!(root.used(), restored.0.capacity_bytes(), "restore credit: {name}");
        let actual = gpu.render_model(&restored.0.triangles().collect::<Vec<_>>(), [64, 64], 0., 0.);
        assert_eq!(actual.pixels, reference.pixels, "{name}");
        drop(restored);
        assert_eq!(root.used(), 0, "restore release: {name}");
    }
    assert_eq!(cache.stats().writes, 6);
    assert_eq!(cache.stats().reads, 6);
    assert_eq!(cache.stats().errors, 0);
}
