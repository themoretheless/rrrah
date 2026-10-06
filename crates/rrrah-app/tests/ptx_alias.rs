//! Extension compatibility only; a renamed PEF is not a camera-produced PTX oracle.
use rrrah_decode::RawDecoder;

#[test]
#[ignore = "requires pinned CC0 raw.pixls.us object 831 PEF"]
fn pef_sensor_and_recipe_survive_ptx_extension_alias() {
    let source = std::path::PathBuf::from(std::env::var("RRRAH_PEF_ALIAS_SOURCE").unwrap());
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&source);
    request.memory_budget = Some(budget.clone());
    let original = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
    let recipe = rrrah_decode::NativeRawDecoder.mosaic_recipe(&request).unwrap();
    let directory = tempfile::tempdir().unwrap();
    for extension in ["ptx", "PTX"] {
        let path = directory.path().join(format!("pentax.{extension}"));
        std::fs::copy(&source, &path).unwrap();
        request.path = path;
        assert!(rrrah_decode::is_supported_image_path(&request.path));
        assert_eq!(
            rrrah_decode::image_source_kind(&request).unwrap(),
            rrrah_decode::ImageSourceKind::Sensor
        );
        assert_eq!(
            rrrah_decode::NativeRawDecoder.mosaic_recipe(&request).unwrap(),
            recipe
        );
        let rrrah_decode::DecodedImage::Sensor(decoded) = rrrah_decode::decode_image(&request).unwrap()
        else {
            panic!("PTX must not select a raster preview");
        };
        assert_eq!(decoded.mosaic.metadata, original.mosaic.metadata);
        assert_eq!(decoded.mosaic.pixels, original.mosaic.pixels);
        assert!(decoded.mosaic.pixels.is_managed());
    }
    drop(original);
    assert_eq!(budget.used(), 0);
}
