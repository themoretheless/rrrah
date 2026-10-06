#[test]
#[ignore = "requires pinned CC0 Leaf Aptus 22 MOS object 3196 and LibRaw sensor dump"]
fn mos_routes_full_sensor_and_rejects_bad_metadata_without_leaks() {
    let source = std::fs::read(std::env::var("RRRAH_LEAF_MOS_SOURCE").unwrap()).unwrap();
    let reference = std::fs::read(std::env::var("RRRAH_LEAF_MOS_REFERENCE").unwrap()).unwrap();
    assert_eq!(&source[8035..8039], b"3270");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.MOS");
    std::fs::write(&path, &source).unwrap();
    let budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&path);
    request.memory_budget = Some(budget.clone());
    let rrrah_decode::DecodedImage::Sensor(decoded) = rrrah_decode::decode_image(&request).unwrap() else {
        panic!("MOS selected preview instead of sensor");
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
    for case in 0..6 {
        let mut bytes = source.clone();
        match case {
            0 => bytes[8035..8039].copy_from_slice(b"0000"),
            1 => bytes[7418..7420].copy_from_slice(b"99"),
            2 => bytes[5016..5020].fill(255), // PKTS length escapes parent.
            3 => bytes[531582 + 9..531582 + 11].copy_from_slice(&4008u16.to_be_bytes()),
            4 => bytes.truncate(531582 + 22892051 - 1),
            5 => bytes[13008..13010].copy_from_slice(b"45"),
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
