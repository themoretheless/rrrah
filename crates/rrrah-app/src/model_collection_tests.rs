//! Opt-in production loader collection stress; renamed fixture copies test keys,
//! cache lifetime and I/O, not independent producer/content coverage.
use super::*;
use std::collections::BTreeMap;
use std::sync::Mutex;
type Audit = (PathBuf, BTreeMap<PathBuf, usize>);
static AUDIT: Mutex<Option<Audit>> = Mutex::new(None);
pub(super) fn observe_decode(path: &Path) {
    let mut audit = AUDIT.lock().unwrap();
    if let Some((prefix, counts)) = audit.as_mut()
        && path.starts_with(prefix)
    {
        *counts.entry(path.to_path_buf()).or_default() += 1;
    }
}
struct AuditReset;
impl Drop for AuditReset {
    fn drop(&mut self) {
        *AUDIT.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}
#[test]
#[ignore = "opt-in large filesystem collection and real asynchronous native swap"]
fn production_collection_prefetch_count_ttl_and_native_restore() {
    let count = std::env::var("RRRAH_MODEL_COLLECTION_COUNT").map_or(1024, |v| v.parse::<usize>().unwrap());
    assert!((16..=10000).contains(&count));
    let directory = tempfile::tempdir().unwrap();
    let sources = directory.path().join("sources");
    std::fs::create_dir(&sources).unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let inputs = [
        "glb/BoxTextured.glb",
        "models/triangle.stl",
        "models/off-triangle.off",
        "models/obj-shared-metadata.obj",
        "models/ply-64-le.ply",
    ];
    let expected: Vec<_> = inputs
        .iter()
        .map(|p| {
            let asset = model_asset::ModelAsset::decode(&DecodeRequest::new(fixtures.join(p))).unwrap();
            (asset.format_name(), asset.triangle_count())
        })
        .collect();
    let paths: Vec<_> = (0..count)
        .map(|i| {
            let input = fixtures.join(inputs[i % inputs.len()]);
            let path = sources.join(format!("{i:05}.{}", input.extension().unwrap().to_str().unwrap()));
            std::fs::copy(input, &path).unwrap();
            path
        })
        .collect();
    *AUDIT.lock().unwrap() = Some((sources, BTreeMap::new()));
    let _reset = AuditReset;
    let root = rrrah_core::MemoryBudget::new(4 * 1024 * 1024);
    let (tx, rx) = unbounded();
    let (prefetch_tx, prefetch_rx) = unbounded();
    let (cache_tx, cache_rx) = unbounded();
    let zero = rrrah_cache::CacheLimits::bytes(0);
    let mut loader = ForegroundLoader::new(
        Some(directory.path().join("cache")),
        false,
        zero,
        tx,
        LoaderWake::ObserveCollection {
            prefetch: prefetch_tx,
            cache: cache_tx,
        },
        Arc::new(DecodeGate::new()),
        Arc::new(CacheTelemetry::new(false, 0)),
        None,
        None,
        Some(rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 256 * 1024 * 1024,
                max_entries: Some(count),
                ttl: None,
            },
            queue_bytes: 4 * 1024 * 1024,
            queue_count: 8,
            restore_bytes: 2 * 1024 * 1024,
        }),
        zero,
        0,
        Some(root.clone()),
        zero,
        rrrah_cache::CacheLimits {
            max_bytes: 1024 * 1024,
            max_entries: Some(3),
            ttl: Some(Duration::from_secs(1)),
        },
        gallery::PrefetchWindow { ahead: 1, behind: 1 },
        UntaggedColor::Strict.into(),
    )
    .unwrap();
    let mut max_entries = 0;
    let mut max_bytes = 0;
    let mut reads = 0;
    let mut writes = 0;
    let mut pending = 0;
    let mut retained = 0;
    let mut visits = 0;
    // Walk the full collection in both directions; await each retained neighbour
    // before advancing, preserving the real generation/foreground protocol.
    for index in (0..count).chain((0..count - 1).rev()) {
        let neighbours: Vec<_> = [index.checked_sub(1), (index + 1 < count).then_some(index + 1)]
            .into_iter()
            .flatten()
            .map(|i| paths[i].clone())
            .collect();
        let generation = loader
            .submit_request_with_prefetch(paths[index].clone(), 0, None, true, Some(neighbours.clone()))
            .unwrap();
        match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            LoadEvent::ModelReady {
                generation: g, mesh, ..
            } => {
                assert_eq!(g, generation);
                assert_eq!(
                    (mesh.format_name(), mesh.triangle_count()),
                    expected[index % inputs.len()]
                );
            }
            other => panic!("collection selection {index} failed: {other:?}"),
        }
        let (g, entries, bytes, r, w, p) = cache_rx.recv_timeout(Duration::from_secs(30)).unwrap();
        assert_eq!(g, generation);
        assert!(entries <= 3);
        assert!(bytes <= 1024 * 1024);
        max_entries = max_entries.max(entries);
        max_bytes = max_bytes.max(bytes);
        reads = r;
        writes = w;
        pending = p;
        for expected_path in neighbours {
            let (g, path, kept) = prefetch_rx.recv_timeout(Duration::from_secs(30)).unwrap();
            assert_eq!(g, generation);
            assert_eq!(path, expected_path);
            assert!(kept);
            retained += 1;
        }
        visits += 1;
        if visits % 128 == 0 {
            eprintln!(
                "MODEL_COLLECTION_PROGRESS visits={visits} reads={reads} writes={writes} managed_used={}",
                root.used()
            );
        }
    }
    loader.shutdown();
    drop(loader);
    drop(rx);
    let counts = AUDIT.lock().unwrap().take().unwrap().1;
    assert_eq!(counts.len(), count);
    assert!(
        counts.values().all(|n| *n == 1),
        "return/preload must restore native swap without source decoding: {:?}",
        counts
            .iter()
            .filter(|(_, n)| **n != 1)
            .take(8)
            .collect::<Vec<_>>()
    );
    assert!(reads > 0);
    assert!(reads + pending >= (count - 3) as u64);
    assert!(writes >= (count - 3) as u64);
    assert_eq!(root.used(), 0);
    #[cfg(feature = "qualification-slow-io")]
    {
        let io = rrrah_cache::qualification_io::snapshot();
        if std::env::var_os("RRRAH_QUALIFY_SWAP_DELAY_MS").is_some() {
            assert!(io.read_calls > 0 && io.write_calls > 0);
            assert!(io.completed_spills > 0 && io.completed_restores > 0);
        }
        eprintln!("MODEL_COLLECTION_IO {io:?}");
    }
    #[cfg(not(feature = "qualification-slow-io"))]
    assert!(
        std::env::var_os("RRRAH_QUALIFY_SWAP_DELAY_MS").is_none(),
        "delayed I/O requires --features qualification-slow-io"
    );
    eprintln!(
        "MODEL_COLLECTION_PASS sources={count} visits={visits} retained={retained} native_decodes={} swap_reads={reads} pending_handoffs={pending} completed_writes={writes} max_entries={max_entries} max_resident={max_bytes} managed_peak={} final=0",
        counts.len(),
        root.peak()
    );
}

