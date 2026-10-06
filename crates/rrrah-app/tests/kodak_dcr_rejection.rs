#[test]
#[ignore = "requires pinned CC0 Kodak DCS760C DCR object 1347 and LibRaw sensor dump"]
fn dcr_routes_full_sensor_and_rejects_bad_metadata_without_leaks() {
    let source = std::fs::read(std::env::var("RRRAH_KODAK_DCR_SOURCE").unwrap()).unwrap();
    let reference = std::fs::read(std::env::var("RRRAH_KODAK_DCR_REFERENCE").unwrap()).unwrap();
    assert_eq!(&source[14028..14036], b"DCS760C\0");
    assert_eq!(&source[17600..17604], &2206u32.to_be_bytes());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.DCR");
    std::fs::write(&path, &source).unwrap();
    let budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&path);
    request.memory_budget = Some(budget.clone());
    let rrrah_decode::DecodedImage::Sensor(decoded) = rrrah_decode::decode_image(&request).unwrap() else {
        panic!("DCR selected preview instead of sensor");
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
            0 => bytes[17600..17604].fill(0), // Zero WB numerator.
            1 => bytes[17604..17608].fill(0), // Zero rational denominator.
            2 => bytes[18824..18826].copy_from_slice(&65535u16.to_be_bytes()),
            3 => bytes[14028..14035].copy_from_slice(b"DCS999C"),
            4 => bytes.truncate(804352 + 5703212 - 1),
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
