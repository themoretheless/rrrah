//! Public-route regression against independent ravif/dav1d/rational fixture oracles.
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use rrrah_decode::{
    DecodeRequest, decode_raster, prepare_raster_for_display, prepare_raster_for_display_with_budget,
};
use std::sync::Arc;

#[test]
fn independently_encoded_prem_and_straight_avif_preserve_integer_and_prepared_pixels() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    for depth in [8, 10] {
        for mode in ["straight", "prem"] {
            let name = format!("ravif-{depth}bit-{mode}.avif");
            let budget = MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new(root.join(&name));
            request.memory_budget = Some(budget.clone());
            let decoded = decode_raster(&request).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!((decoded.width(), decoded.height()), (16, 16));
            assert_eq!(decoded.color_space(), &RasterColorSpace::Srgb);
            let expected_pixels = if depth == 8 {
                let bytes = std::fs::read(root.join(format!("{name}.rgba"))).unwrap();
                let RasterPixels::Rgba8(actual) = decoded.pixels() else {
                    panic!("8-bit precision");
                };
                assert_eq!(&actual[..], bytes.as_slice());
                RasterPixels::Rgba8(Arc::new(bytes).into())
            } else {
                let bytes = std::fs::read(root.join(format!("{name}.rgba16le"))).unwrap();
                assert_eq!(bytes.len() % 2, 0);
                let words: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|v| u16::from_le_bytes([v[0], v[1]]))
                    .collect();
                let RasterPixels::Rgba16(actual) = decoded.pixels() else {
                    panic!("10-bit precision");
                };
                assert_eq!(&actual[..], words.as_slice());
                RasterPixels::Rgba16(Arc::new(words).into())
            };
            let expected = DecodedRaster::new(16, 16, expected_pixels, RasterColorSpace::Srgb).unwrap();
            let expected = prepare_raster_for_display(&expected).unwrap();
            let prepared = prepare_raster_for_display_with_budget(&decoded, Some(&budget)).unwrap();
            let (RasterPixels::Rgba32Float(actual), RasterPixels::Rgba32Float(reference)) =
                (prepared.pixels(), expected.pixels())
            else {
                panic!("linear preparation");
            };
            assert_eq!(actual.len(), reference.len());
            assert!(
                actual
                    .iter()
                    .zip(reference.iter())
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
                "{name}"
            );
            drop(prepared);
            drop(decoded);
            assert_eq!(budget.used(), 0, "{name}");
        }
    }
}
