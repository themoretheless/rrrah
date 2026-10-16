//! Owned ETC1 block validation and color reconstruction.
pub(crate) fn validate(block: &[u8]) -> Result<(), &'static str> {
    if block[3] & 2 != 0 {
        for value in &block[..3] {
            let base = i16::from(value >> 3);
            let delta = i16::from(value & 7);
            let delta = if delta >= 4 { delta - 8 } else { delta };
            if !(0..32).contains(&(base + delta)) {
                return Err("invalid ETC1 differential color");
            }
        }
    }
    Ok(())
}
// Owned ETC1 color reconstruction. Differential endpoints are validated before
// output admission by validate_etc1; selectors are column-major in the wire block.
pub(crate) fn decode(block: &[u8], output: &mut [u32; 16]) {
    const MODIFIERS: [[i16; 4]; 8] = [
        [2, 8, -2, -8],
        [5, 17, -5, -17],
        [9, 29, -9, -29],
        [13, 42, -13, -42],
        [18, 60, -18, -60],
        [24, 80, -24, -80],
        [33, 106, -33, -106],
        [47, 183, -47, -183],
    ];
    let mut endpoints = [[0i16; 3]; 2];
    for channel in 0..3 {
        let byte = block[channel];
        if block[3] & 2 == 0 {
            endpoints[0][channel] = i16::from(byte >> 4) * 17;
            endpoints[1][channel] = i16::from(byte & 15) * 17;
        } else {
            let first = i16::from(byte >> 3);
            let delta = i16::from(byte & 7);
            let second = first + if delta >= 4 { delta - 8 } else { delta };
            endpoints[0][channel] = (first << 3) | (first >> 2);
            endpoints[1][channel] = (second << 3) | (second >> 2);
        }
    }
    let tables = [usize::from(block[3] >> 5), usize::from((block[3] >> 2) & 7)];
    let selectors = u32::from_be_bytes(block[4..8].try_into().unwrap());
    for y in 0..4 {
        for x in 0..4 {
            let part = usize::from(if block[3] & 1 == 0 { x >= 2 } else { y >= 2 });
            let bit = x * 4 + y;
            let selector = ((selectors >> bit) & 1) | (((selectors >> (bit + 16)) & 1) << 1);
            let modifier = MODIFIERS[tables[part]][selector as usize];
            let rgb = endpoints[part].map(|base| (base + modifier).clamp(0, 255) as u8);
            output[y * 4 + x] = u32::from_le_bytes([rgb[2], rgb[1], rgb[0], 255]);
        }
    }
}

#[cfg(test)]
mod profiles {
    #[test]
    #[ignore = "manual alternating paired ETC1 block decoder profile"]
    fn owned_and_previous_decoder_paired_profile() {
        use std::{hint::black_box, time::Instant};
        let blocks = include_bytes!("../../../tests/fixtures/raster/etc1-block-oracles.bin");
        fn previous(block: &[u8], output: &mut [u32; 16]) {
            texture2ddecoder::decode_etc1_block(block, output);
        }
        let run = |decoder: fn(&[u8], &mut [u32; 16])| {
            let decoder = black_box(decoder);
            let mut output = [0u32; 16];
            let start = Instant::now();
            for _ in 0..128 {
                for block in blocks.as_chunks::<8>().0 {
                    decoder(black_box(block), black_box(&mut output));
                    black_box(&output);
                }
            }
            start.elapsed().as_nanos()
        };
        run(previous);
        run(super::decode);
        for pair in 0..12 {
            let (previous_ns, owned_ns) = if pair % 2 == 0 {
                (run(previous), run(super::decode))
            } else {
                let owned = run(super::decode);
                (run(previous), owned)
            };
            eprintln!("ETC1_AB pair={pair} blocks=524288 previous_ns={previous_ns} owned_ns={owned_ns}");
        }
    }
}
