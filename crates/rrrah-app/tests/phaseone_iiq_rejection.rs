#[test]
#[ignore = "requires pinned CC0 Phase One P20+ IIQ object 4366 and LibRaw sensor dump"]
fn iiq_routes_full_sensor_and_rejects_bad_metadata_without_leaks() {
    let source = std::fs::read(std::env::var("RRRAH_PHASEONE_IIQ_SOURCE").unwrap()).unwrap();
    let reference = std::fs::read(std::env::var("RRRAH_PHASEONE_IIQ_REFERENCE").unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.IIQ");
    std::fs::write(&path, &source).unwrap();
    let budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&path);
    request.memory_budget = Some(budget.clone());
    let rrrah_decode::DecodedImage::Sensor(decoded) = rrrah_decode::decode_image(&request).unwrap() else {
        panic!("IIQ selected preview instead of sensor");
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
    for case in 0..7 {
        let mut bytes = source.clone();
        match case {
            0 => bytes[20583392..20583396].copy_from_slice(&0f32.to_le_bytes()),
            1 => bytes[20583392..20583396].copy_from_slice(&f32::NAN.to_le_bytes()),
            2 => bytes[20608244] = b'X',
            3 => bytes[20583476..20583480].copy_from_slice(&1u32.to_le_bytes()),
            4 => bytes.truncate(21348900),
            5 => bytes[20608740 + 16..20608740 + 18].copy_from_slice(&746u16.to_le_bytes()),
            6 => bytes[21348968 + 8..21348968 + 12].copy_from_slice(&0x419u32.to_le_bytes()),
            _ => unreachable!(),
        }
        std::fs::write(&path, bytes).unwrap();
        assert!(rrrah_decode::decode_image(&request).is_err(), "case {case}");
        assert_eq!(budget.used(), 0, "case {case}");
    }
    std::fs::write(&path, &source).unwrap();
    let small = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
    request.memory_budget = Some(small.clone());
    assert!(rrrah_decode::decode_image(&request).is_err());
    assert_eq!(small.used(), 0);
}