#[test]
fn tentative_warm_models_do_not_override_a_higher_priority_cold_neighbour() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let root = rrrah_core::MemoryBudget::new(2 * 1024 * 1024);
    let request = |p: &str| {
        let mut r = DecodeRequest::new(fixtures.join(p));
        r.memory_budget = Some(root.clone());
        r
    };
    let visible = request("models/off-triangle.off");
    let warm = request("models/triangle.stl");
    let cold = request("glb/BoxTextured.glb");
    let mut cache = ModelCache::<model_asset::ModelAsset>::new(rrrah_cache::CacheLimits {
        max_bytes: root.limit(),
        max_entries: Some(2),
        ttl: None,
    });
    drop(
        cache
            .load(&visible, true, || model_asset::ModelAsset::decode(&visible))
            .unwrap(),
    );
    drop(
        cache
            .load(&warm, false, || model_asset::ModelAsset::decode(&warm))
            .unwrap(),
    );
    let mut future = vec![cache.prefetch_lease(&warm).unwrap()];
    let (model, lease) =
        preload_model_window(&cold, &mut cache, &Arc::new(DecodeGate::new()), &mut future).unwrap();
    assert_eq!(model.format_name(), "GLB");
    assert!(lease.is_some());
    assert!(future.is_empty());
    assert!(cache.prefetch_lease(&warm).is_none());
    assert!(cache.prefetch_lease(&visible).is_some());
    drop((lease, model, cache));
    assert_eq!(root.used(), 0);
}
