//! PKM ETC1/ETC2 color textures, cropped from padded 4x4 blocks.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPkm(s)
}
fn validate_etc1(block: &[u8]) -> Result<(), RasterDecodeError> {
    crate::etc1::validate(block).map_err(bad)
}
use crate::etc1::decode as decode_etc1;
pub fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PKM 10") || bytes.starts_with(b"PKM 20")
}
pub fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    crate::check_single_image(request)?;
    let h = bytes.get(..16).ok_or_else(|| bad("truncated header"))?;
    if !has_magic(h) {
        return Err(bad("invalid signature"));
    }
    let word = |at| usize::from(u16::from_be_bytes([h[at], h[at + 1]]));
    let kind = word(6);
    let ew = word(8);
    let eh = word(10);
    let width = word(12);
    let height = word(14);
    if width == 0 || height == 0 || ew != width.div_ceil(4) * 4 || eh != height.div_ceil(4) * 4 {
        return Err(bad("invalid encoded or original dimensions"));
    }
    if &h[..6] == b"PKM 10" && kind != 0 {
        return Err(bad("PKM10 requires ETC1"));
    }
    let block_size = match kind {
        0 | 1 | 4 | 5 | 7 => 8,
        3 | 6 | 8 => 16,
        _ => return Err(bad("ETC/EAC storage type unsupported")),
    };
    let count = (ew / 4) * (eh / 4);
    let output_bytes = if (5..=8).contains(&kind) { 16 } else { 4 };
    if (width as u64) * (height as u64) * output_bytes > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if bytes.len() != 16 + count * block_size {
        return Err(bad("payload size mismatch"));
    }
    if (5..=8).contains(&kind) {
        return decode_eac(&bytes[16..], width, height, ew, kind, request);
    }
    if kind == 0 {
        for (i, block) in bytes[16..].chunks_exact(block_size).enumerate() {
            if i % 4096 == 0 {
                request.check_cancelled()?;
            }
            validate_etc1(block)?;
        }
    }
    let size = width * height * 4;
    let mut pixels: rrrah_core::PixelBuffer<u8> = match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(size, 0u8)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => {
            let mut pixels = Vec::new();
            pixels
                .try_reserve_exact(size)
                .map_err(|_| RasterDecodeError::OutputTooLarge)?;
            pixels.resize(size, 0);
            Arc::new(pixels).into()
        }
    };
    let output = pixels.get_mut().unwrap();
    for (i, block) in bytes[16..].chunks_exact(block_size).enumerate() {
        request.check_cancelled()?;
        let mut decoded = [0u32; 16];
        match kind {
            0 => {
                decode_etc1(block, &mut decoded);
            }
            1 => texture2ddecoder::decode_etc2_rgb_block(block, &mut decoded),
            3 => texture2ddecoder::decode_etc2_rgba8_block(block, &mut decoded),
            4 => texture2ddecoder::decode_etc2_rgba1_block(block, &mut decoded),
            _ => unreachable!(),
        }
        let (left, top) = ((i % (ew / 4)) * 4, (i / (ew / 4)) * 4);
        for y in 0..4 {
            for x in 0..4 {
                if left + x < width && top + y < height {
                    let [b, g, r, a] = decoded[y * 4 + x].to_le_bytes();
                    let at = ((top + y) * width + left + x) * 4;
                    output[at..at + 4].copy_from_slice(&[r, g, b, a]);
                }
            }
        }
    }
    Ok(DecodedRaster::new(
        width as u32,
        height as u32,
        RasterPixels::Rgba8(pixels),
        RasterColorSpace::AssumedSrgb,
    )?)
}
fn decode_eac(
    source: &[u8],
    width: usize,
    height: usize,
    encoded_width: usize,
    kind: usize,
    request: &DecodeRequest,
) -> Result<DecodedRaster, RasterDecodeError> {
    let signed = kind >= 7;
    let dual = matches!(kind, 6 | 8);
    let denominator = if signed { 1023f32 } else { 2047f32 };
    let count = width * height * 4;
    let mut pixels: rrrah_core::PixelBuffer<f32> = match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(count, 0f32)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => {
            let mut values = Vec::new();
            values
                .try_reserve_exact(count)
                .map_err(|_| RasterDecodeError::OutputTooLarge)?;
            values.resize(count, 0f32);
            Arc::new(values).into()
        }
    };
    let output = pixels.get_mut().unwrap();
    for (i, block) in source.chunks_exact(if dual { 16 } else { 8 }).enumerate() {
        request.check_cancelled()?;
        let red = crate::eac::decode(block[..8].try_into().unwrap(), signed);
        let green = if dual {
            crate::eac::decode(block[8..].try_into().unwrap(), signed)
        } else {
            [0; 16]
        };
        let left = i % (encoded_width / 4) * 4;
        let top = i / (encoded_width / 4) * 4;
        for y in 0..4 {
            for x in 0..4 {
                if left + x < width && top + y < height {
                    let index = y * 4 + x;
                    let at = ((top + y) * width + left + x) * 4;
                    output[at..at + 4].copy_from_slice(&[
                        f32::from(red[index]) / denominator,
                        f32::from(green[index]) / denominator,
                        0.,
                        1.,
                    ]);
                }
            }
        }
    }
    request.check_cancelled()?;
    Ok(DecodedRaster::new(
        width as u32,
        height as u32,
        RasterPixels::Rgba32Float(pixels),
        request.qualify_linear_color(RasterColorSpace::LinearRgbUnspecified),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "manual EAC cancellation after observed16MiB output admission"]
    fn eac_cancel_after_admission_preserves_other_owner_and_exact_retry() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for kind in 5..=8 {
            let original = std::fs::read(root.join(format!("pkm-eac-{kind}-4x4.pkm"))).unwrap();
            let expected = std::fs::read(root.join(format!("pkm-eac-{kind}-4x4.pkm.rgba32f"))).unwrap();
            let mut source = original[..16].to_vec();
            for offset in [8, 10, 12, 14] {
                source[offset..offset + 2].copy_from_slice(&1024u16.to_be_bytes());
            }
            for _ in 0..256 * 256 {
                source.extend_from_slice(&original[16..]);
            }
            let output_bytes = 1024 * 1024 * 16u64;
            let budget = rrrah_core::MemoryBudget::new(output_bytes + 4096);
            let other = budget.try_reserve(4096).unwrap();
            let generation = Arc::new(AtomicU64::new(1));
            let mut request = DecodeRequest::new("large-signed-rg.pkm");
            request.memory_budget = Some(budget.clone());
            request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 1));
            let (ready, waiting) = std::sync::mpsc::channel();
            let watched = budget.clone();
            let invalidate = generation.clone();
            let observer = std::thread::spawn(move || {
                ready.send(()).unwrap();
                let start = std::time::Instant::now();
                while watched.used() < output_bytes + 4096 {
                    assert!(
                        start.elapsed() < std::time::Duration::from_secs(3),
                        "output admission not observed"
                    );
                    std::thread::yield_now();
                }
                invalidate.store(2, Ordering::Release);
            });
            waiting.recv().unwrap();
            assert!(matches!(
                decode(&source, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            observer.join().unwrap();
            assert_eq!(budget.used(), 4096);
            assert_eq!(budget.peak(), output_bytes + 4096);
            request.cancellation = None;
            let retry = decode(&source, &request).unwrap();
            let RasterPixels::Rgba32Float(values) = retry.pixels() else {
                panic!()
            };
            for y in 0..1024 {
                for x in 0..1024 {
                    for channel in 0..4 {
                        let at = ((y % 4) * 4 + x % 4) * 16 + channel * 4;
                        let reference = u32::from_le_bytes(expected[at..at + 4].try_into().unwrap());
                        assert_eq!(values[(y * 1024 + x) * 4 + channel].to_bits(), reference);
                    }
                }
            }
            drop(retry);
            assert_eq!(budget.used(), 4096);
            drop(other);
            assert_eq!(budget.used(), 0);
            eprintln!(
                "EAC_CANCEL kind={kind} observed_output_bytes={output_bytes} independent_owner=4096 retry_pixels=1048576 exact=true final=0"
            );
        }
    }
    #[test]
    fn full_eac_parameter_corpus_preserves_normalized_samples_and_channel_layout() {
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
        let unsigned = inflate(include_bytes!(
            "../../../tests/fixtures/raster/eac-r11-full-oracles.u11le.zlib"
        ));
        let signed = inflate(include_bytes!(
            "../../../tests/fixtures/raster/eac-r11-full-oracles.i11le.zlib"
        ));
        assert_eq!(blocks.len(), 65536 * 8);
        for kind in 5..=8u16 {
            let dual = matches!(kind, 6 | 8);
            let denominator = if kind >= 7 { 1023f64 } else { 2047f64 };
            let oracle = if kind >= 7 { &signed } else { &unsigned };
            let mut source = b"PKM 20".to_vec();
            for word in [kind, 1024, 1024, 1024, 1024] {
                source.extend(word.to_be_bytes());
            }
            for (i, block) in blocks.as_chunks::<8>().0.iter().enumerate() {
                source.extend(block);
                if dual {
                    let next = (i + 1) % 65536;
                    source.extend_from_slice(&blocks[next * 8..next * 8 + 8]);
                }
            }
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024 * 16);
            let mut request = DecodeRequest::new("full-parameter.pkm");
            request.memory_budget = Some(budget.clone());
            let image = decode(&source, &request).unwrap();
            let RasterPixels::Rgba32Float(values) = image.pixels() else {
                panic!()
            };
            for block in 0..65536 {
                let left = block % 256 * 4;
                let top = block / 256 * 4;
                for pixel in 0..16 {
                    let at = ((top + pixel / 4) * 1024 + left + pixel % 4) * 4;
                    for channel in 0..if dual { 2 } else { 1 } {
                        let reference_block = (block + channel) % 65536;
                        let offset = reference_block * 32 + pixel * 2;
                        let code = i16::from_le_bytes(oracle[offset..offset + 2].try_into().unwrap());
                        let expected = (f64::from(code) / denominator) as f32;
                        assert_eq!(
                            values[at + channel].to_bits(),
                            expected.to_bits(),
                            "kind={kind} block={block} pixel={pixel} channel={channel}"
                        );
                    }
                    if !dual {
                        assert_eq!(values[at + 1].to_bits(), 0);
                    }
                    assert_eq!(values[at + 2].to_bits(), 0);
                    assert_eq!(values[at + 3].to_bits(), 1f32.to_bits());
                }
            }
            assert_eq!((budget.used(), budget.peak()), (16777216, 16777216));
            drop(image);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn eac_native_precision_managed_quotas_and_explicit_linear_policy() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut cases = 0;
        for line in include_str!("../../../tests/fixtures/raster/pkm-eac-manifest.tsv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = line.split('\t').collect();
            let source = std::fs::read(root.join(fields[0])).unwrap();
            let expected = std::fs::read(root.join(fields[4])).unwrap();
            let size = expected.len() as u64;
            let budget = rrrah_core::MemoryBudget::new(size);
            let mut request = DecodeRequest::new(fields[0]);
            request.memory_budget = Some(budget.clone());
            for qualify in [false, true] {
                request.assume_untagged_linear_srgb = qualify;
                let image = decode(&source, &request).unwrap();
                assert_eq!(
                    image.color_space(),
                    &if qualify {
                        RasterColorSpace::LinearSrgb
                    } else {
                        RasterColorSpace::LinearRgbUnspecified
                    }
                );
                let RasterPixels::Rgba32Float(values) = image.pixels() else {
                    panic!()
                };
                let actual: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
                assert_eq!(actual, expected, "{}", fields[0]);
                assert_eq!((budget.used(), budget.peak()), (size, size));
                let held = image.clone();
                drop(image);
                assert_eq!(budget.used(), size);
                drop(held);
                assert_eq!(budget.used(), 0);
            }
            let short = rrrah_core::MemoryBudget::new(size - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(&source, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!((short.used(), short.peak()), (0, 0));
            let zero = rrrah_core::MemoryBudget::new(0);
            request.memory_budget = Some(zero.clone());
            assert!(matches!(
                decode(&source[..source.len() - 1], &request),
                Err(RasterDecodeError::InvalidPkm("payload size mismatch"))
            ));
            let mut old = source.clone();
            old[4..6].copy_from_slice(b"10");
            assert!(matches!(
                decode(&old, &request),
                Err(RasterDecodeError::InvalidPkm("PKM10 requires ETC1"))
            ));
            assert_eq!((zero.used(), zero.peak()), (0, 0));
            cases += 1;
        }
        assert_eq!(cases, 16);
    }
    #[test]
    fn owned_etc1_matches_independent_android_blocks() {
        let blocks = include_bytes!("../../../tests/fixtures/raster/etc1-block-oracles.bin");
        let oracle = include_bytes!("../../../tests/fixtures/raster/etc1-block-oracles.rgb");
        assert_eq!(blocks.len(), 4096 * 8);
        assert_eq!(oracle.len(), 4096 * 16 * 3);
        for (block, expected) in blocks.chunks_exact(8).zip(oracle.chunks_exact(48)) {
            validate_etc1(block).unwrap();
            let mut output = [0u32; 16];
            decode_etc1(block, &mut output);
            for (actual, expected) in output.iter().zip(expected.chunks_exact(3)) {
                let [b, g, r, a] = actual.to_le_bytes();
                assert_eq!([r, g, b], expected);
                assert_eq!(a, 255);
            }
        }
    }
    #[test]
    fn managed_oracle_pixels_exact_short_and_shared_ownership() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("manifest.tsv")).unwrap();
        let mut cases = 0;
        for line in manifest.lines() {
            let fields: Vec<_> = line.split('\t').collect();
            if !fields[0].ends_with(".pkm") {
                continue;
            }
            let source = std::fs::read(root.join(fields[0])).unwrap();
            let oracle = std::fs::read(root.join(fields[4])).unwrap();
            let size = oracle.len() as u64;
            let budget = rrrah_core::MemoryBudget::new(size);
            let mut request = DecodeRequest::new(fields[0]);
            request.memory_budget = Some(budget.clone());
            let frame = decode(&source, &request).unwrap();
            let RasterPixels::Rgba8(pixels) = frame.pixels() else {
                panic!()
            };
            assert_eq!(&pixels[..], oracle, "{}", fields[0]);
            assert_eq!((budget.used(), budget.peak()), (size, size));
            let shared = frame.clone();
            drop(frame);
            assert_eq!(budget.used(), size);
            drop(shared);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(size - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(&source, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 6);
    }

    #[test]
    fn malformed_etc1_precedes_zero_budget_admission() {
        let source = include_bytes!("../../../tests/fixtures/raster/individual.pkm");
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("bad.pkm");
        request.memory_budget = Some(budget.clone());
        for channel in 0..3 {
            for value in [4, 249] {
                let mut bytes = source.to_vec();
                bytes[16 + channel] = value;
                bytes[19] |= 2;
                assert!(matches!(
                    decode(&bytes, &request),
                    Err(RasterDecodeError::InvalidPkm(_))
                ));
                assert_eq!((budget.used(), budget.peak()), (0, 0));
            }
        }
        assert!(matches!(
            decode(source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
    }

    #[test]
    fn rejects_invalid_dimensions_and_payload() {
        for b in [
            b"PKM 10\0\0\0\x04\0\x04\0\0\0\x03".as_slice(),
            b"PKM 10\0\0\0\x04\0\x04\0\x03\0\x03".as_slice(),
        ] {
            assert!(decode(b, &DecodeRequest::new("x.pkm")).is_err());
        }
    }
    #[test]
    fn invalid_etc1_differential_and_excessive_output_are_rejected() {
        let mut bytes = include_bytes!("../../../tests/fixtures/raster/individual.pkm").to_vec();
        bytes[16] = 249;
        bytes[19] |= 2;
        assert!(decode(&bytes, &DecodeRequest::new("x.pkm")).is_err());
        let mut bytes = b"PKM 10".to_vec();
        for value in [0u16, 65532, 65532, 65532, 65532] {
            bytes.extend(value.to_be_bytes());
        }
        assert!(matches!(
            decode(&bytes, &DecodeRequest::new("x.pkm")),
            Err(RasterDecodeError::OutputTooLarge)
        ));
    }
    #[test]
    fn constant_alpha_reaches_linear_display() {
        let frame = decode(
            include_bytes!("../../../tests/fixtures/raster/etc2-alpha-constant.pkm"),
            &DecodeRequest::new("x.pkm"),
        )
        .unwrap();
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!()
        };
        assert_eq!(p[3], 17);
    }
}
