#[test]
#[ignore = "requires pinned DSC-F828 SRF and independent full sensor oracle"]
fn srf_public_sensor_route_and_memory_refusals() {
    let source = std::fs::read(std::env::var("RRRAH_SRF_SOURCE").unwrap()).unwrap();
    let reference = std::fs::read(std::env::var("RRRAH_SRF_ORACLE").unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.SRF");
    std::fs::write(&path, &source).unwrap();
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&path);
    request.memory_budget = Some(budget.clone());
    let rrrah_decode::DecodedImage::Sensor(decoded) = rrrah_decode::decode_image(&request).unwrap() else {
        panic!("SRF selected a preview");
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
    assert_eq!(
        decoded.mosaic.metadata.cfa.as_ref().unwrap().rgbe_quad().unwrap(),
        [3, 0, 2, 1]
    );
    assert_eq!(
        decoded.mosaic.metadata.white_balance,
        [397. / 256., 1., 728. / 256., 1.]
    );
    let preview = decoded.mosaic.thumbnail_rgba8(128);
    assert_eq!(preview.len(), 128 * 96 * 4);
    assert!(preview.chunks_exact(4).all(|p| p[3] == 255));
    assert!(preview.chunks_exact(4).any(|p| p[0] != p[1] || p[1] != p[2]));
    let held = decoded.mosaic.pixels.clone();
    drop(decoded);
    assert_eq!(budget.used(), 16531200);
    drop(held);
    assert_eq!(budget.used(), 0);
    for case in 0..5 {
        let mut malformed = source.clone();
        match case {
            0 => malformed[274] = b'X',
            1 => {
                malformed.pop();
            }
            2 => {
                for (i, b) in 256u16.to_le_bytes().into_iter().enumerate() {
                    malformed[0x28732 + i] ^= b;
                }
            }
            3 => malformed[0x28732 - 8] ^= 1,
            4 => malformed[0x28726 - 6] ^= 3 ^ 4,
            _ => unreachable!(),
        }
        std::fs::write(&path, malformed).unwrap();
        assert!(rrrah_decode::decode_image(&request).is_err(), "case {case}");
        assert_eq!(budget.used(), 0);
    }
    std::fs::write(&path, &source).unwrap();
    let small = rrrah_core::MemoryBudget::new(32 * 1024 * 1024);
    request.memory_budget = Some(small.clone());
    assert!(rrrah_decode::decode_image(&request).is_err());
    assert_eq!(small.used(), 0);
}
