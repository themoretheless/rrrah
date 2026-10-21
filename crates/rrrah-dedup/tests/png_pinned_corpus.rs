#![cfg(feature = "decode")]

use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::decode::decode_selected_frame;

#[test]
#[ignore = "requires the SHA-verified pinned spng PNG corpus in RRR_PNG_CORPUS"]
fn pinned_png_corpus_decodes_and_adam7_matches_noninterlaced_pixels() {
    let directory = std::path::PathBuf::from(std::env::var_os("RRR_PNG_CORPUS").expect("RRR_PNG_CORPUS"));
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut paths: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "png")
                && !path.file_name().unwrap().to_string_lossy().starts_with('x')
        })
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 161, "expected the pinned valid PNG corpus");
    let mut failures = Vec::new();
    for path in &paths {
        {
            let bytes = std::fs::read(path).unwrap();
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
            let frame = match decode_selected_frame(&DecodeRequest::new(path), 1_000_000, &budget, || false) {
                Ok(frame) => frame,
                Err(error) => {
                    failures.push(format!(
                        "{}: {error}",
                        path.file_name().unwrap().to_string_lossy()
                    ));
                    assert_eq!(
                        budget.used(),
                        0,
                        "failed decode retained budget: {}",
                        path.display()
                    );
                    continue;
                }
            };
            let view = frame.view(|| false).unwrap();
            assert_eq!(view.dimensions(), (width, height), "{}", path.display());
            for y in 0..height {
                for x in 0..width {
                    let rgba = view.rgba(x, y).unwrap();
                    assert!(
                        rgba.iter().all(|value| value.is_finite()),
                        "{} ({x},{y}): {rgba:?}",
                        path.display()
                    );
                    assert!(
                        (0.0..=1.0).contains(&rgba[3]),
                        "{} ({x},{y}): {rgba:?}",
                        path.display()
                    );
                }
            }
            // Exercise the exact pixel digest, including normalized alpha.
            frame.selected_frame_digest(|| false).unwrap();
        }
        assert_eq!(budget.used(), 0, "{}", path.display());
    }
    eprintln!(
        "valid corpus decoded: {}/161; failures: {failures:#?}",
        161 - failures.len()
    );
    assert!(failures.is_empty(), "valid PNG decode failures: {failures:#?}");
    let mut pairs = 0;
    for path in paths
        .iter()
        .filter(|path| path.file_name().unwrap().to_string_lossy().starts_with("basi"))
    {
        {
            let name = path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .replacen("basi", "basn", 1);
            let counterpart = directory.join(name);
            let load = |path| {
                decode_selected_frame(&DecodeRequest::new(path), 1_000_000, &budget, || false).unwrap()
            };
            let interlaced = load(path);
            let plain = load(&counterpart);
            assert!(
                interlaced.same_selected_frame(&plain, || false).unwrap(),
                "{}",
                path.display()
            );
            assert_eq!(
                interlaced.selected_frame_digest(|| false).unwrap(),
                plain.selected_frame_digest(|| false).unwrap(),
                "{}",
                path.display()
            );
        }
        assert_eq!(budget.used(), 0);
        pairs += 1;
    }
    assert_eq!(pairs, 15);
}
