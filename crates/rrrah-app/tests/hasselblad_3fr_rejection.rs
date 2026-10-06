#[test]
#[ignore = "requires pinned CC0 Hasselblad X1D 3FR object 2058 and LibRaw sensor dump"]
fn three_fr_routes_full_sensor_and_rejects_bad_metadata_without_leaks() {
    let source = std::fs::read(std::env::var("RRRAH_HASSELBLAD_3FR_SOURCE").unwrap()).unwrap();
    let reference = std::fs::read(std::env::var("RRRAH_HASSELBLAD_3FR_REFERENCE").unwrap()).unwrap();
    assert_eq!(&source[355..370], b"Hasselblad X1D\0");
    assert_eq!(&source[567..571], &65536u32.to_le_bytes());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.3FR");
    std::fs::write(&path, &source).unwrap();
    let budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&path);
    request.memory_budget = Some(budget.clone());
    let rrrah_decode::DecodedImage::Sensor(decoded) = rrrah_decode::decode_image(&request).unwrap() else {
        panic!("3FR selected preview instead of sensor");
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
            0 => bytes[567..571].fill(0),                 // Zero AsShotNeutral.
            1 => bytes[571..575].fill(0),                 // Rational denominator zero.
            2 => bytes[495..567].fill(0),                 // Singular/invalid matrix.
            3 => bytes[366..369].copy_from_slice(b"X9D"), // Unknown camera profile.
            4 => bytes.truncate(4673536 + 105705472 - 1),
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
