//! Owned EAC R11 signed/unsigned integer reconstruction (Khronos Data Format22.5/22.7).
pub(crate) fn decode(block: &[u8; 8], signed: bool) -> [i16; 16] {
    const MODIFIERS: [[i16; 8]; 16] = [
        [-3, -6, -9, -15, 2, 5, 8, 14],
        [-3, -7, -10, -13, 2, 6, 9, 12],
        [-2, -5, -8, -13, 1, 4, 7, 12],
        [-2, -4, -6, -13, 1, 3, 5, 12],
        [-3, -6, -8, -12, 2, 5, 7, 11],
        [-3, -7, -9, -11, 2, 6, 8, 10],
        [-4, -7, -8, -11, 3, 6, 7, 10],
        [-3, -5, -8, -11, 2, 4, 7, 10],
        [-2, -6, -8, -10, 1, 5, 7, 9],
        [-2, -5, -8, -10, 1, 4, 7, 9],
        [-2, -4, -8, -10, 1, 3, 7, 9],
        [-2, -5, -7, -10, 1, 4, 6, 9],
        [-3, -4, -7, -10, 2, 3, 6, 9],
        [-1, -2, -3, -10, 0, 1, 2, 9],
        [-4, -6, -8, -9, 3, 5, 7, 8],
        [-3, -5, -7, -9, 2, 4, 6, 8],
    ];
    let base = if signed {
        i16::from(block[0] as i8).max(-127) * 8
    } else {
        i16::from(block[0]) * 8 + 4
    };
    let multiplier = (i16::from(block[1] >> 4) * 8).max(1);
    let table = MODIFIERS[usize::from(block[1] & 15)];
    let bits = u64::from_be_bytes(*block);
    let mut output = [0i16; 16];
    for y in 0..4 {
        for x in 0..4 {
            let index = ((bits >> (45 - 3 * (x * 4 + y))) & 7) as usize;
            output[y * 4 + x] = (base + table[index] * multiplier)
                .clamp(if signed { -1023 } else { 0 }, if signed { 1023 } else { 2047 });
        }
    }
    output
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_base_table_multiplier_and_selector_matches_independent_reference() {
        use std::io::Read;
        fn inflate(bytes: &[u8]) -> Vec<u8> {
            let mut output = Vec::new();
            flate2::read::ZlibDecoder::new(bytes)
                .read_to_end(&mut output)
                .unwrap();
            output
        }
        let blocks = inflate(include_bytes!(
            "../../../tests/fixtures/raster/eac-r11-full-oracles.bin.zlib"
        ));
        assert_eq!(blocks.len(), 65536 * 8);
        let mut seen = vec![false; 65536];
        for block in blocks.as_chunks::<8>().0 {
            let table = usize::from(block[1] & 15);
            let multiplier = usize::from(block[1] >> 4);
            let key = usize::from(block[0]) * 256 + table * 16 + multiplier;
            assert!(!seen[key]);
            seen[key] = true;
            let bits = u64::from_be_bytes(*block);
            let mut selectors = 0u8;
            for pixel in 0..16 {
                selectors |= 1 << ((bits >> (pixel * 3)) & 7);
            }
            assert_eq!(selectors, 255);
        }
        assert!(seen.iter().all(|value| *value));
        for (signed, compressed) in [
            (
                false,
                include_bytes!("../../../tests/fixtures/raster/eac-r11-full-oracles.u11le.zlib").as_slice(),
            ),
            (
                true,
                include_bytes!("../../../tests/fixtures/raster/eac-r11-full-oracles.i11le.zlib").as_slice(),
            ),
        ] {
            let oracle = inflate(compressed);
            assert_eq!(oracle.len(), 65536 * 32);
            for (block, expected) in blocks.as_chunks::<8>().0.iter().zip(oracle.as_chunks::<32>().0) {
                let actual = super::decode(block, signed);
                for (value, bytes) in actual.iter().zip(expected.as_chunks::<2>().0) {
                    assert_eq!(
                        *value,
                        i16::from_le_bytes(*bytes),
                        "signed={signed} block={block:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn signed_and_unsigned_match_all_independent_ericsson_samples() {
        let blocks = include_bytes!("../../../tests/fixtures/raster/eac-r11-block-oracles.bin");
        for (signed, oracle) in [
            (
                false,
                include_bytes!("../../../tests/fixtures/raster/eac-r11-block-oracles.u11le").as_slice(),
            ),
            (
                true,
                include_bytes!("../../../tests/fixtures/raster/eac-r11-block-oracles.i11le").as_slice(),
            ),
        ] {
            assert_eq!(blocks.len(), 4096 * 8);
            assert_eq!(oracle.len(), 4096 * 32);
            for (block, expected) in blocks.as_chunks::<8>().0.iter().zip(oracle.as_chunks::<32>().0) {
                let actual = super::decode(block, signed);
                for (value, bytes) in actual.iter().zip(expected.as_chunks::<2>().0) {
                    assert_eq!(*value, i16::from_le_bytes(*bytes));
                }
            }
        }
    }
}
