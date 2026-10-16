//! FITS primary/IMAGE scalar arrays, retaining native values and original cards.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, PixelBuffer, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidFits(s)
}
fn buffer<T: Copy>(
    len: usize,
    value: T,
    request: Option<&DecodeRequest>,
) -> Result<PixelBuffer<T>, RasterDecodeError> {
    Ok(match request.and_then(|r| r.memory_budget.as_ref()) {
        Some(budget) => budget
            .try_buffer(len, value)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => Arc::new(vec![value; len]).into(),
    })
}
pub fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"SIMPLE  =")
}
#[derive(Debug, Clone)]
pub enum FitsSamples {
    U8(PixelBuffer<u8>),
    I16(PixelBuffer<i16>),
    I32(PixelBuffer<i32>),
    I64(PixelBuffer<i64>),
    F32(PixelBuffer<f32>),
    F64(PixelBuffer<f64>),
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
    request: DecodeRequest,
    pub width: u32,
    pub height: u32,
    /// Global plane selection across primary/IMAGE HDUs, skipping tables/empty HDUs.
    pub image_index: usize,
    pub image_count: usize,
    pub hdu_index: usize,
    pub section_index: usize,
    pub cards: rrrah_core::PixelBuffer<[u8; 80]>,
    pub samples: FitsSamples,
    pub bscale: f64,
    pub bzero: f64,
    pub blank: Option<i64>,
}
impl FitsImage {
    /// Reuse native samples with a new output budget and cancellation context.
    pub fn to_raster_with_request(
        &self,
        request: &DecodeRequest,
    ) -> Result<DecodedRaster, RasterDecodeError> {
        self.raster(Some(request))
    }
    /// Window native samples with an explicit display request. Original sample
    /// reservations remain owned by the original budget; only output uses this request.
    pub fn windowed_with_request(
        &self,
        minimum: f64,
        maximum: f64,
        request: &DecodeRequest,
    ) -> Result<DecodedRaster, RasterDecodeError> {
        self.windowed_request(minimum, maximum, Some(request))
    }

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
        let request = request.or(Some(&self.request));
        if let Some(request) = request {
            request.check_cancelled()?;
        }
        let mut values = buffer(self.samples.len() * 4, 0f32, request)?;
        let output = values.get_mut().unwrap();
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
            output[i * 4..i * 4 + 4].copy_from_slice(&[value, value, value, 1.]);
        }
        self.frame(values)
    }
    fn frame(&self, values: PixelBuffer<f32>) -> Result<DecodedRaster, RasterDecodeError> {
        Ok(DecodedRaster::new(
            self.width,
            self.height,
            RasterPixels::Rgba32Float(values),
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
        let request = request.or(Some(&self.request));
        if let Some(request) = request {
            request.check_cancelled()?;
        }
        let range = maximum - minimum;
        if !minimum.is_finite() || !maximum.is_finite() || !range.is_finite() || range <= 0. {
            return Err(bad("window must have finite increasing physical bounds"));
        }
        let mut values = buffer(self.samples.len() * 4, 0f32, request)?;
        let output = values.get_mut().unwrap();
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
            output[i * 4..i * 4 + 4].copy_from_slice(&[gray, gray, gray, 1.]);
        }
        Ok(DecodedRaster::new(
            self.width,
            self.height,
            RasterPixels::Rgba32Float(values),
            RasterColorSpace::LinearSrgb,
        )?
        .with_image_selection(self.image_index, self.image_count)?)
    }
}
struct Fields<'a> {
    entries: rrrah_core::PixelBuffer<(&'a str, &'a str, usize)>,
}
impl<'a> Fields<'a> {
    fn get_key_value(&self, key: &str) -> Option<(&&'a str, &&'a str)> {
        let end = self.entries.partition_point(|entry| entry.0 <= key);
        let entry = self.entries.get(end.checked_sub(1)?)?;
        (entry.0 == key).then_some((&entry.0, &entry.1))
    }
    fn get(&self, key: &str) -> Option<&&'a str> {
        self.get_key_value(key).map(|(_, value)| value)
    }
    fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
    #[cfg(test)]
    fn single(key: &'a str, value: &'a str) -> Self {
        Self {
            entries: Arc::new(vec![(key, value, 0)]).into(),
        }
    }
}
struct Header<'a> {
    cards: &'a [[u8; 80]],
    fields: Fields<'a>,
    end: usize,
}
fn field(card: &[u8; 80]) -> Result<Option<(&str, &str)>, RasterDecodeError> {
    let key = std::str::from_utf8(&card[..8]).unwrap().trim_end();
    if matches!(key, "" | "COMMENT" | "HISTORY") || &card[8..10] != b"= " {
        return Ok(None);
    }
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
    Ok(Some((key, std::str::from_utf8(&card[10..end]).unwrap().trim())))
}
fn structural(key: &str) -> bool {
    matches!(
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
    ) || key.starts_with("NAXIS")
}
fn parse_header<'a>(
    b: &'a [u8],
    start: usize,
    request: Option<&DecodeRequest>,
) -> Result<Header<'a>, RasterDecodeError> {
    let mut at = start;
    let mut count = 0;
    let (cards, end) = loop {
        if let Some(request) = request {
            request.check_cancelled()?;
        }
        if at - start > 1024 * 1024 - 80 {
            return Err(bad("oversized header"));
        }
        let card: &[u8; 80] = b
            .get(at..at + 80)
            .ok_or_else(|| bad("truncated header"))?
            .try_into()
            .unwrap();
        if card.iter().any(|c| !(32..=126).contains(c)) {
            return Err(bad("header cards must be printable ASCII"));
        }
        at += 80;
        if &card[..8] == b"END     " {
            if card[8..].iter().any(|v| *v != b' ') {
                return Err(bad("END card has a value"));
            }
            let end = at.div_ceil(2880) * 2880;
            if b.get(at..end).is_none_or(|v| v.iter().any(|v| *v != b' ')) {
                return Err(bad("invalid header padding"));
            }
            let (cards, remainder) = b[start..at].as_chunks::<80>();
            debug_assert!(remainder.is_empty());
            break (cards, end);
        }
        count += usize::from(field(card)?.is_some());
    };
    let mut entries = buffer(count, ("", "", 0usize), request)?;
    let values = entries.get_mut().unwrap();
    let mut next = 0;
    for (order, card) in cards.iter().enumerate() {
        if let Some(request) = request {
            request.check_cancelled()?;
        }
        if let Some((key, value)) = field(card)? {
            values[next] = (key, value, order);
            next += 1;
        }
    }
    values.sort_unstable_by(|a, b| a.0.cmp(b.0).then(a.2.cmp(&b.2)));
    for pair in values.windows(2) {
        if pair[0].0 == pair[1].0 && structural(pair[0].0) {
            return Err(bad("duplicate structural keyword"));
        }
    }
    Ok(Header {
        cards,
        fields: Fields { entries },
        end,
    })
}
// FITS axis keywords occupy at most the eight-byte keyword field (NAXIS999).
fn axis_keyword(axis: usize, bytes: &mut [u8; 8]) -> &str {
    debug_assert!((1..=999).contains(&axis));
    bytes[..5].copy_from_slice(b"NAXIS");
    let digits = if axis >= 100 {
        3
    } else if axis >= 10 {
        2
    } else {
        1
    };
    let mut value = axis;
    for offset in (5..5 + digits).rev() {
        bytes[offset] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    std::str::from_utf8(&bytes[..5 + digits]).unwrap()
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
    let mut normalized = [0u8; 80];
    let text = if v.contains('D') {
        let bytes = &mut normalized[..v.len()];
        for (out, source) in bytes.iter_mut().zip(v.bytes()) {
            *out = if source == b'D' { b'E' } else { source };
        }
        std::str::from_utf8(bytes).unwrap()
    } else {
        *v
    };
    let value = text.parse::<f64>().map_err(|_| bad("invalid scaling keyword"))?;
    if !value.is_finite() {
        return Err(bad("nonfinite scaling"));
    }
    Ok(value)
}
pub fn decode_fits(request: &DecodeRequest) -> Result<FitsImage, RasterDecodeError> {
    request.check_cancelled()?;
    decode(&read_bounded(request)?, request)
}
pub fn decode_raster(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    decode(b, request)?.raster(Some(request))
}
pub fn decode(b: &[u8], request: &DecodeRequest) -> Result<FitsImage, RasterDecodeError> {
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
        let header = parse_header(b, at, Some(request))?;
        let key = |i: usize| {
            header
                .cards
                .get(i)
                .map(|c| std::str::from_utf8(&c[..8]).unwrap().trim_end())
        };
        let image = if hdu == 0 {
            if key(0) != Some("SIMPLE") || header.fields.get("SIMPLE").copied() != Some("T") {
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
            if header.fields.get("ZIMAGE").is_some_and(|v| *v == "T") {
                return Err(bad("compressed image convention unsupported"));
            }
            typ == "IMAGE"
        };
        if key(1) != Some("BITPIX") || key(2) != Some("NAXIS") {
            return Err(bad("required keyword order"));
        }
        if header.fields.get("GROUPS").is_some_and(|v| *v == "T") {
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
        // Only displayed 2D/3D images need retained axis lengths. Still validate
        // every declared axis, including empty higher-dimensional HDUs.
        let mut sizes = [0usize; 3];
        let mut axis_bytes = [0u8; 8];
        let mut count = if dimension == 0 { 0usize } else { 1usize };
        for axis in 1..=dimension {
            let name = axis_keyword(axis as usize, &mut axis_bytes);
            if key(axis as usize + 2) != Some(name) {
                return Err(bad("axis keyword order"));
            }
            let size = usize::try_from(number(&header, name)?).map_err(|_| bad("negative axis size"))?;
            count = count.checked_mul(size).ok_or(RasterDecodeError::OutputTooLarge)?;
            if axis <= 3 {
                sizes[axis as usize - 1] = size;
            }
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
                drop(header.fields);
                macro_rules! unpack {
                    ($t:ty,$variant:ident) => {{
                        let mut values = buffer(plane, 0 as $t, Some(request))?;
                        let output = values.get_mut().unwrap();
                        for (i, v) in data.chunks_exact(bytes).enumerate() {
                            if i % 4096 == 0 {
                                request.check_cancelled()?;
                            }
                            output[i] = <$t>::from_be_bytes(v.try_into().unwrap());
                        }
                        FitsSamples::$variant(values)
                    }};
                }
                let samples = match bitpix {
                    8 => {
                        let mut values = buffer(plane, 0u8, Some(request))?;
                        for (dest, source) in values
                            .get_mut()
                            .unwrap()
                            .chunks_mut(65536)
                            .zip(data.chunks(65536))
                        {
                            request.check_cancelled()?;
                            dest.copy_from_slice(source);
                        }
                        FitsSamples::U8(values)
                    }
                    16 => unpack!(i16, I16),
                    32 => unpack!(i32, I32),
                    64 => unpack!(i64, I64),
                    -32 => unpack!(f32, F32),
                    _ => unpack!(f64, F64),
                };
                let mut cards = buffer(header.cards.len(), [0u8; 80], Some(request))?;
                cards.get_mut().unwrap().copy_from_slice(header.cards);
                selected = Some(FitsImage {
                    request: request.clone(),
                    width: sizes[0] as u32,
                    height: sizes[1] as u32,
                    image_index: request.image_index,
                    image_count: 0,
                    hdu_index: hdu,
                    section_index: section,
                    cards,
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
            request: DecodeRequest::new("synthetic.fits"),
            width: 1,
            height: 1,
            image_index: 0,
            image_count: 1,
            hdu_index: 0,
            section_index: 0,
            cards: Arc::new(Vec::new()).into(),
            samples: FitsSamples::I64(Arc::new(vec![i64::MAX]).into()),
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
    fn stack_axis_keywords_and_empty_maximum_dimension_validate_all_axes() {
        let mut bytes = [0u8; 8];
        for axis in (1..=999).rev() {
            assert_eq!(axis_keyword(axis, &mut bytes), format!("NAXIS{axis}"));
        }
        let mut source = Vec::new();
        let mut card = |text: String| {
            let mut bytes = [b' '; 80];
            bytes[..text.len()].copy_from_slice(text.as_bytes());
            source.extend(bytes);
        };
        card("SIMPLE  = T".into());
        card("BITPIX  = 8".into());
        card("NAXIS   = 999".into());
        for axis in 1..=999 {
            card(format!(
                "{:<8}= {}",
                format!("NAXIS{axis}"),
                usize::from(axis != 1)
            ));
        }
        card("END".into());
        source.resize(source.len().next_multiple_of(2880), b' ');
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new("empty-999.fits");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::Source(
                crate::DecodeError::UnsupportedImageIndex { index: 0 }
            ))
        ));
        assert_eq!(budget.used(), 0);
        let at = (3 + 998) * 80;
        source[at + 10..at + 12].copy_from_slice(b"-1");
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::InvalidFits("negative axis size"))
        ));
        assert_eq!(budget.used(), 0);
        source[at..at + 8].copy_from_slice(b"NAXIS998");
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::InvalidFits("duplicate structural keyword"))
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn fortran_scaling_uses_bounded_normalization_and_preserves_values() {
        for (text, expected) in [
            ("1.25D+2", 125f64),
            ("-2.5D-3", -0.0025),
            ("0D0", 0.),
            ("-0D0", -0.),
        ] {
            let header = Header {
                cards: &[],
                fields: Fields::single("BSCALE", text),
                end: 0,
            };
            assert_eq!(
                scaling(&header, "BSCALE", 1.).unwrap().to_bits(),
                expected.to_bits()
            );
        }
        let text = format!("0.{}1D0", "0".repeat(65));
        assert_eq!(text.len(), 70);
        let header = Header {
            cards: &[],
            fields: Fields::single("BSCALE", text.as_str()),
            end: 0,
        };
        assert_eq!(
            scaling(&header, "BSCALE", 1.).unwrap().to_bits(),
            1e-66f64.to_bits()
        );
    }
    #[test]
    fn sorted_fields_admit_exact_storage_and_preserve_last_nonstructural_value() {
        let make = |duplicate: &str| {
            let mut bytes = Vec::new();
            for text in ["ZETA    = 1", "ALPHA   = 2", "ZETA    = 3", duplicate, "END"] {
                let mut card = [b' '; 80];
                card[..text.len()].copy_from_slice(text.as_bytes());
                bytes.extend(card);
            }
            bytes.resize(2880, b' ');
            bytes
        };
        let bytes = make("BITPIX  = 8");
        let expected = (4 * std::mem::size_of::<(&str, &str, usize)>()) as u64;
        let budget = rrrah_core::MemoryBudget::new(expected);
        let mut request = DecodeRequest::new("fields.fits");
        request.memory_budget = Some(budget.clone());
        let header = parse_header(&bytes, 0, Some(&request)).unwrap();
        assert_eq!((budget.used(), budget.peak()), (expected, expected));
        assert_eq!(header.fields.get("ZETA").copied(), Some("3"));
        assert_eq!(header.fields.get("ALPHA").copied(), Some("2"));
        for missing in ["", "A", "ALPHAA", "ZZZZ"] {
            assert!(header.fields.get(missing).is_none());
        }
        drop(header);
        assert_eq!(budget.used(), 0);
        let short = rrrah_core::MemoryBudget::new(expected - 1);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            parse_header(&bytes, 0, Some(&request)),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!((short.used(), short.peak()), (0, 0));
        let mut duplicate = make("BITPIX  = 8");
        duplicate[0..8].copy_from_slice(b"BITPIX  ");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            parse_header(&duplicate, 0, Some(&request)),
            Err(RasterDecodeError::InvalidFits("duplicate structural keyword"))
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn parsed_header_keys_and_values_borrow_original_cards() {
        let source = include_bytes!("../../../tests/fixtures/raster/fits-u8.fits");
        let header = parse_header(source, 0, None).unwrap();
        let (key, value) = header.fields.get_key_value("SIMPLE").unwrap();
        assert_eq!(key.as_ptr(), source.as_ptr());
        let expected = std::str::from_utf8(&source[10..30]).unwrap().trim();
        assert_eq!(*value, "T");
        assert_eq!(value.as_ptr(), expected.as_ptr());
    }
    #[test]
    fn end_card_must_fit_entirely_inside_header_limit() {
        let original = include_bytes!("../../../tests/fixtures/raster/fits-u8.fits");
        let header = parse_header(original, 0, None).unwrap();
        let make = |count: usize| {
            let mut bytes = original[..(header.cards.len() - 1) * 80].to_vec();
            let mut comment = [b' '; 80];
            comment[..8].copy_from_slice(b"COMMENT ");
            while bytes.len() / 80 + 1 < count {
                bytes.extend(comment);
            }
            bytes.extend(header.cards.last().unwrap());
            bytes.resize(bytes.len().next_multiple_of(2880), b' ');
            bytes.extend(&original[header.end..]);
            bytes
        };
        let maximum = 1024 * 1024 / 80;
        let valid = make(maximum);
        let budget = rrrah_core::MemoryBudget::new((maximum * 80 + 15) as u64);
        let mut request = DecodeRequest::new("limit.fits");
        request.memory_budget = Some(budget.clone());
        let image = decode(&valid, &request).unwrap();
        assert_eq!(image.cards.len(), maximum);
        drop(image);
        assert_eq!(budget.used(), 0);
        let oversized = make(maximum + 1);
        let zero = rrrah_core::MemoryBudget::new(0);
        request.memory_budget = Some(zero.clone());
        assert!(matches!(
            decode(&oversized, &request),
            Err(RasterDecodeError::InvalidFits("oversized header"))
        ));
        assert_eq!((zero.used(), zero.peak()), (0, 0));
    }
    #[test]
    fn header_cards_borrow_source_then_retain_shared_managed_credit() {
        let source = include_bytes!("../../../tests/fixtures/raster/fits-u8.fits");
        let header = parse_header(source, 0, None).unwrap();
        assert_eq!(header.cards.as_ptr().cast::<u8>(), source.as_ptr());
        let bytes = (header.cards.len() * 80) as u64;
        let budget = rrrah_core::MemoryBudget::new(bytes + 15);
        let mut request = DecodeRequest::new("cards.fits");
        request.memory_budget = Some(budget.clone());
        let image = decode(source, &request).unwrap();
        let cards = image.cards.clone();
        drop(image);
        assert_eq!(budget.used(), bytes);
        assert_eq!(&cards[0][..8], b"SIMPLE  ");
        assert_eq!(&cards.last().unwrap()[..8], b"END     ");
        let shared = cards.clone();
        drop(cards);
        assert_eq!(budget.used(), bytes);
        drop(shared);
        assert_eq!(budget.used(), 0);
        let short = rrrah_core::MemoryBudget::new(bytes + 14);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
    }
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
            let source_bytes = std::fs::metadata(&request.path).unwrap().len();
            let native_bytes = std::fs::metadata(root.join(c[5])).unwrap().len();
            let card_bytes = (image.cards.len() * 80) as u64;
            // Later HDUs are validated while the selected samples/cards remain live.
            let probe = rrrah_core::MemoryBudget::new(source_bytes + native_bytes + card_bytes + 1024 * 1024);
            request.memory_budget = Some(probe.clone());
            let probed = decode_fits(&request).unwrap();
            let native_peak = probe.peak();
            assert!(native_peak >= source_bytes + native_bytes + card_bytes);
            drop(probed);
            assert_eq!(probe.used(), 0);
            let budget = rrrah_core::MemoryBudget::new(native_peak);
            request.memory_budget = Some(budget.clone());
            let admitted = decode_fits(&request).unwrap();
            assert_eq!(
                (budget.used(), budget.peak()),
                (native_bytes + card_bytes, native_peak)
            );
            let shared = admitted.samples.clone();
            drop(admitted);
            assert_eq!(budget.used(), native_bytes);
            drop(shared);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(native_peak - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode_fits(&request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
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
            let output_bytes = u64::from(image.width) * u64::from(image.height) * 16;
            let exact = native_peak.max(card_bytes + native_bytes + output_bytes);
            let managed = rrrah_core::MemoryBudget::new(exact);
            request.memory_budget = Some(managed.clone());
            let window = crate::ScalarWindow::new(-100., 1000.).unwrap();
            let displayed = crate::decode_raster_with_window(&request, window).unwrap();
            assert_eq!((managed.used(), managed.peak()), (output_bytes, exact));
            drop(displayed);
            assert_eq!(managed.used(), 0);
            let short = rrrah_core::MemoryBudget::new(exact - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                crate::decode_raster_with_window(&request, window),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
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
        let bytes = include_bytes!("../../../tests/fixtures/raster/fits-i16.fits");
        std::fs::write(&path, bytes).unwrap();
        let mut request = DecodeRequest::new(&path);
        request.image_index = 1;
        let raw = super::decode_raster(bytes, &request).unwrap();
        let window =
            crate::decode_raster_with_window(&request, crate::ScalarWindow::new(-32768., 32767.).unwrap())
                .unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!((raw.image_index(), raw.image_count()), (1, 2));
        assert_eq!(raw.color_space(), &RasterColorSpace::Unspecified);
        assert_eq!(window.color_space(), &RasterColorSpace::LinearSrgb);
        assert_eq!((window.image_index(), window.image_count()), (1, 2));
    }
}
