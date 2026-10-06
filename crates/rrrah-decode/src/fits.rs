//! FITS primary/IMAGE scalar arrays, retaining native values and original cards.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{collections::BTreeMap, sync::Arc};
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidFits(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"SIMPLE  =")
}
#[derive(Debug, Clone)]
pub enum FitsSamples {
    U8(Arc<Vec<u8>>),
    I16(Arc<Vec<i16>>),
    I32(Arc<Vec<i32>>),
    I64(Arc<Vec<i64>>),
    F32(Arc<Vec<f32>>),
    F64(Arc<Vec<f64>>),
}
impl FitsSamples {
    pub fn len(&self) -> usize {
        match self {
            Self::U8(v) => v.len(),
            Self::I16(v) => v.len(),
            Self::I32(v) => v.len(),
            Self::I64(v) => v.len(),
            Self::F32(v) => v.len(),
            Self::F64(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn value(&self, i: usize) -> (f64, Option<i64>) {
        match self {
            Self::U8(v) => (f64::from(v[i]), Some(i64::from(v[i]))),
            Self::I16(v) => (f64::from(v[i]), Some(i64::from(v[i]))),
            Self::I32(v) => (f64::from(v[i]), Some(i64::from(v[i]))),
            Self::I64(v) => (v[i] as f64, Some(v[i])),
            Self::F32(v) => (f64::from(v[i]), None),
            Self::F64(v) => (v[i], None),
        }
    }
}
#[derive(Debug, Clone)]
pub struct FitsImage {
    pub width: u32,
    pub height: u32,
    /// Global plane selection across primary/IMAGE HDUs, skipping tables/empty HDUs.
    pub image_index: usize,
    pub image_count: usize,
    pub hdu_index: usize,
    pub section_index: usize,
    pub cards: Vec<[u8; 80]>,
    pub samples: FitsSamples,
    pub bscale: f64,
    pub bzero: f64,
    pub blank: Option<i64>,
}
impl FitsImage {
    fn physical_integer(&self, i: usize) -> Option<i128> {
        let (_, integer) = self.samples.value(i);
        let integer = integer?;
        if Some(integer) == self.blank {
            return None;
        }
        let limit = 2f64.powi(127);
        if self.bscale < -limit || self.bscale >= limit || self.bzero < -limit || self.bzero >= limit {
            return None;
        }
        let scale = self.bscale as i128;
        let zero = self.bzero as i128;
        if scale as f64 != self.bscale || zero as f64 != self.bzero {
            return None;
        }
        i128::from(integer).checked_mul(scale)?.checked_add(zero)
    }
    fn physical(&self, i: usize) -> Result<f64, RasterDecodeError> {
        let (raw, integer) = self.samples.value(i);
        if integer.is_some() && integer == self.blank {
            return Ok(f64::NAN);
        }
        if let Some(value) = self.physical_integer(i) {
            return Ok(value as f64);
        }
        if integer.is_some_and(|n| raw as i128 != i128::from(n)) {
            return Err(bad("wide integer scaling requires exact wider arithmetic"));
        }
        if self.bscale == 1. && self.bzero == 0. {
            return Ok(raw);
        }
        let value = raw * self.bscale + self.bzero;
        if raw.is_finite() && !value.is_finite() {
            return Err(bad("physical scaling overflow"));
        }
        Ok(value)
    }
    /// Convert only when physical values are exactly representable by the common float32 raster.
    /// Higher-precision native samples remain available through `samples`.
    pub fn to_raster(&self) -> Result<DecodedRaster, RasterDecodeError> {
        self.raster(None)
    }
    fn raster(&self, request: Option<&DecodeRequest>) -> Result<DecodedRaster, RasterDecodeError> {
        let mut values = Vec::with_capacity(self.samples.len() * 4);
        for i in 0..self.samples.len() {
            if i % 4096 == 0 {
                if let Some(r) = request {
                    r.check_cancelled()?;
                }
            }
            let physical = self.physical(i)?;
            let value = if self.bscale == 1. && self.bzero == 0. {
                if let FitsSamples::F32(v) = &self.samples {
                    v[i]
                } else {
                    physical as f32
                }
            } else {
                physical as f32
            };
            if self
                .physical_integer(i)
                .is_some_and(|n| f64::from(value) >= 2f64.powi(127) || value as i128 != n)
            {
                return Err(bad("native integer precision requires the FITS sample API"));
            }
            if physical.is_finite() && f64::from(value) != physical {
                return Err(bad(
                    "physical precision requires explicit windowing or the FITS sample API",
                ));
            }
            values.extend([value, value, value, 1.]);
        }
        self.frame(values)
    }
    fn frame(&self, values: Vec<f32>) -> Result<DecodedRaster, RasterDecodeError> {
        Ok(DecodedRaster::new(
            self.width,
            self.height,
            RasterPixels::Rgba32Float(Arc::new(values).into()),
            RasterColorSpace::Unspecified,
        )?
        .with_image_selection(self.image_index, self.image_count)?)
    }
    /// Explicit physical-unit grayscale display. Native integer samples remain untouched;
    /// display scaling is evaluated in float64 before conversion to float32 grayscale.
    pub fn windowed(&self, minimum: f64, maximum: f64) -> Result<DecodedRaster, RasterDecodeError> {
        self.windowed_request(minimum, maximum, None)
    }
    pub(crate) fn windowed_request(
        &self,
        minimum: f64,
        maximum: f64,
        request: Option<&DecodeRequest>,
    ) -> Result<DecodedRaster, RasterDecodeError> {
        let range = maximum - minimum;
        if !minimum.is_finite() || !maximum.is_finite() || !range.is_finite() || range <= 0. {
            return Err(bad("window must have finite increasing physical bounds"));
        }
        let mut values = Vec::with_capacity(self.samples.len() * 4);
        for i in 0..self.samples.len() {
            if i % 4096 == 0 {
                if let Some(r) = request {
                    r.check_cancelled()?;
                }
            }
            let physical = self.physical(i)?;
            if !physical.is_finite() {
                return Err(bad(
                    "undefined/nonfinite sample requires an explicit missing-value policy",
                ));
            }
            let gray = ((physical - minimum) / range).clamp(0., 1.) as f32;
            values.extend([gray, gray, gray, 1.]);
        }
        Ok(DecodedRaster::new(
            self.width,
            self.height,
            RasterPixels::Rgba32Float(Arc::new(values).into()),
            RasterColorSpace::LinearSrgb,
        )?
        .with_image_selection(self.image_index, self.image_count)?)
    }
}
struct Header {
    cards: Vec<[u8; 80]>,
    fields: BTreeMap<String, String>,
    end: usize,
}
fn parse_header(b: &[u8], start: usize) -> Result<Header, RasterDecodeError> {
    let mut cards = Vec::new();
    let mut fields = BTreeMap::new();
    let mut at = start;
    loop {
        if at - start >= 1024 * 1024 {
            return Err(bad("oversized header"));
        }
        let card: [u8; 80] = b
            .get(at..at + 80)
            .ok_or_else(|| bad("truncated header"))?
            .try_into()
            .unwrap();
        if card.iter().any(|c| !(32..=126).contains(c)) {
            return Err(bad("header cards must be printable ASCII"));
        }
        at += 80;
        let key = std::str::from_utf8(&card[..8]).unwrap().trim_end();
        if key == "END" {
            if card[8..].iter().any(|v| *v != b' ') {
                return Err(bad("END card has a value"));
            }
            let end = at.div_ceil(2880) * 2880;
            if b.get(at..end).is_none_or(|v| v.iter().any(|v| *v != b' ')) {
                return Err(bad("invalid header padding"));
            }
            cards.push(card);
            return Ok(Header { cards, fields, end });
        }
        if !matches!(key, "" | "COMMENT" | "HISTORY") && &card[8..10] == b"= " {
            let mut quoted = false;
            let mut i = 10;
            let mut end = 80;
            while i < 80 {
                if card[i] == b'\'' {
                    if quoted && i + 1 < 80 && card[i + 1] == b'\'' {
                        i += 2;
                        continue;
                    }
                    quoted = !quoted;
                }
                if card[i] == b'/' && !quoted {
                    end = i;
                    break;
                }
                i += 1;
            }
            if quoted {
                return Err(bad("unterminated string"));
            }
            let value = std::str::from_utf8(&card[10..end]).unwrap().trim().to_owned();
            if fields.insert(key.to_owned(), value).is_some()
                && (matches!(
                    key,
                    "SIMPLE"
                        | "XTENSION"
                        | "BITPIX"
                        | "NAXIS"
                        | "BSCALE"
                        | "BZERO"
                        | "BLANK"
                        | "PCOUNT"
                        | "GCOUNT"
                        | "GROUPS"
                ) || key.starts_with("NAXIS"))
            {
                return Err(bad("duplicate structural keyword"));
            }
        }
        cards.push(card);
    }
}
fn number(h: &Header, key: &str) -> Result<i64, RasterDecodeError> {
    h.fields
        .get(key)
        .ok_or_else(|| bad("missing required keyword"))?
        .parse()
        .map_err(|_| bad("invalid integer keyword"))
}
fn scaling(h: &Header, key: &str, default: f64) -> Result<f64, RasterDecodeError> {
    let Some(v) = h.fields.get(key) else {
        return Ok(default);
    };
    let value = v
        .replace('D', "E")
        .parse::<f64>()
        .map_err(|_| bad("invalid scaling keyword"))?;
    if !value.is_finite() {
        return Err(bad("nonfinite scaling"));
    }
    Ok(value)
}
pub fn decode_fits(request: &DecodeRequest) -> Result<FitsImage, RasterDecodeError> {
    request.check_cancelled()?;
    decode(&read_bounded(request)?, request)
}
pub(crate) fn decode_raster(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    decode(b, request)?.raster(Some(request))
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<FitsImage, RasterDecodeError> {
    request.check_cancelled()?;
    if !has_magic(b) || b.len() % 2880 != 0 {
        return Err(bad("missing signature or incomplete FITS record"));
    }
    let mut at = 0;
    let mut hdu = 0;
    let mut total = 0usize;
    let mut selected = None;
    while at < b.len() {
        request.check_cancelled()?;
        if hdu >= 65536 {
            return Err(bad("too many HDUs"));
        }
        let header = parse_header(b, at)?;
        let key = |i: usize| {
            header
                .cards
                .get(i)
                .map(|c| std::str::from_utf8(&c[..8]).unwrap().trim_end())
        };
        let image = if hdu == 0 {
            if key(0) != Some("SIMPLE") || header.fields.get("SIMPLE").map(String::as_str) != Some("T") {
                return Err(bad("primary SIMPLE must be true"));
            }
            true
        } else {
            if key(0) != Some("XTENSION") {
                return Err(bad("missing extension type"));
            }
            let typ = header
                .fields
                .get("XTENSION")
                .ok_or_else(|| bad("missing extension value"))?
                .trim_matches('\'')
                .trim();
            if !matches!(typ, "IMAGE" | "BINTABLE" | "TABLE") {
                return Err(bad("unsupported extension type"));
            }
            if header.fields.get("ZIMAGE").is_some_and(|v| v == "T") {
                return Err(bad("compressed image convention unsupported"));
            }
            typ == "IMAGE"
        };
        if key(1) != Some("BITPIX") || key(2) != Some("NAXIS") {
            return Err(bad("required keyword order"));
        }
        if header.fields.get("GROUPS").is_some_and(|v| v == "T") {
            return Err(bad("random groups unsupported"));
        }
        let bitpix = number(&header, "BITPIX")?;
        let bytes = match bitpix {
            8 => 1usize,
            16 => 2,
            32 | -32 => 4,
            64 | -64 => 8,
            _ => return Err(bad("invalid BITPIX")),
        };
        let dimension = number(&header, "NAXIS")?;
        if !(0..=999).contains(&dimension) {
            return Err(bad("invalid axis count"));
        }
        let mut sizes = Vec::new();
        let mut count = if dimension == 0 { 0usize } else { 1usize };
        for axis in 1..=dimension {
            let name = format!("NAXIS{axis}");
            if key(axis as usize + 2) != Some(name.as_str()) {
                return Err(bad("axis keyword order"));
            }
            let size = usize::try_from(number(&header, &name)?).map_err(|_| bad("negative axis size"))?;
            count = count.checked_mul(size).ok_or(RasterDecodeError::OutputTooLarge)?;
            sizes.push(size);
        }
        let (pcount, gcount) = if hdu == 0 {
            (0usize, 1usize)
        } else {
            (
                usize::try_from(number(&header, "PCOUNT")?).map_err(|_| bad("negative PCOUNT"))?,
                usize::try_from(number(&header, "GCOUNT")?).map_err(|_| bad("negative GCOUNT"))?,
            )
        };
        if !image && (bitpix != 8 || dimension != 2 || gcount != 1) {
            return Err(bad("invalid table storage"));
        }
        if gcount == 0 || image && (pcount != 0 || gcount != 1) {
            return Err(bad("invalid image parameter/group count"));
        }
        let length = count
            .checked_add(pcount)
            .and_then(|v| v.checked_mul(gcount))
            .and_then(|v| v.checked_mul(bytes))
            .ok_or(RasterDecodeError::OutputTooLarge)?;
        let end = header
            .end
            .checked_add(length)
            .ok_or(RasterDecodeError::OutputTooLarge)?;
        let padded = end.checked_add(2879).ok_or(RasterDecodeError::OutputTooLarge)? / 2880 * 2880;
        if padded > b.len() {
            return Err(bad("truncated HDU data"));
        }
        let ascii_table = header
            .fields
            .get("XTENSION")
            .is_some_and(|v| v.trim_matches('\'').trim() == "TABLE");
        let fill = if ascii_table { b' ' } else { 0 };
        if b[end..padded].iter().any(|v| *v != fill) {
            return Err(bad("invalid data padding"));
        }
        if image && count > 0 {
            if !matches!(dimension, 2 | 3) || sizes.iter().any(|v| *v > 65536) {
                return Err(bad("only bounded scalar planes/volumes supported"));
            }
            if length as u64 > MAX_RASTER_BYTES || sizes[0] as u64 * sizes[1] as u64 * 16 > MAX_RASTER_BYTES {
                return Err(RasterDecodeError::OutputTooLarge);
            }
            let sections = if dimension == 3 { sizes[2] } else { 1 };
            let next = total
                .checked_add(sections)
                .ok_or(RasterDecodeError::OutputTooLarge)?;
            if request.image_index >= total && request.image_index < next {
                let plane = sizes[0] * sizes[1];
                let section = request.image_index - total;
                let data =
                    &b[header.end + section * plane * bytes..header.end + (section + 1) * plane * bytes];
                macro_rules! unpack {
                    ($t:ty,$variant:ident) => {{
                        let mut values = Vec::with_capacity(plane);
                        for (i, v) in data.chunks_exact(bytes).enumerate() {
                            if i % 4096 == 0 {
                                request.check_cancelled()?;
                            }
                            values.push(<$t>::from_be_bytes(v.try_into().unwrap()));
                        }
                        FitsSamples::$variant(Arc::new(values))
                    }};
                }
                let samples = match bitpix {
                    8 => FitsSamples::U8(Arc::new(data.to_vec())),
                    16 => unpack!(i16, I16),
                    32 => unpack!(i32, I32),
                    64 => unpack!(i64, I64),
                    -32 => unpack!(f32, F32),
                    _ => unpack!(f64, F64),
                };
                let bscale = scaling(&header, "BSCALE", 1.)?;
                let bzero = scaling(&header, "BZERO", 0.)?;
                let blank = if bitpix > 0 && header.fields.contains_key("BLANK") {
                    Some(number(&header, "BLANK")?)
                } else {
                    None
                };
                if blank.is_some_and(|v| match bitpix {
                    8 => !(0..=255).contains(&v),
                    16 => i16::try_from(v).is_err(),
                    32 => i32::try_from(v).is_err(),
                    _ => false,
                }) {
                    return Err(bad("BLANK outside stored integer range"));
                }
                selected = Some(FitsImage {
                    width: sizes[0] as u32,
                    height: sizes[1] as u32,
                    image_index: request.image_index,
                    image_count: 0,
                    hdu_index: hdu,
                    section_index: section,
                    cards: header.cards,
                    samples,
                    bscale,
                    bzero,
                    blank,
                });
            }
            total = next;
        }
        at = padded;
        hdu += 1;
    }
    request.check_cancelled()?;
    let mut selected = selected.ok_or(crate::DecodeError::UnsupportedImageIndex {
        index: request.image_index,
    })?;
    selected.image_count = total;
    Ok(selected)
}

#[cfg(test)]
mod exact_scaling_tests {
    use super::*;
    #[test]
    fn floating_bits_scaling_syntax_and_undefined_contracts() {
        let request = DecodeRequest::new("synthetic.fits");
        let source = include_bytes!("../../../tests/fixtures/raster/fits-nonfinite.fits");
        let image = decode(source, &request).unwrap();
        let raster = image.to_raster().unwrap();
        let (FitsSamples::F32(native), RasterPixels::Rgba32Float(values)) = (&image.samples, raster.pixels())
        else {
            panic!()
        };
        for (native, pixel) in native.iter().zip(values.chunks_exact(4)) {
            assert_eq!(native.to_bits(), pixel[0].to_bits());
        }
        assert_eq!(values[12].to_bits(), 0x80000000);
        assert!(image.windowed(0., 1.).is_err());
        let modify = |source: &[u8], old: &str, key: &str, value: &str| {
            let mut b = source.to_vec();
            let at = b[..2880]
                .chunks_exact(80)
                .position(|c| std::str::from_utf8(&c[..8]).unwrap().trim() == old)
                .unwrap()
                * 80;
            let card = format!("{key:<8}= {value:>20}");
            b[at..at + 80].fill(b' ');
            b[at..at + card.len()].copy_from_slice(card.as_bytes());
            b
        };
        let scaled = include_bytes!("../../../tests/fixtures/raster/fits-scaled.fits");
        let b = modify(scaled, "BZERO", "BZERO", "1.025D1");
        assert_eq!(decode(&b, &request).unwrap().bzero, 10.25);
        let b = modify(scaled, "BSCALE", "BSCALE", "NaN");
        assert!(decode(&b, &request).is_err());
        let b = modify(scaled, "BZERO", "BSCALE", "1.0");
        assert!(decode(&b, &request).is_err());
        let blank = include_bytes!("../../../tests/fixtures/raster/fits-blank.fits");
        let b = modify(blank, "BLANK", "BLANK", "32768");
        assert!(decode(&b, &request).is_err());
        let f32file = include_bytes!("../../../tests/fixtures/raster/fits-f32.fits");
        let b = modify(f32file, "OBJECT", "BLANK", "0");
        assert_eq!(decode(&b, &request).unwrap().blank, None);
        let b = modify(f32file, "OBJECT", "GROUPS", "T");
        assert!(decode(&b, &request).is_err());
    }
    #[test]
    fn wide_integer_offset_preserves_small_differences() {
        let mut image = FitsImage {
            width: 1,
            height: 1,
            image_index: 0,
            image_count: 1,
            hdu_index: 0,
            section_index: 0,
            cards: vec![],
            samples: FitsSamples::I64(Arc::new(vec![i64::MAX])),
            bscale: 1.,
            bzero: -(2f64.powi(63)),
            blank: None,
        };
        assert_eq!(image.physical(0).unwrap(), -1.);
        let raster = image.to_raster().unwrap();
        let RasterPixels::Rgba32Float(v) = raster.pixels() else {
            panic!()
        };
        assert_eq!(&v[..], &[-1., -1., -1., 1.]);
        let raster = image.windowed(-2., 0.).unwrap();
        let RasterPixels::Rgba32Float(v) = raster.pixels() else {
            panic!()
        };
        assert_eq!(&v[..], &[0.5, 0.5, 0.5, 1.]);
        image.bzero = 0.;
        assert!(image.to_raster().is_err());
        image.bscale = 0.5;
        image.bzero = -(2f64.powi(62));
        assert!(image.windowed(-1., 1.).is_err());
        let FitsSamples::I64(v) = image.samples else {
            panic!()
        };
        assert_eq!(v[0], i64::MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_native_physical_and_window_oracles() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for line in include_str!("../../../tests/fixtures/raster/fits-manifest.tsv")
            .lines()
            .filter(|s| !s.starts_with('#') && !s.is_empty())
        {
            let c: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(c[0]));
            request.image_index = c[1].parse().unwrap();
            let image = decode_fits(&request).unwrap();
            assert_eq!(
                (image.width, image.height),
                (c[2].parse().unwrap(), c[3].parse().unwrap())
            );
            assert_eq!(image.image_index, request.image_index);
            let mut native = Vec::new();
            macro_rules! native_bytes {
                ($v:expr) => {
                    for v in $v.iter() {
                        native.extend_from_slice(&v.to_le_bytes());
                    }
                };
            }
            match &image.samples {
                FitsSamples::U8(v) => native.extend_from_slice(v),
                FitsSamples::I16(v) => native_bytes!(v),
                FitsSamples::I32(v) => native_bytes!(v),
                FitsSamples::I64(v) => native_bytes!(v),
                FitsSamples::F32(v) => native_bytes!(v),
                FitsSamples::F64(v) => native_bytes!(v),
            }
            assert_eq!(native, std::fs::read(root.join(c[5])).unwrap(), "native {}", c[0]);
            let oracle = std::fs::read(root.join(c[6])).unwrap();
            for (i, expected) in oracle.chunks_exact(8).enumerate() {
                let expected = f64::from_le_bytes(expected.try_into().unwrap());
                let actual = image.physical(i).unwrap();
                assert!(
                    expected.is_nan() && actual.is_nan() || expected == actual,
                    "physical {} {i}: {expected} != {actual}",
                    c[0]
                );
            }
            if oracle
                .chunks_exact(8)
                .any(|b| !f64::from_le_bytes(b.try_into().unwrap()).is_finite())
            {
                assert!(image.windowed(-100., 1000.).is_err());
                continue;
            }
            let frame = image.windowed(-100., 1000.).unwrap();
            assert_eq!(
                (frame.image_index(), frame.image_count()),
                (image.image_index, image.image_count)
            );
            let RasterPixels::Rgba32Float(values) = frame.pixels() else {
                panic!()
            };
            let oracle = std::fs::read(root.join(c[7])).unwrap();
            for (value, expected) in values.iter().zip(oracle.chunks_exact(4)) {
                assert_eq!(
                    value.to_bits(),
                    u32::from_le_bytes(expected.try_into().unwrap()),
                    "window {}",
                    c[0]
                );
            }
            if c[0].contains("fits-i32.") || c[0].contains("fits-i64.") || c[0].contains("fits-f64.") {
                assert!(image.to_raster().is_err());
            }
        }
    }
    #[test]
    fn tables_are_skipped_and_header_metadata_is_retained() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut request = DecodeRequest::new(root.join("fits-multi-hdu.fits"));
        for (index, hdu, section, w, h) in [(0, 2, 0, 5, 3), (1, 2, 1, 5, 3), (2, 3, 0, 5, 3)] {
            request.image_index = index;
            let image = decode_fits(&request).unwrap();
            assert_eq!(
                (image.hdu_index, image.section_index, image.width, image.height),
                (hdu, section, w, h)
            );
            assert_eq!(image.image_count, 3);
        }
        request.image_index = 3;
        assert!(decode_fits(&request).is_err());
        request.path = root.join("fits-u8.fits");
        request.image_index = 0;
        let image = decode_fits(&request).unwrap();
        assert!(
            image
                .cards
                .iter()
                .any(|c| std::str::from_utf8(c).unwrap().contains("CC0 / O''Brien"))
        );
        assert_eq!(
            image.cards.iter().filter(|c| c.starts_with(b"HISTORY ")).count(),
            2
        );
        let FitsSamples::U8(values) = image.samples else {
            panic!()
        };
        assert_eq!(values[0], 0);
    }
    #[test]
    fn structural_corruption_truncation_and_padding_fail() {
        let original = include_bytes!("../../../tests/fixtures/raster/fits-i16.fits");
        let request = DecodeRequest::new("synthetic.fits");
        for end in [0, 79, 2879, 2880, original.len() - 1] {
            assert!(decode(&original[..end], &request).is_err());
        }
        let replace = |keyword: &str, value: &str| {
            let mut b = original.to_vec();
            let at = b[..2880]
                .chunks_exact(80)
                .position(|c| std::str::from_utf8(&c[..8]).unwrap().trim() == keyword)
                .unwrap()
                * 80;
            let card = format!("{keyword:<8}= {value:>20}");
            b[at..at + 80].fill(b' ');
            b[at..at + card.len()].copy_from_slice(card.as_bytes());
            b
        };
        for (key, value) in [
            ("SIMPLE", "F"),
            ("BITPIX", "24"),
            ("NAXIS", "-1"),
            ("NAXIS1", "-1"),
            ("NAXIS1", "65537"),
        ] {
            assert!(decode(&replace(key, value), &request).is_err());
        }
        let mut b = original.to_vec();
        b[80..160].copy_from_slice(&original[160..240]);
        assert!(decode(&b, &request).is_err());
        let mut b = original.to_vec();
        b[2879] = 0;
        assert!(decode(&b, &request).is_err());
        let mut b = original.to_vec();
        let last = b.len() - 1;
        b[last] = 1;
        assert!(decode(&b, &request).is_err());
        let mut b = original.to_vec();
        b.extend_from_slice(&[0; 2880]);
        assert!(decode(&b, &request).is_err());
    }
    #[test]
    fn common_router_and_explicit_window_preserve_selection() {
        let path = std::env::temp_dir().join(format!("rrrah-fits-router-{}.cr3", std::process::id()));
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/fits-i16.fits"),
        )
        .unwrap();
        let mut request = DecodeRequest::new(&path);
        request.image_index = 1;
        let raw = crate::decode_image(&request).unwrap();
        let window =
            crate::decode_raster_with_window(&request, crate::ScalarWindow::new(-32768., 32767.).unwrap())
                .unwrap();
        std::fs::remove_file(path).unwrap();
        let crate::DecodedImage::Raster(raw) = raw else {
            panic!()
        };
        assert_eq!((raw.image_index(), raw.image_count()), (1, 2));
        assert!(crate::prepare_raster_for_display(&raw).is_err());
        assert_eq!(window.color_space(), &RasterColorSpace::LinearSrgb);
        assert_eq!((window.image_index(), window.image_count()), (1, 2));
    }
}
