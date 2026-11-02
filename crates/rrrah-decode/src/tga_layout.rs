//! Legacy TGA row interleaving after the common decoder normalizes origin.
use crate::{DecodeError, DecodeRequest, raster::RasterDecodeError};
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidTgaLayout(reason)
}
#[derive(Clone, Copy)]
pub(super) struct Layout {
    ways: usize,
    top: bool,
}
impl Layout {
    pub(super) fn parse(bytes: &[u8], request: &DecodeRequest) -> Result<Self, RasterDecodeError> {
        request.check_cancelled()?;
        let descriptor = *bytes.get(17).ok_or_else(|| bad("truncated header"))?;
        let ways = match descriptor >> 6 {
            0 => 1,
            1 => 2,
            2 => 4,
            _ => return Err(bad("reserved interleaving mode")),
        };
        // TGA2 removed the older interleaving field and reserves both bits.
        if ways != 1 && bytes.ends_with(b"TRUEVISION-XFILE.\0") {
            return Err(bad("TGA2 reserved descriptor bits"));
        }
        Ok(Self {
            ways,
            top: descriptor & 32 != 0,
        })
    }
    fn source_row(self, y: usize, height: usize) -> usize {
        let logical = if self.top { y } else { height - 1 - y };
        let phase = logical % self.ways;
        let packed = (0..phase)
            .map(|p| height.saturating_sub(p).div_ceil(self.ways))
            .sum::<usize>()
            + logical / self.ways;
        if self.top { packed } else { height - 1 - packed }
    }
    pub(super) fn apply(
        self,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        request: &DecodeRequest,
    ) -> Result<(), RasterDecodeError> {
        request.check_cancelled()?;
        if self.ways == 1 || height <= 1 {
            return Ok(());
        }
        let height = height as usize;
        let stride = (width as usize)
            .checked_mul(4)
            .ok_or_else(|| bad("row size overflow"))?;
        if stride == 0 || stride.checked_mul(height) != Some(pixels.len()) {
            return Err(bad("inconsistent decoded extent"));
        }
        let marks = height.div_ceil(8);
        let scratch = stride.checked_add(marks).ok_or_else(|| bad("scratch overflow"))?;
        let _credit = request
            .memory_budget
            .as_ref()
            .map(|b| b.try_reserve(scratch as u64))
            .transpose()
            .map_err(DecodeError::Memory)?;
        let mut temporary = Vec::new();
        temporary
            .try_reserve_exact(stride)
            .map_err(|_| RasterDecodeError::OutputTooLarge)?;
        temporary.resize(stride, 0);
        let mut visited = Vec::new();
        visited
            .try_reserve_exact(marks)
            .map_err(|_| RasterDecodeError::OutputTooLarge)?;
        visited.resize(marks, 0u8);
        for start in 0..height {
            request.check_cancelled()?;
            if visited[start / 8] & (1 << (start % 8)) != 0 {
                continue;
            }
            let mut current = start;
            temporary.copy_from_slice(&pixels[start * stride..(start + 1) * stride]);
            loop {
                request.check_cancelled()?;
                visited[current / 8] |= 1 << (current % 8);
                let source = self.source_row(current, height);
                if source == start {
                    pixels[current * stride..(current + 1) * stride].copy_from_slice(&temporary);
                    break;
                }
                pixels.copy_within(source * stride..(source + 1) * stride, current * stride);
                current = source;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use crate::{DecodeRequest, decode_raster};
    use rrrah_core::{MemoryBudget, RasterPixels};
    #[test]
    fn ffmpeg_legacy_interleaved_images_preserve_all_pixels_and_credit() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/raster/tga-interleave-reference.json"
        ))
        .unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for row in reference["rows"].as_array().unwrap() {
            let name = row["name"].as_str().unwrap();
            let budget = MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new(root.join(name));
            request.memory_budget = Some(budget.clone());
            let frame = decode_raster(&request).unwrap_or_else(|e| panic!("{name}: {e}"));
            let expected = std::fs::read(root.join(row["rgba8_file"].as_str().unwrap())).unwrap();
            let RasterPixels::Rgba8(pixels) = frame.pixels() else {
                panic!()
            };
            assert_eq!(&pixels[..], &expected, "{name}");
            assert_eq!(
                (frame.width(), frame.height()),
                (
                    row["width"].as_u64().unwrap() as u32,
                    row["height"].as_u64().unwrap() as u32
                )
            );
            drop(frame);
            assert_eq!(budget.used(), 0, "{name}");
        }
    }
    #[test]
    fn layout_refusal_and_exact_scratch_preserve_pixels_and_credit() {
        use super::*;
        let mut bytes = [0u8; 18];
        bytes[17] = 64 | 32;
        let request = DecodeRequest::new("legacy.tga");
        let layout = Layout::parse(&bytes, &request).unwrap();
        let pixels = (0..7 * 7 * 4).map(|v| v as u8).collect::<Vec<_>>();
        let mut request = DecodeRequest::new("legacy.tga");
        let short = MemoryBudget::new(28);
        request.memory_budget = Some(short.clone());
        let mut actual = pixels.clone();
        assert!(matches!(
            layout.apply(&mut actual, 7, 7, &request),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(actual, pixels);
        assert_eq!(short.used(), 0);
        let exact = MemoryBudget::new(29);
        request.memory_budget = Some(exact.clone());
        layout.apply(&mut actual, 7, 7, &request).unwrap();
        assert_eq!(exact.peak(), 29);
        assert_eq!(exact.used(), 0);
        let empty = MemoryBudget::new(0);
        request.memory_budget = Some(empty.clone());
        bytes[17] = 192;
        assert!(matches!(
            Layout::parse(&bytes, &request),
            Err(RasterDecodeError::InvalidTgaLayout("reserved interleaving mode"))
        ));
        bytes[17] = 64;
        let mut v2 = bytes.to_vec();
        v2.extend_from_slice(b"TRUEVISION-XFILE.\0");
        assert!(matches!(
            Layout::parse(&v2, &request),
            Err(RasterDecodeError::InvalidTgaLayout(
                "TGA2 reserved descriptor bits"
            ))
        ));
        let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1));
        request.cancellation = Some(crate::GenerationToken::new(generation, 0));
        let mut actual = pixels.clone();
        assert!(matches!(
            layout.apply(&mut actual, 7, 7, &request),
            Err(RasterDecodeError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(actual, pixels);
        assert_eq!(empty.peak(), 0);
    }
    #[test]
    fn row_cycles_are_bounded_permutations_for_nondivisible_and_short_heights() {
        use super::*;
        for height in 1..=1024 {
            for ways in [2, 4] {
                for top in [false, true] {
                    let layout = Layout { ways, top };
                    let mut seen = vec![false; height];
                    for y in 0..height {
                        let source = layout.source_row(y, height);
                        assert!(source < height);
                        assert!(!seen[source]);
                        seen[source] = true;
                    }
                    assert!(seen.into_iter().all(|v| v));
                }
            }
        }
    }

    #[test]
    fn public_tga_exact_peak_short_budget_and_precancel_release_all_credit() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/tga-interleave-rgba-h7-4way-origin3-rle.tga");
        let full = MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(full.clone());
        drop(decode_raster(&request).unwrap());
        let peak = full.peak();
        assert_eq!(full.used(), 0);
        let exact = MemoryBudget::new(peak);
        request.memory_budget = Some(exact.clone());
        drop(decode_raster(&request).unwrap());
        assert_eq!(exact.used(), 0);
        let short = MemoryBudget::new(peak - 1);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode_raster(&request),
            Err(crate::raster::RasterDecodeError::Source(
                crate::DecodeError::Memory(_)
            ))
        ));
        assert_eq!(short.used(), 0);
        let zero = MemoryBudget::new(0);
        request.memory_budget = Some(zero.clone());
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
            0,
        ));
        assert!(matches!(
            decode_raster(&request),
            Err(crate::raster::RasterDecodeError::Source(
                crate::DecodeError::Cancelled
            ))
        ));
        assert_eq!(zero.peak(), 0);
    }
}
