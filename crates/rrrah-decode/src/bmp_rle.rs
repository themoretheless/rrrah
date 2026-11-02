//! One-pass Windows RLE4/RLE8 to admitted RGBA; no per-command heap buffers.
use crate::{DecodeRequest, raster::RasterDecodeError};
use image::{ColorType, ImageDecoder, ImageError, ImageFormat, ImageResult};
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidBmpRle(reason)
}
fn malformed(reason: &'static str) -> ImageError {
    ImageError::Decoding(image::error::DecodingError::new(ImageFormat::Bmp.into(), reason))
}
pub(super) struct Decoder<'a> {
    bytes: &'a [u8],
    palette: [[u8; 4]; 256],
    colors: usize,
    request: &'a DecodeRequest,
    at: usize,
    end: usize,
    width: u32,
    height: u32,
    bits: u16,
}
impl<'a> Decoder<'a> {
    pub fn new(bytes: &'a [u8], request: &'a DecodeRequest) -> Result<Option<Self>, RasterDecodeError> {
        request.check_cancelled()?;
        let u32_at = |at: usize| -> Result<u32, RasterDecodeError> {
            Ok(u32::from_le_bytes(
                bytes
                    .get(at..at + 4)
                    .ok_or_else(|| bad("truncated header"))?
                    .try_into()
                    .unwrap(),
            ))
        };
        let header = u32_at(14)? as usize;
        if !matches!(header, 40 | 52 | 56 | 108 | 124) {
            return Ok(None);
        }
        let compression = u32_at(30)?;
        if !matches!(compression, 1 | 2) {
            return Ok(None);
        }
        let width = u32_at(18)? as i32;
        let height = u32_at(22)? as i32;
        if width <= 0 || height <= 0 {
            return Err(bad("RLE requires positive bottom-up dimensions"));
        }
        let bits = u16::from_le_bytes(
            bytes
                .get(28..30)
                .ok_or_else(|| bad("truncated bit depth"))?
                .try_into()
                .unwrap(),
        );
        if (compression == 1 && bits != 8) || (compression == 2 && bits != 4) {
            return Err(bad("compression and bit depth disagree"));
        }
        if bytes.get(26..28) != Some(&1u16.to_le_bytes()) {
            return Err(bad("invalid plane count"));
        }
        let used = u32_at(46)?;
        let colors = if used == 0 { 1usize << bits } else { used as usize };
        if colors == 0 || colors > 1usize << bits {
            return Err(bad("palette count exceeds bit depth"));
        }
        let palette_start = 14 + header;
        let palette_end = palette_start + colors * 4;
        let at = u32_at(10)? as usize;
        if at < palette_end {
            return Err(bad("palette overlaps pixel stream"));
        }
        let raw_palette = bytes
            .get(palette_start..palette_end)
            .ok_or_else(|| bad("truncated palette"))?;
        let mut palette = [[0u8; 4]; 256];
        for (pixel, bgr) in palette.iter_mut().zip(raw_palette.chunks_exact(4)) {
            *pixel = [bgr[2], bgr[1], bgr[0], 255];
        }
        let size = u32_at(34)? as usize;
        let end = if size == 0 {
            bytes.len()
        } else {
            at.checked_add(size)
                .ok_or_else(|| bad("stream extent overflow"))?
        };
        if at > end || end > bytes.len() {
            return Err(bad("compressed stream outside input"));
        }
        Ok(Some(Self {
            bytes,
            palette,
            colors,
            request,
            at,
            end,
            width: width as u32,
            height: height as u32,
            bits,
        }))
    }
    fn take(&mut self, n: usize) -> ImageResult<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&end| end <= self.end)
            .ok_or_else(|| malformed("truncated BMP RLE command or missing end marker"))?;
        let data = &self.bytes[self.at..end];
        self.at = end;
        Ok(data)
    }
    fn check(&self) -> ImageResult<()> {
        self.request
            .check_cancelled()
            .map_err(|_| malformed("BMP RLE cancelled"))
    }
    fn color(&self, index: u8) -> ImageResult<[u8; 4]> {
        if usize::from(index) >= self.colors {
            return Err(malformed("BMP RLE index outside declared palette"));
        }
        Ok(self.palette[usize::from(index)])
    }
    fn span<'b>(&self, out: &'b mut [u8], x: usize, y: usize, count: usize) -> ImageResult<&'b mut [u8]> {
        if y >= self.height as usize || x > self.width as usize || count > self.width as usize - x {
            return Err(malformed("BMP RLE run crosses image boundary"));
        }
        let start = ((self.height as usize - y - 1) * self.width as usize + x) * 4;
        Ok(&mut out[start..start + count * 4])
    }
    fn absolute(&self, out: &mut [u8], x: usize, y: usize, count: usize, data: &[u8]) -> ImageResult<()> {
        let pixels = self.span(out, x, y, count)?.as_chunks_mut::<4>().0;
        if self.bits == 8 {
            let indices = &data[..count];
            if self.colors.is_power_of_two() {
                let mask = !(self.colors - 1) as u8;
                if mask != 0 {
                    let (words, tail) = indices.as_chunks::<8>();
                    let combined = words
                        .iter()
                        .fold(0u64, |bits, &word| bits | u64::from_ne_bytes(word));
                    if combined & u64::from_ne_bytes([mask; 8]) != 0 || tail.iter().any(|&v| v & mask != 0) {
                        return Err(malformed("BMP RLE index outside declared palette"));
                    }
                }
                for (pixel, &index) in pixels.iter_mut().zip(indices) {
                    *pixel = self.palette[usize::from(index)];
                }
            } else {
                for (pixel, &index) in pixels.iter_mut().zip(indices) {
                    if usize::from(index) >= self.colors {
                        return Err(malformed("BMP RLE index outside declared palette"));
                    }
                    *pixel = self.palette[usize::from(index)];
                }
            }
        } else {
            if self.colors < 16 {
                let mut maximum = data[..count / 2]
                    .iter()
                    .map(|&p| (p >> 4).max(p & 15))
                    .max()
                    .unwrap_or(0);
                if count % 2 != 0 {
                    maximum = maximum.max(data[count / 2] >> 4);
                }
                if usize::from(maximum) >= self.colors {
                    return Err(malformed("BMP RLE index outside declared palette"));
                }
            }
            let (pairs, tail) = pixels.as_chunks_mut::<2>();
            for (pair, &packed) in pairs.iter_mut().zip(data) {
                pair[0] = self.palette[usize::from(packed >> 4)];
                pair[1] = self.palette[usize::from(packed & 15)];
            }
            if !tail.is_empty() {
                tail[0] = self.palette[usize::from(data[count / 2] >> 4)];
            }
        }
        Ok(())
    }
    fn black(&self, out: &mut [u8]) -> ImageResult<()> {
        for block in out.chunks_mut(16384) {
            self.check()?;
            block.as_chunks_mut::<4>().0.fill([0, 0, 0, 255]);
        }
        Ok(())
    }
    fn encoded(&self, out: &mut [u8], x: usize, y: usize, count: usize, index: u8) -> ImageResult<()> {
        let first = self.color(if self.bits == 8 { index } else { index >> 4 })?;
        let row = self.span(out, x, y, count)?;
        if self.bits == 8 {
            row.as_chunks_mut::<4>().0.fill(first);
        } else {
            let second = if count > 1 { self.color(index & 15)? } else { first };
            let pair = [
                first[0], first[1], first[2], first[3], second[0], second[1], second[2], second[3],
            ];
            let (pairs, tail) = row.as_chunks_mut::<8>();
            pairs.fill(pair);
            if !tail.is_empty() {
                tail.copy_from_slice(&first);
            }
        }
        Ok(())
    }
}
impl ImageDecoder for Decoder<'_> {
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    fn color_type(&self) -> ColorType {
        ColorType::Rgba8
    }
    fn read_image(mut self, out: &mut [u8]) -> ImageResult<()> {
        if out.len() as u64 != u64::from(self.width) * u64::from(self.height) * 4 {
            return Err(malformed("wrong BMP RLE output buffer length"));
        }
        let (mut x, mut y) = (0usize, 0usize);
        loop {
            self.check()?;
            let pair = self.take(2)?;
            let (count, op) = (pair[0] as usize, pair[1]);
            if count != 0 {
                self.encoded(out, x, y, count, op)?;
                x += count;
                continue;
            }
            match op {
                0 => {
                    if y >= self.height as usize {
                        return Err(malformed("BMP RLE row outside image"));
                    }
                    self.black(self.span(out, x, y, self.width as usize - x)?)?;
                    x = 0;
                    y += 1;
                }
                1 => {
                    if y < self.height as usize {
                        self.black(self.span(out, x, y, self.width as usize - x)?)?;
                        self.black(&mut out[..(self.height as usize - y - 1) * self.width as usize * 4])?;
                    }
                    return Ok(());
                }
                2 => {
                    let delta = self.take(2)?;
                    let next_x = x + usize::from(delta[0]);
                    let next_y = y + usize::from(delta[1]);
                    if next_x > self.width as usize || next_y >= self.height as usize {
                        return Err(malformed("BMP RLE delta outside image"));
                    }
                    // Only skipped positions need initialization. Fully painted
                    // rows avoid a second whole-output pass; every successful
                    // image still defines all channels, including opaque holes.
                    if next_y == y {
                        self.black(self.span(out, x, y, next_x - x)?)?;
                    } else {
                        self.black(self.span(out, x, y, self.width as usize - x)?)?;
                        let start = (self.height as usize - next_y) * self.width as usize * 4;
                        let end = (self.height as usize - y - 1) * self.width as usize * 4;
                        self.black(&mut out[start..end])?;
                        self.black(self.span(out, 0, next_y, next_x)?)?;
                    }
                    x = next_x;
                    y = next_y;
                }
                n => {
                    let count = usize::from(n);
                    let packed = if self.bits == 8 { count } else { count.div_ceil(2) };
                    let data = self.take((packed + 1) & !1)?;
                    self.absolute(out, x, y, count, data)?;
                    x += count;
                }
            }
        }
    }
    fn read_image_boxed(self: Box<Self>, out: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rrrah_core::{MemoryBudget, RasterPixels};
    fn fixtures() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
    }
    fn bitmap(bits: u16, width: i32, height: i32, body: &[u8]) -> Vec<u8> {
        let mut bytes = std::fs::read(fixtures().join(format!("bmp-rle{bits}-mixed.bmp"))).unwrap();
        bytes.truncate(86);
        bytes[18..22].copy_from_slice(&width.to_le_bytes());
        bytes[22..26].copy_from_slice(&height.to_le_bytes());
        bytes[34..38].copy_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(body);
        let len = bytes.len() as u32;
        bytes[2..6].copy_from_slice(&len.to_le_bytes());
        bytes
    }
    #[test]
    fn independent_rle_samples_and_exact_public_file_admission() {
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixtures().join("bmp-rle-reference.json")).unwrap())
                .unwrap();
        for case in manifest["cases"].as_array().unwrap() {
            let path = fixtures().join(case["path"].as_str().unwrap());
            let hex = case["rgba8_hex"].as_str().unwrap();
            let expected: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
                .collect();
            let source = std::fs::metadata(&path).unwrap().len();
            let root = MemoryBudget::new(source + expected.len() as u64);
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(root.clone());
            let raster = crate::decode_raster(&request).unwrap();
            let RasterPixels::Rgba8(pixels) = raster.pixels() else {
                panic!()
            };
            assert_eq!(&pixels[..], &expected, "{}", path.display());
            assert_eq!(root.used(), expected.len() as u64);
            assert_eq!(root.peak(), source + expected.len() as u64);
            drop(raster);
            assert_eq!(root.used(), 0);
            let root = MemoryBudget::new(source + expected.len() as u64 - 1);
            request.memory_budget = Some(root.clone());
            assert!(crate::decode_raster(&request).is_err());
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn every_truncated_prefix_rejects_without_retained_output() {
        for bits in [4, 8] {
            let bytes = bitmap(bits, 3, 1, &[3, if bits == 8 { 1 } else { 0x12 }, 0, 0, 0, 1]);
            for length in 0..bytes.len() {
                let root = MemoryBudget::new(12);
                let mut request = DecodeRequest::new("truncated.bmp");
                request.memory_budget = Some(root.clone());
                assert!(
                    crate::raster::decode_raster_bytes(bytes[..length].to_vec(), &request).is_err(),
                    "bits={bits} prefix={length}"
                );
                assert_eq!(root.used(), 0);
            }
        }
    }
    #[test]
    fn invalid_runs_deltas_palette_indices_and_missing_end_markers_reject() {
        for bits in [4, 8] {
            for body in [
                vec![4, 1, 0, 1],
                vec![3, 1, 0, 0],
                vec![0, 2, 4, 0, 0, 1],
                vec![0, 2, 0, 1, 0, 1],
                vec![2, if bits == 8 { 8 } else { 0x18 }, 0, 1],
            ] {
                let root = MemoryBudget::new(12);
                let mut request = DecodeRequest::new("invalid.bmp");
                request.memory_budget = Some(root.clone());
                assert!(
                    crate::raster::decode_raster_bytes(bitmap(bits, 3, 1, &body), &request).is_err(),
                    "bits={bits} body={body:?}"
                );
                assert_eq!(root.used(), 0);
            }
            let mut bytes = bitmap(bits, 3, 1, &[3, 1, 0, 1]);
            bytes[34..38].copy_from_slice(&2u32.to_le_bytes()); // EOB outside declared stream.
            assert!(
                crate::raster::decode_raster_bytes(bytes, &DecodeRequest::new("short-size.bmp")).is_err()
            );
            assert!(
                crate::raster::decode_raster_bytes(
                    bitmap(bits, 3, -1, &[0, 1]),
                    &DecodeRequest::new("top-down.bmp")
                )
                .is_err()
            );
        }
    }
    #[test]
    fn delta_and_early_end_define_every_byte_of_a_nonzero_caller_buffer() {
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixtures().join("bmp-rle-reference.json")).unwrap())
                .unwrap();
        for case in manifest["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["path"].as_str().unwrap().contains("delta-gaps"))
        {
            let bytes = std::fs::read(fixtures().join(case["path"].as_str().unwrap())).unwrap();
            let request = DecodeRequest::new("gaps.bmp");
            let decoder = Decoder::new(&bytes, &request).unwrap().unwrap();
            let mut output = vec![0xa5; decoder.total_bytes() as usize];
            decoder.read_image(&mut output).unwrap();
            let hex = case["rgba8_hex"].as_str().unwrap();
            let expected: Vec<_> = (0..hex.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
                .collect();
            assert_eq!(output, expected);
        }
    }

    #[test]
    fn rle8_palette_validation_covers_every_count_value_word_and_tail() {
        let request = DecodeRequest::new("palette.bmp");
        let mut table = [[0; 4]; 256];
        for (i, pixel) in table.iter_mut().enumerate() {
            *pixel = [i as u8, (i as u8) ^ 1, (i as u8).wrapping_mul(3), 255];
        }
        let mut decoder = Decoder {
            bytes: &[],
            palette: table,
            colors: 1,
            request: &request,
            at: 0,
            end: 0,
            width: 17,
            height: 1,
            bits: 8,
        };
        for colors in 1..=256 {
            decoder.colors = colors;
            for index in 0..=255u8 {
                for position in [0, 7, 8, 15, 16] {
                    let mut data = [0u8; 17];
                    data[position] = index;
                    let mut output = [0xa5u8; 68];
                    let result = decoder.absolute(&mut output, 0, 0, 17, &data);
                    assert_eq!(
                        result.is_ok(),
                        usize::from(index) < colors,
                        "colors={colors} index={index} position={position}"
                    );
                    if result.is_ok() {
                        assert_eq!(
                            &output[position * 4..position * 4 + 4],
                            &table[usize::from(index)]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn rle4_encoded_counts_and_odd_absolute_tails_preserve_exact_spans() {
        let request = DecodeRequest::new("palette4.bmp");
        let mut table = [[0; 4]; 256];
        for (i, pixel) in table.iter_mut().enumerate() {
            *pixel = [i as u8, (i as u8) ^ 1, (i as u8).wrapping_mul(3), 255];
        }
        let mut decoder = Decoder {
            bytes: &[],
            palette: table,
            colors: 1,
            request: &request,
            at: 0,
            end: 0,
            width: 255,
            height: 1,
            bits: 4,
        };
        for colors in 1..=16 {
            decoder.colors = colors;
            for packed in 0..=255u8 {
                for count in 1..=255 {
                    let mut output = [0xa5u8; 1020];
                    let valid = usize::from(packed >> 4) < colors
                        && (count == 1 || usize::from(packed & 15) < colors);
                    let result = decoder.encoded(&mut output, 0, 0, count, packed);
                    assert_eq!(
                        result.is_ok(),
                        valid,
                        "colors={colors} packed={packed} count={count}"
                    );
                    if valid {
                        for (i, pixel) in output[..count * 4].chunks_exact(4).enumerate() {
                            let index = if i % 2 == 0 { packed >> 4 } else { packed & 15 };
                            assert_eq!(pixel, table[usize::from(index)]);
                        }
                    }
                    assert!(output[count * 4..].iter().all(|&v| v == 0xa5));
                }
            }
            for count in (3..=255).step_by(2) {
                for high in 0..16u8 {
                    let mut data = [0u8; 128];
                    data[count / 2] = high << 4 | 15;
                    let mut output = [0xa5u8; 1020];
                    let result = decoder.absolute(&mut output, 0, 0, count, &data);
                    assert_eq!(result.is_ok(), usize::from(high) < colors);
                    if result.is_ok() {
                        assert_eq!(&output[(count - 1) * 4..count * 4], &table[usize::from(high)]);
                    }
                    assert!(output[count * 4..].iter().all(|&v| v == 0xa5));
                }
            }
        }
    }

    #[test]
    fn unused_rle4_low_nibble_is_not_a_palette_access() {
        let raster = crate::raster::decode_raster_bytes(
            bitmap(4, 1, 1, &[1, 0x1f, 0, 1]),
            &DecodeRequest::new("one.bmp"),
        )
        .unwrap();
        let RasterPixels::Rgba8(pixels) = raster.pixels() else {
            panic!()
        };
        assert_eq!(&pixels[..], &[255, 0, 0, 255]);
    }
    #[test]
    fn cancellation_after_output_admission_refuses_publication() {
        use std::sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        };
        let generation = Arc::new(AtomicU64::new(1));
        let mut request = DecodeRequest::new("cancel.bmp");
        request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 1));
        let bytes = bitmap(8, 1, 1, &[1, 1, 0, 1]);
        let decoder = Decoder::new(&bytes, &request).unwrap().unwrap();
        let root = MemoryBudget::new(4);
        let mut output = root.try_buffer(4, 0u8).unwrap();
        generation.store(2, Ordering::Release);
        assert!(decoder.read_image(&mut output).is_err());
        drop(output);
        assert_eq!(root.used(), 0);
    }
    #[test]
    #[ignore = "manual paired native versus legacy BMP RLE CPU decode timing"]
    fn paired_native_rle_decode_cpu() {
        for bits in [4, 8] {
            for absolute in [false, true] {
                let pattern = if absolute { "absolute" } else { "encoded" };
                let mut body = Vec::new();
                for y in 0..512usize {
                    let mut x = 0;
                    for (part, count) in [255u8, 253, 4].into_iter().enumerate() {
                        if absolute {
                            body.extend_from_slice(&[0, count]);
                            let indices: Vec<u8> =
                                (0..usize::from(count)).map(|i| ((x + i + y) % 8) as u8).collect();
                            let mut packed: Vec<u8> = if bits == 8 {
                                indices
                            } else {
                                indices
                                    .chunks(2)
                                    .map(|p| p[0] << 4 | p.get(1).copied().unwrap_or(0))
                                    .collect()
                            };
                            if packed.len() % 2 != 0 {
                                packed.push(0);
                            }
                            body.extend_from_slice(&packed);
                        } else {
                            let high = ((y + part) % 8) as u8;
                            let index = if bits == 8 {
                                high
                            } else {
                                high << 4 | ((high + 1) % 8)
                            };
                            body.extend_from_slice(&[count, index]);
                        }
                        x += usize::from(count);
                    }
                    body.extend_from_slice(&[0, 0]);
                }
                body.extend_from_slice(&[0, 1]);
                let bytes = bitmap(bits, 512, 512, &body);
                let request = DecodeRequest::new("benchmark.bmp");
                let decode = |native| {
                    let decoder: Box<dyn ImageDecoder + '_> = if native {
                        Box::new(Decoder::new(&bytes, &request).unwrap().unwrap())
                    } else {
                        Box::new(image::codecs::bmp::BmpDecoder::new(std::io::Cursor::new(&bytes)).unwrap())
                    };
                    image::DynamicImage::from_decoder(decoder)
                        .unwrap()
                        .into_rgba8()
                        .into_raw()
                };
                assert_eq!(decode(true), decode(false));
                for _ in 0..4 {
                    std::hint::black_box(decode(true));
                    std::hint::black_box(decode(false));
                }
                let timing = |native| {
                    let started = std::time::Instant::now();
                    for _ in 0..4 {
                        std::hint::black_box(decode(native));
                    }
                    started.elapsed().as_nanos() / 4
                };
                for sample in 0..30 {
                    let (native, legacy) = if sample % 2 == 0 {
                        (timing(true), timing(false))
                    } else {
                        let legacy = timing(false);
                        (timing(true), legacy)
                    };
                    eprintln!(
                        "RLE CPU bits={bits} pattern={pattern} sample={sample} native_ns={native} legacy_ns={legacy}"
                    );
                }
            }
        }
    }
}
