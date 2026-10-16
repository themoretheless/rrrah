//! Scientific managed-memory regressions and manual volume/cancellation probes.
use crate::{DecodeRequest, GenerationToken, RasterDecodeError};
use std::{
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
const EDGE: usize = 1024;
fn nrrd() -> Vec<u8> {
    let mut bytes =
        format!("NRRD0005\ntype: uint8\ndimension: 2\nsizes: {EDGE} {EDGE}\nencoding: raw\n\n").into_bytes();
    bytes.resize(bytes.len() + EDGE * EDGE, 7);
    bytes
}
fn mrc() -> Vec<u8> {
    let mut header = include_bytes!("../../../tests/fixtures/raster/mrc-i8-little.mrc")[..1024].to_vec();
    for (at, value) in [(0, EDGE as u32), (4, EDGE as u32), (8, 1), (92, 0)] {
        header[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    header.resize(1024 + EDGE * EDGE, 7);
    header
}
fn fits() -> Vec<u8> {
    let original = include_bytes!("../../../tests/fixtures/raster/fits-u8.fits");
    let mut header = original[..2880].to_vec();
    for card in header.chunks_exact_mut(80) {
        let value = match &card[..8] {
            b"NAXIS1  " | b"NAXIS2  " => EDGE,
            b"NAXIS3  " => 1,
            _ => continue,
        };
        card[10..30].copy_from_slice(format!("{value:>20}").as_bytes());
    }
    header.resize(2880 + EDGE * EDGE, 7);
    header.resize(header.len().div_ceil(2880) * 2880, 0);
    header
}
#[test]
#[ignore = "manual concurrent cancellation after observed managed output admission"]
fn cancellation_after_large_output_admission_preserves_other_owners() {
    let sources = [("NRRD", nrrd()), ("MRC", mrc()), ("FITS", fits())];
    for (format, bytes) in sources {
        let root = rrrah_core::MemoryBudget::new(20 * 1024 * 1024);
        let unrelated = root.try_reserve(4096).unwrap();
        let generation = Arc::new(AtomicU64::new(1));
        let mut request = DecodeRequest::new("cancel-stress");
        request.memory_budget = Some(root.clone());
        request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let barrier = Arc::new(Barrier::new(2));
        std::thread::scope(|scope| {
            let budget = root.clone();
            let start = barrier.clone();
            let owner = generation.clone();
            let observer = scope.spawn(move || {
                start.wait();
                let deadline = Instant::now() + Duration::from_secs(3);
                while budget.used() < (EDGE * EDGE * 16) as u64 {
                    if Instant::now() >= deadline {
                        owner.store(2, Ordering::Release);
                        return false;
                    }
                    std::thread::yield_now();
                }
                owner.store(2, Ordering::Release);
                true
            });
            barrier.wait();
            let result = match format {
                "NRRD" => crate::nrrd::decode(&bytes, &request).map(|_| ()),
                "MRC" => crate::mrc::decode(&bytes, &request).map(|_| ()),
                _ => crate::fits::decode_raster(&bytes, &request).map(|_| ()),
            };
            assert!(
                observer.join().unwrap(),
                "{format}: output admission was not observed"
            );
            assert!(
                matches!(
                    result,
                    Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
                ),
                "{format}: {result:?}"
            );
        });
        assert_eq!(root.used(), 4096, "{format}: partial decoder ownership leaked");
        eprintln!(
            "SCIENTIFIC_CANCEL format={format} output_bytes={} peak={} unrelated=4096 cancelled=true released=true",
            EDGE * EDGE * 16,
            root.peak()
        );
        drop(unrelated);
        assert_eq!(root.used(), 0);
    }
}

fn inherited_context<T: Clone>(
    decode: impl Fn(&DecodeRequest) -> T,
    display: impl Fn(&T) -> Result<rrrah_core::DecodedRaster, RasterDecodeError>,
    live_bytes: u64,
    redisplay: impl Fn(&T, &DecodeRequest) -> Result<rrrah_core::DecodedRaster, RasterDecodeError>,
) {
    let budget = rrrah_core::MemoryBudget::new(live_bytes + 240);
    let generation = Arc::new(AtomicU64::new(1));
    let mut request = DecodeRequest::new("inherited-context");
    request.image_index = 1;
    request.memory_budget = Some(budget.clone());
    request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
    let image = decode(&request);
    assert_eq!(budget.used(), live_bytes);
    let output = display(&image).unwrap();
    assert_eq!(budget.used(), live_bytes + 240);
    let rrrah_core::RasterPixels::Rgba32Float(pixels) = output.pixels() else {
        panic!()
    };
    let expected: Vec<_> = pixels.iter().map(|v| v.to_bits()).collect();
    let selection = (output.image_index(), output.image_count());
    assert_eq!(selection, (1, 2));
    drop(output);
    assert_eq!(budget.used(), live_bytes);
    let pressure = budget.try_reserve(1).unwrap();
    assert!(matches!(
        display(&image),
        Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
    ));
    assert_eq!(budget.used(), live_bytes + 1);
    drop(pressure);
    let clone = image.clone();
    drop(image);
    generation.store(2, Ordering::Release);
    assert!(matches!(
        display(&clone),
        Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
    ));
    assert_eq!(budget.used(), live_bytes);
    let fresh_generation = Arc::new(AtomicU64::new(1));
    let mut fresh = DecodeRequest::new("already-decoded; no file IO");
    fresh.memory_budget = Some(budget.child(239));
    fresh.cancellation = Some(GenerationToken::new(fresh_generation.clone(), 1));
    assert!(matches!(
        redisplay(&clone, &fresh),
        Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
    ));
    assert_eq!(budget.used(), live_bytes);
    let output_budget = budget.child(240);
    fresh.memory_budget = Some(output_budget.clone());
    let output = redisplay(&clone, &fresh).unwrap();
    let rrrah_core::RasterPixels::Rgba32Float(pixels) = output.pixels() else {
        panic!()
    };
    assert_eq!(pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
    assert_eq!(output_budget.used(), 240);
    assert_eq!((output.image_index(), output.image_count()), selection);
    fresh_generation.store(2, Ordering::Release);
    assert!(matches!(
        redisplay(&clone, &fresh),
        Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
    ));
    drop(clone);
    assert_eq!(budget.used(), 240);
    drop(output);
    assert_eq!((budget.used(), output_budget.used()), (0, 0));
}
#[test]
fn convenience_display_inherits_budget_and_cancellation_after_clone() {
    inherited_context(
        |r| {
            crate::nrrd::decode(
                include_bytes!("../../../tests/fixtures/raster/nrrd-u8-raw-little.nrrd"),
                r,
            )
            .unwrap()
        },
        |image| image.windowed(-100., 1000.),
        240 + crate::nrrd::header_credit(
            include_bytes!("../../../tests/fixtures/raster/nrrd-u8-raw-little.nrrd"),
            &DecodeRequest::new("header"),
        )
        .unwrap(),
        |image, request| image.windowed_with_request(-100., 1000., request),
    );
    inherited_context(
        |r| {
            crate::mrc::decode(
                include_bytes!("../../../tests/fixtures/raster/mrc-i8-little.mrc"),
                r,
            )
            .unwrap()
        },
        |image| image.windowed(-100., 1000.),
        272,
        |image, request| image.windowed_with_request(-100., 1000., request),
    );
    inherited_context(
        |r| crate::fits::decode(include_bytes!("../../../tests/fixtures/raster/fits-u8.fits"), r).unwrap(),
        |image| image.windowed(-100., 1000.),
        15 + (crate::fits::decode(
            include_bytes!("../../../tests/fixtures/raster/fits-u8.fits"),
            &DecodeRequest::new("header"),
        )
        .unwrap()
        .cards
        .len()
            * 80) as u64,
        |image, request| image.windowed_with_request(-100., 1000., request),
    );
    inherited_context(
        |r| crate::fits::decode(include_bytes!("../../../tests/fixtures/raster/fits-u8.fits"), r).unwrap(),
        crate::fits::FitsImage::to_raster,
        15 + (crate::fits::decode(
            include_bytes!("../../../tests/fixtures/raster/fits-u8.fits"),
            &DecodeRequest::new("header"),
        )
        .unwrap()
        .cards
        .len()
            * 80) as u64,
        crate::fits::FitsImage::to_raster_with_request,
    );
}

#[test]
#[ignore = "manual 256 MiB gzip NRRD full-volume scan with selected-plane managed admission"]
fn large_gzip_volume_selected_planes_keep_bounded_memory() {
    use std::io::Write;
    const SIDE: usize = 512;
    const SECTIONS: usize = 512;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("volume.nrrd");
    let mut file = std::fs::File::create(&path).unwrap();
    writeln!(file, "NRRD0005\ntype: uint16\ndimension: 3\nsizes: {SIDE} {SIDE} {SECTIONS}\nkinds: domain domain domain\nencoding: gzip\nendian: little\n").unwrap();
    let mut gzip = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
    let mut plane = vec![0u8; SIDE * SIDE * 2];
    for section in 0..SECTIONS {
        for (i, sample) in plane.chunks_exact_mut(2).enumerate() {
            sample.copy_from_slice(&(section as u16 * 17 + (i % 257) as u16).to_le_bytes());
        }
        gzip.write_all(&plane).unwrap();
    }
    drop(gzip.finish().unwrap());
    drop(plane);
    let source_bytes = std::fs::metadata(&path).unwrap().len();
    let metadata_credit =
        crate::nrrd::header_credit(&std::fs::read(&path).unwrap(), &DecodeRequest::new("header")).unwrap();
    let output_bytes = (SIDE * SIDE * 16) as u64;
    let scratch_bytes = (SIDE * SIDE * 2) as u64;
    for section in [0usize, SECTIONS / 2, SECTIONS - 1] {
        let cap = source_bytes + metadata_credit + scratch_bytes + output_bytes;
        let budget = rrrah_core::MemoryBudget::new(cap);
        let mut request = DecodeRequest::new(&path);
        request.image_index = section;
        request.memory_budget = Some(budget.clone());
        let started = Instant::now();
        let image = crate::nrrd::decode_nrrd(&request).unwrap();
        let elapsed = started.elapsed().as_nanos();
        assert_eq!(
            (budget.used(), budget.peak()),
            (output_bytes + metadata_credit, cap)
        );
        assert_eq!(
            (image.raster.image_index(), image.raster.image_count()),
            (section, SECTIONS)
        );
        let rrrah_core::RasterPixels::Rgba32Float(pixels) = image.raster.pixels() else {
            panic!()
        };
        for (i, pixel) in pixels.chunks_exact(4).enumerate() {
            let expected = (section * 17 + i % 257) as f32;
            assert_eq!(pixel, &[expected, expected, expected, 1.]);
        }
        drop(image);
        assert_eq!(budget.used(), 0);
        eprintln!(
            "LARGE_NRRD section={section} source_bytes={source_bytes} volume_bytes={} output_bytes={output_bytes} scratch_bytes={scratch_bytes} cap={cap} peak={} decode_ns={elapsed} final=0",
            SIDE * SIDE * SECTIONS * 2,
            budget.peak()
        );
        let window_cap =
            (source_bytes + metadata_credit + scratch_bytes + output_bytes).max(output_bytes * 2);
        let window_budget = rrrah_core::MemoryBudget::new(window_cap);
        request.memory_budget = Some(window_budget.clone());
        let window = crate::ScalarWindow::new(0., 9000.).unwrap();
        let started = Instant::now();
        let raster = crate::decode_raster_with_window(&request, window).unwrap();
        let elapsed = started.elapsed().as_nanos();
        assert_eq!(
            (window_budget.used(), window_budget.peak()),
            (output_bytes, window_cap)
        );
        let rrrah_core::RasterPixels::Rgba32Float(pixels) = raster.pixels() else {
            panic!()
        };
        for (i, pixel) in pixels.chunks_exact(4).enumerate() {
            let expected = (section * 17 + i % 257) as f32 / 9000.;
            assert_eq!(pixel, &[expected, expected, expected, 1.]);
        }
        drop(raster);
        assert_eq!(window_budget.used(), 0);
        eprintln!(
            "LARGE_NRRD_WINDOW section={section} cap={} peak={} decode_ns={elapsed} final=0",
            window_cap,
            window_budget.peak()
        );
    }
}
