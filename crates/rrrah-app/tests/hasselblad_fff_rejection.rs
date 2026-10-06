#[test]
#[ignore = "requires pinned CC0 Hasselblad CFV-50 FFF object 1637 and LibRaw sensor dump"]
fn fff_routes_full_sensor_and_rejects_bad_metadata_without_leaks() {
    let source = std::fs::read(std::env::var("RRRAH_HASSELBLAD_FFF_SOURCE").unwrap()).unwrap();
    let reference = std::fs::read(std::env::var("RRRAH_HASSELBLAD_FFF_REFERENCE").unwrap()).unwrap();
    assert_eq!(&source[412..430], b"Hasselblad CFV-50\0");
    assert_eq!(&source[2356..2360], &8192u32.to_be_bytes());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.FFF");
    std::fs::write(&path, &source).unwrap();
    let budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&path);
    request.memory_budget = Some(budget.clone());
    let rrrah_decode::DecodedImage::Sensor(decoded) = rrrah_decode::decode_image(&request).unwrap() else {
        panic!("FFF selected preview instead of sensor");
    };
    assert!(decoded.mosaic.pixels.is_managed());
    assert_eq!(decoded.mosaic.pixels.len() * 2, reference.len());
    assert!(
        decoded
            .mosaic
            .pixels
            .iter()
            .zip(reference.chunks_exact(2))
            .all(|(a, b)| *a == u16::from_le_bytes([b[0], b[1]]))
    );
    drop(decoded);
    assert_eq!(budget.used(), 0);
    for case in 0..5 {
        let mut bytes = source.clone();
        match case {
            0 => bytes[2356..2360].fill(0), // Zero AsShotNeutral.
            1 => bytes[2360..2364].fill(0), // Zero rational denominator.
            2 => bytes[9825436 + 63] = 1,   // Unsupported point transform.
            3 => bytes[423..429].copy_from_slice(b"CFV-99"),
            4 => bytes.truncate(9825436 + 69614078 - 1),
            _ => unreachable!(),
        }
        std::fs::write(&path, bytes).unwrap();
        assert!(rrrah_decode::decode_image(&request).is_err(), "case {case}");
        assert_eq!(budget.used(), 0, "case {case}");
    }
    std::fs::write(&path, &source).unwrap();
    let small = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    request.memory_budget = Some(small.clone());
    assert!(rrrah_decode::decode_image(&request).is_err());
    assert_eq!(small.used(), 0);
}
