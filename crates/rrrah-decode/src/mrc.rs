//! MRC2014 scalar sections in storage order; physical coordinates are retained.
use crate::sample::half_to_f32 as half;
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidMrc(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.get(208..212) == Some(b"MAP ")
}
#[derive(Debug, Clone)]
pub struct MrcImage {
    pub raster: DecodedRaster,
    /// Complete original header, including geometry, statistics and labels.
    pub header: [u8; 1024],
    /// Original extended metadata; exporter-specific interpretation is caller-owned.
    pub extended_header: Arc<Vec<u8>>,
    pub big_endian: bool,
    /// Physical axes corresponding to storage columns, rows and sections (1=X,2=Y,3=Z).
    pub axes: [u32; 3],
}
impl MrcImage {
    pub fn windowed(&self, minimum: f32, maximum: f32) -> Result<DecodedRaster, RasterDecodeError> {
        crate::scientific::windowed(&self.raster, minimum, maximum, bad)
    }
}
pub fn decode_mrc(request: &DecodeRequest) -> Result<MrcImage, RasterDecodeError> {
    decode(&read_bounded(request)?, request)
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<MrcImage, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() < 1024 || !has_magic(b) {
        return Err(bad("missing MRC2014 header/signature"));
    }
    let be = match &b[212..214] {
        [0x44, 0x44 | 0x41] => false,
        [0x11, 0x11] => true,
        _ => return Err(bad("unsupported machine stamp")),
    };
    let word = |at: usize| {
        let v = b[at..at + 4].try_into().unwrap();
        if be {
            u32::from_be_bytes(v)
        } else {
            u32::from_le_bytes(v)
        }
    };
    if !matches!(word(108), 20140 | 20141) {
        return Err(bad("unsupported format version"));
    }
    let dimensions = [word(0), word(4), word(8)];
    if dimensions.iter().any(|v| *v == 0 || *v > 65536) {
        return Err(bad("invalid dimensions"));
    }
    let axes = [word(64), word(68), word(72)];
    let mut sorted = axes;
    sorted.sort();
    if sorted != [1, 2, 3] {
        return Err(bad("invalid physical-axis permutation"));
    }
    if word(220) > 10 {
        return Err(bad("invalid label count"));
    }
    let mode = word(12);
    let bytes = match mode {
        0 => 1usize,
        1 | 6 | 12 => 2,
        2 => 4,
        _ => return Err(bad("unsupported scalar mode")),
    };
    let count = dimensions
        .iter()
        .try_fold(1usize, |n, v| n.checked_mul(*v as usize))
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    let expected = count
        .checked_mul(bytes)
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    let plane = dimensions[0] as usize * dimensions[1] as usize;
    if expected as u64 > MAX_RASTER_BYTES || plane as u64 * 16 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let extended = word(92) as usize;
    let start = 1024usize
        .checked_add(extended)
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    if start.checked_add(expected) != Some(b.len()) {
        return Err(bad("file size differs from header/array bounds"));
    }
    if request.image_index >= dimensions[2] as usize {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let first = start + request.image_index * plane * bytes;
    let mut values = Vec::with_capacity(plane * 4);
    for (i, sample) in b[first..first + plane * bytes].chunks_exact(bytes).enumerate() {
        if i % 4096 == 0 {
            request.check_cancelled()?;
        }
        let value = match mode {
            0 => f32::from(sample[0] as i8),
            1 => {
                let a = sample.try_into().unwrap();
                f32::from(if be {
                    i16::from_be_bytes(a)
                } else {
                    i16::from_le_bytes(a)
                })
            }
            6 | 12 => {
                let a = sample.try_into().unwrap();
                let n = if be {
                    u16::from_be_bytes(a)
                } else {
                    u16::from_le_bytes(a)
                };
                if mode == 12 { half(n) } else { f32::from(n) }
            }
            _ => {
                let a = sample.try_into().unwrap();
                if be {
                    f32::from_be_bytes(a)
                } else {
                    f32::from_le_bytes(a)
                }
            }
        };
        values.extend([value, value, value, 1.]);
    }
    request.check_cancelled()?;
    let raster = DecodedRaster::new(
        dimensions[0],
        dimensions[1],
        RasterPixels::Rgba32Float(Arc::new(values).into()),
        RasterColorSpace::Unspecified,
    )?
    .with_image_selection(request.image_index, dimensions[2] as usize)?;
    Ok(MrcImage {
        raster,
        header: b[..1024].try_into().unwrap(),
        extended_header: Arc::new(b[1024..start].to_vec()),
        big_endian: be,
        axes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_native_sample_oracles_and_metadata() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for line in include_str!("../../../tests/fixtures/raster/mrc-manifest.tsv")
            .lines()
            .filter(|s| !s.starts_with('#') && !s.is_empty())
        {
            let columns: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(columns[0]));
            request.image_index = columns[1].parse().unwrap();
            let image = decode_mrc(&request).unwrap();
            assert_eq!((image.raster.width(), image.raster.height()), (5, 3));
            assert_eq!(image.raster.image_index(), request.image_index);
            assert_eq!(image.raster.image_count(), 2);
            assert_eq!(image.axes, [3, 1, 2]);
            assert_eq!(&image.extended_header[..], &(0u8..32).collect::<Vec<_>>()[..]);
            assert_eq!(image.big_endian, columns[0].contains("-big"));
            assert_eq!(image.raster.color_space(), &RasterColorSpace::Unspecified);
            let source = std::fs::read(&request.path).unwrap();
            assert_eq!(&image.header[..], &source[..1024]);
            let oracle = std::fs::read(root.join(columns[4])).unwrap();
            let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
                panic!()
            };
            assert_eq!(oracle.len(), values.len() * 4);
            for (value, expected) in values.iter().zip(oracle.chunks_exact(4)) {
                assert_eq!(
                    value.to_bits(),
                    u32::from_le_bytes(expected.try_into().unwrap()),
                    "{} {}",
                    columns[0],
                    columns[1]
                );
            }
            let displayed = image.windowed(-128., 65535.).unwrap();
            assert_eq!(displayed.image_index(), request.image_index);
            assert_eq!(displayed.image_count(), 2);
            assert_eq!(displayed.color_space(), &RasterColorSpace::LinearSrgb);
        }
    }
    #[test]
    fn exhaustive_half_conversion_matches_numpy() {
        let oracle = include_bytes!("../../../tests/fixtures/raster/mrc-half-all.r32f");
        for (bits, expected) in oracle.chunks_exact(4).enumerate() {
            assert_eq!(
                half(bits as u16).to_bits(),
                u32::from_le_bytes(expected.try_into().unwrap()),
                "half {bits:04x}"
            );
        }
    }
    #[test]
    fn nonfinite_array_is_retained_and_needs_window_policy() {
        let mut bytes = include_bytes!("../../../tests/fixtures/raster/mrc-f32-little.mrc").to_vec();
        bytes[1056..1060].copy_from_slice(&f32::NAN.to_le_bytes());
        let image = decode(&bytes, &DecodeRequest::new("nonfinite.mrc")).unwrap();
        let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
            panic!()
        };
        assert!(values[0].is_nan());
        assert!(matches!(
            image.windowed(0., 1.),
            Err(RasterDecodeError::InvalidMrc(_))
        ));
    }
    #[test]
    fn malformed_bounds_modes_axes_and_selection_fail() {
        let original = include_bytes!("../../../tests/fixtures/raster/mrc-i16-little.mrc");
        let request = DecodeRequest::new("synthetic.mrc");
        for end in [0, 211, 1023, 1050, original.len() - 1] {
            assert!(decode(&original[..end], &request).is_err());
        }
        for (at, value) in [
            (0, 0),
            (4, 65537),
            (8, u32::MAX),
            (12, 3),
            (12, 4),
            (12, 101),
            (64, 0),
            (68, 3),
            (92, u32::MAX),
            (108, 0),
            (220, 11),
        ] {
            let mut b = original.to_vec();
            b[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&b, &request).is_err(), "offset {at} value {value}");
        }
        for at in [208, 212, 213] {
            let mut b = original.to_vec();
            b[at] = 0;
            assert!(decode(&b, &request).is_err());
        }
        let mut b = original.to_vec();
        b.push(0);
        assert!(decode(&b, &request).is_err());
        let mut request = request;
        request.image_index = 2;
        assert!(matches!(
            decode(original, &request),
            Err(RasterDecodeError::Source(
                crate::DecodeError::UnsupportedImageIndex { index: 2 }
            ))
        ));
    }
    #[test]
    fn selected_slice_routes_under_raw_extension_without_guessed_display() {
        let path = std::env::temp_dir().join(format!("rrrah-mrc-router-{}.cr3", std::process::id()));
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/mrc-f32-big.mrc"),
        )
        .unwrap();
        let mut request = DecodeRequest::new(&path);
        request.image_index = 1;
        let result = crate::decode_image(&request);
        std::fs::remove_file(path).unwrap();
        let crate::DecodedImage::Raster(raster) = result.unwrap() else {
            panic!()
        };
        assert_eq!(raster.image_index(), 1);
        assert_eq!(raster.image_count(), 2);
        assert!(crate::prepare_raster_for_display(&raster).is_err());
    }
}
