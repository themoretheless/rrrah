//! Native single-root DICOM Part 10, Implicit VR LE and Explicit VR LE/BE pixels.
//! Integer 8/16-bit and finite IEEE float32/float64; explicit scalar display window.
//! Unsigned full-width RGB8/16 preserves planar layout, color declarations and ICC.
//! No guessed color, VOI LUT, presentation-state or compression fallback.
use crate::{DecodeRequest, RasterDecodeError, ScalarWindow};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{collections::BTreeSet, sync::Arc};
#[path = "dicom_encapsulation.rs"]
mod encapsulation;
#[path = "dicom_geometry.rs"]
mod geometry;
#[path = "dicom_implicit_tags.rs"]
mod implicit_tags;
#[path = "dicom_palette.rs"]
mod palette;
#[path = "dicom_rgb.rs"]
mod rgb;
#[path = "dicom_sequences.rs"]
mod sequences;
// Only fields interpreted by the scalar decoder get a concrete VR here.
// Other recognized scalar metadata/private values are skipped as opaque bytes.
fn implicit_vr(tag: u32) -> Option<&'static [u8]> {
    Some(match tag {
        0x00280010 | 0x00280011 | 0x00280002 | 0x00280100 | 0x00280101 | 0x00280102 | 0x00280103
        | 0x00280006 => b"US",
        0x00281052 | 0x00281053 | 0x00280030 | 0x00181164 | 0x00182010 => b"DS",
        0x00280004 | 0x00282002 => b"CS",
        0x00282000 => b"OB",
        0x00280008 | 0x00280034 => b"IS",
        0x7fe00010 | 0x00281201..=0x00281203 | 0x00281221..=0x00281223 => b"OW",
        0x7fe00008 => b"OF",
        0x7fe00009 => b"OD",
        _ if tag >> 16 == 0xfffe => b"SQ",
        _ if (tag >> 16) & 1 != 0 => b"UN",
        _ if implicit_tags::SEQUENCES.binary_search(&tag).is_ok() => b"SQ",
        _ if implicit_tags::KNOWN.binary_search(&tag).is_ok() || tag & 0xffff == 0 => b"UN",
        _ => {
            let &(_, _, sequence) = implicit_tags::REPEATERS
                .iter()
                .find(|&&(mask, value, _)| tag & mask == value)?;
            if sequence { b"SQ" } else { b"UN" }
        }
    })
}
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidDicom(s)
}
pub fn has_magic(b: &[u8]) -> bool {
    b.get(128..132) == Some(b"DICM")
}
fn text(b: &[u8]) -> Result<&str, RasterDecodeError> {
    std::str::from_utf8(b)
        .map(|s| s.trim_end_matches([' ', '\0']))
        .map_err(|_| bad("invalid text"))
}
fn word(bytes: [u8; 2], little: bool) -> u16 {
    if little {
        u16::from_le_bytes(bytes)
    } else {
        u16::from_be_bytes(bytes)
    }
}
fn dword(bytes: [u8; 4], little: bool) -> u32 {
    if little {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    }
}
fn us(vr: &[u8], b: &[u8], little: bool) -> Result<u16, RasterDecodeError> {
    if vr != b"US" || b.len() != 2 {
        return Err(bad("invalid US element"));
    }
    Ok(word(b.try_into().unwrap(), little))
}
fn ds(vr: &[u8], b: &[u8]) -> Result<f64, RasterDecodeError> {
    if vr != b"DS" || b.len() > 16 {
        return Err(bad("invalid DS element"));
    }
    let x = text(b)?
        .trim()
        .parse::<f64>()
        .map_err(|_| bad("invalid decimal"))?;
    if !x.is_finite() {
        return Err(bad("nonfinite decimal"));
    }
    Ok(x)
}
pub fn decode(
    b: &[u8],
    request: &DecodeRequest,
    window: Option<ScalarWindow>,
) -> Result<DecodedRaster, RasterDecodeError> {
    if !has_magic(b) {
        return Err(bad("Part 10 preamble required"));
    }
    request.check_cancelled()?;
    let (mut at, mut count) = (132usize, 0usize);
    let mut seen = BTreeSet::new();
    let mut previous = 0u32;
    let mut meta_end = None;
    let mut dataset_started = false;
    let (mut rows, mut columns, mut samples, mut bits, mut stored, mut high, mut signed) =
        (None, None, None, None, None, None, None);
    let (mut photo, mut syntax, mut pixels) = (None, None, None);
    let mut encapsulated = None;
    let (mut slope, mut intercept) = (None, None);
    let mut frames = 1usize;
    let mut geometry = geometry::Geometry::default();
    let mut palette = palette::Palette::default();
    let (mut planar, mut icc, mut color_label) = (None, None, None);
    let (mut padding, mut padding_limit) = (None, None);
    while at < b.len() {
        count += 1;
        if count > 65536 {
            return Err(bad("element count limit"));
        }
        if count % 32 == 0 {
            request.check_cancelled()?;
        }
        let element_start = at;
        let h = b.get(at..at + 8).ok_or_else(|| bad("truncated element"))?;
        // Part10 file meta is always explicit LE, including for a BE dataset.
        // Its checked group-length boundary determines where byte order changes.
        let little = syntax != Some("1.2.840.10008.1.2.2") || meta_end.is_none_or(|end| element_start < end);
        let group = word(h[..2].try_into().unwrap(), little);
        let tag = (u32::from(group) << 16) | u32::from(word(h[2..4].try_into().unwrap(), little));
        if tag < previous || !seen.insert(tag) {
            return Err(bad("unordered or duplicate tag"));
        }
        previous = tag;
        if group != 2 && !dataset_started {
            if meta_end != Some(element_start)
                || ![0x00020001, 0x00020002, 0x00020003, 0x00020012]
                    .iter()
                    .all(|tag| seen.contains(tag))
            {
                return Err(bad("incomplete/inconsistent Part 10 file meta information"));
            }
            dataset_started = true;
        }
        if group != 2
            && !matches!(
                syntax,
                Some(
                    "1.2.840.10008.1.2"
                        | "1.2.840.10008.1.2.1"
                        | "1.2.840.10008.1.2.2"
                        | "1.2.840.10008.1.2.5"
                )
            )
        {
            return Err(bad("unsupported native DICOM transfer syntax"));
        }
        let implicit = group != 2 && syntax == Some("1.2.840.10008.1.2");
        let vr = if implicit {
            implicit_vr(tag).ok_or_else(|| bad("unknown public implicit VR tag"))?
        } else {
            &h[4..6]
        };
        let long = matches!(
            vr,
            b"OB" | b"OD" | b"OF" | b"OL" | b"OV" | b"OW" | b"SQ" | b"UC" | b"UR" | b"UT" | b"UN"
        );
        let length = if implicit {
            let n = dword(h[4..8].try_into().unwrap(), true);
            at += 8;
            n as usize
        } else if long {
            if h[6..8] != [0, 0] {
                return Err(bad("reserved VR bytes"));
            }
            let n = dword(
                b.get(at + 8..at + 12)
                    .ok_or_else(|| bad("truncated length"))?
                    .try_into()
                    .unwrap(),
                little,
            );
            at += 12;
            n as usize
        } else {
            if !matches!(
                vr,
                b"AE"
                    | b"AS"
                    | b"AT"
                    | b"CS"
                    | b"DA"
                    | b"DS"
                    | b"DT"
                    | b"FD"
                    | b"FL"
                    | b"IS"
                    | b"LO"
                    | b"LT"
                    | b"PN"
                    | b"SH"
                    | b"SL"
                    | b"SS"
                    | b"ST"
                    | b"TM"
                    | b"UI"
                    | b"UL"
                    | b"US"
            ) {
                return Err(bad("unsupported VR"));
            }
            let n = word(h[6..8].try_into().unwrap(), little);
            at += 8;
            usize::from(n)
        };
        if vr == b"SQ" && group != 2 && sequences::ignorable(tag) {
            sequences::skip(b, &mut at, length as u32, (little, implicit), &mut count, request)?;
            continue;
        }
        if tag == 0x7fe00010 && syntax == Some("1.2.840.10008.1.2.5") {
            // Some established 16-bit producers use OW for encapsulated RLE.
            // The fragment payload remains an opaque byte stream; never swap it.
            if !matches!(vr, b"OB" | b"OW")
                || length != u32::MAX as usize
                || pixels.is_some()
                || encapsulated.is_some()
            {
                return Err(bad("RLE pixels require encapsulated OB or OW"));
            }
            encapsulated = Some(encapsulation::select(
                b,
                &mut at,
                frames,
                request.image_index,
                request,
            )?);
            continue;
        }
        if length == u32::MAX as usize || vr == b"SQ" {
            return Err(bad("sequences/encapsulated data are not qualified"));
        }
        if length % 2 != 0 {
            return Err(bad("odd element length"));
        }
        let end = at.checked_add(length).ok_or_else(|| bad("element overflow"))?;
        let v = b.get(at..end).ok_or_else(|| bad("truncated value"))?;
        at = end;
        match tag {
            0x00281101..=0x00281104 | 0x00281201..=0x00281204 | 0x00281221..=0x00281224 => {
                palette.record(tag, vr, v, implicit)?
            }
            0x00280030 | 0x00181164 | 0x00182010 | 0x00280034 => geometry.record(tag, vr, v)?,
            0x00020000 => {
                if count != 1 || vr != b"UL" || length != 4 {
                    return Err(bad("file meta group length"));
                }
                let size = u32::from_le_bytes(v.try_into().unwrap()) as usize;
                meta_end = Some(
                    at.checked_add(size)
                        .filter(|end| *end <= b.len())
                        .ok_or_else(|| bad("file meta range"))?,
                );
            }
            0x00020001 => {
                if vr != b"OB" || v != [0, 1] {
                    return Err(bad("file meta version"));
                }
            }
            0x00020002 | 0x00020003 | 0x00020012 => {
                if vr != b"UI"
                    || length > 64
                    || text(v)?.is_empty()
                    || !text(v)?.bytes().all(|c| c.is_ascii_digit() || c == b'.')
                {
                    return Err(bad("file meta UID"));
                }
            }
            0x00020010 => {
                if vr != b"UI" || length > 64 {
                    return Err(bad("invalid transfer syntax"));
                }
                syntax = Some(text(v)?);
            }
            0x00280010 => rows = Some(us(vr, v, little)?),
            0x00280011 => columns = Some(us(vr, v, little)?),
            0x00280002 => samples = Some(us(vr, v, little)?),
            0x00280006 => planar = Some(us(vr, v, little)?),
            0x00282000 => {
                if vr != b"OB" {
                    return Err(bad("ICC profile VR"));
                }
                icc = Some(v);
            }
            0x00282002 => {
                if vr != b"CS" || v.len() > 16 || text(v)?.is_empty() {
                    return Err(bad("color label VR/value"));
                }
                color_label = Some(text(v)?);
            }
            0x00280100 => bits = Some(us(vr, v, little)?),
            0x00280101 => stored = Some(us(vr, v, little)?),
            0x00280102 => high = Some(us(vr, v, little)?),
            0x00280103 => signed = Some(us(vr, v, little)?),
            0x00280004 => {
                if vr != b"CS" || length > 16 {
                    return Err(bad("photometric VR"));
                }
                photo = Some(text(v)?);
            }
            0x00280008 => {
                if vr != b"IS" || length > 12 {
                    return Err(bad("frame count VR"));
                }
                frames = text(v)?.trim().parse().map_err(|_| bad("frame count"))?;
            }
            0x00281052 => intercept = Some(ds(vr, v)?),
            0x00281053 => slope = Some(ds(vr, v)?),
            0x00280120 | 0x00280121 => {
                let value = (if implicit { None } else { Some(vr) }, v);
                if tag == 0x00280120 {
                    padding = Some(value);
                } else {
                    padding_limit = Some(value);
                }
            }
            0x00280122..=0x00280125 => return Err(bad("pixel padding needs explicit missing-value policy")),
            0x7fe00010 => {
                if !matches!(vr, b"OB" | b"OW") {
                    return Err(bad("pixel VR"));
                }
                if pixels.replace((v, vr)).is_some() {
                    return Err(bad("multiple pixel data elements"));
                }
            }
            0x7fe00008 | 0x7fe00009 => {
                let required = if tag == 0x7fe00008 { b"OF" } else { b"OD" };
                if vr != required {
                    return Err(bad("floating pixel VR"));
                }
                if pixels.replace((v, vr)).is_some() {
                    return Err(bad("multiple pixel data elements"));
                }
            }
            _ => {}
        }
    }
    let width = u32::from(columns.ok_or_else(|| bad("missing columns"))?);
    let height = u32::from(rows.ok_or_else(|| bad("missing rows"))?);
    let bits = bits.ok_or_else(|| bad("missing bits allocated"))?;
    if syntax == Some("1.2.840.10008.1.2.5") && encapsulated.is_none() {
        return Err(bad("missing encapsulated RLE pixels"));
    }
    let (stored_pixels, pixel_vr) = if let Some((pixels, vr)) = pixels {
        (Some(pixels), vr)
    } else if encapsulated.is_some() {
        (None, b"OW".as_slice())
    } else {
        return Err(bad("missing pixel data"));
    };
    let floating = matches!(pixel_vr, b"OF" | b"OD");
    let (stored, signed) = if floating {
        if bits != if pixel_vr == b"OF" { 32 } else { 64 }
            || stored.is_some()
            || high.is_some()
            || signed.is_some()
        {
            return Err(bad("floating sample bit layout"));
        }
        (0, 0)
    } else {
        let stored = stored.ok_or_else(|| bad("missing bits stored"))?;
        let high = high.ok_or_else(|| bad("missing high bit"))?;
        let signed = signed.ok_or_else(|| bad("missing pixel representation"))?;
        if !matches!(bits, 8 | 16) || stored == 0 || stored > bits || high != stored - 1 || signed > 1 {
            return Err(bad("sample bit layout"));
        }
        (stored, signed)
    };
    let photo = photo.ok_or_else(|| bad("missing photometric interpretation"))?;
    if width == 0 || height == 0 || frames == 0 || frames > 65536 || request.image_index >= frames {
        return Err(bad("dimensions/frame selection"));
    }
    let pixel_aspect = geometry.aspect(width, height)?;
    let pixel_frames = if encapsulated.is_some() { 1 } else { frames };
    let pixel_index = if encapsulated.is_some() {
        0
    } else {
        request.image_index
    };
    let decoded_rle;
    let pixels = if let Some(frame) = encapsulated {
        let components = samples.ok_or_else(|| bad("missing RLE samples"))?;
        if photo == "PALETTE COLOR" && signed != 0 {
            return Err(bad("RLE palette indices must be unsigned"));
        }
        let output_bytes = match photo {
            "MONOCHROME1" | "MONOCHROME2" if components == 1 => 16,
            "RGB" | "YBR_FULL" if components == 3 => {
                if bits == 8 {
                    4
                } else {
                    8
                }
            }
            "PALETTE COLOR" if components == 1 => palette.output_bytes(true)?,
            _ => return Err(bad("unsupported RLE photometric layout")),
        };
        if u64::from(width) * u64::from(height) * output_bytes > crate::raster::MAX_RASTER_BYTES {
            return Err(RasterDecodeError::OutputTooLarge);
        }
        decoded_rle = crate::dicom_rle::decode_frame_padded(
            frame,
            crate::dicom_rle::RleFrameLayout {
                width,
                height,
                samples: components,
                bits,
                planar: planar == Some(1),
            },
            request,
        )?;
        decoded_rle.as_slice()
    } else {
        stored_pixels.unwrap()
    };
    if photo == "PALETTE COLOR" {
        if samples != Some(1)
            || floating
            || window.is_some()
            || slope.is_some()
            || intercept.is_some()
            || padding.is_some()
            || padding_limit.is_some()
            || planar.is_some()
        {
            return Err(bad("unsupported indexed palette layout/transform"));
        }
        if bits == 16 && pixel_vr != b"OW" {
            return Err(bad("word pixels require OW VR"));
        }
        return palette.decode(
            palette::Input {
                width,
                height,
                frames,
                pixel_frames,
                pixel_index,
                bits,
                stored,
                signed,
                pixels,
                little: syntax != Some("1.2.840.10008.1.2.2"),
                word_vr: pixel_vr == b"OW",
                icc,
                color_label,
                aspect: pixel_aspect,
            },
            request,
        );
    }
    if palette.present() {
        return Err(bad("supplemental palette requires qualified interpretation"));
    }
    if matches!(photo, "RGB" | "YBR_FULL" | "YBR_FULL_422") {
        if samples != Some(3)
            || floating
            || signed != 0
            || stored != bits
            || !matches!(planar, Some(0 | 1))
            || window.is_some()
            || slope.is_some()
            || intercept.is_some()
            || padding.is_some()
            || padding_limit.is_some()
        {
            return Err(bad("unsupported RGB layout/transform"));
        }
        let encoding = match photo {
            "RGB" => rgb::Encoding::Rgb,
            "YBR_FULL" => rgb::Encoding::YbrFull,
            _ => rgb::Encoding::Ybr422,
        };
        if encoding != rgb::Encoding::Rgb && bits != 8 {
            return Err(bad("YBR requires unsigned full-width 8-bit samples"));
        }
        if encoding == rgb::Encoding::Ybr422 && (planar != Some(0) || width % 2 != 0) {
            return Err(bad("native YBR422 requires interleaved even-width rows"));
        }
        if bits == 16 && pixel_vr != b"OW" {
            return Err(bad("word pixels require OW VR"));
        }
        return Ok(rgb::Rgb {
            encoding,
            width,
            height,
            frames,
            pixel_frames,
            pixel_index,
            bits,
            planar: planar.unwrap(),
            pixels,
            word_vr: pixel_vr == b"OW",
            little: syntax != Some("1.2.840.10008.1.2.2"),
            icc,
            color_label,
        }
        .decode(request)?
        .with_pixel_aspect(pixel_aspect)?);
    }
    if icc.is_some() || color_label.is_some() {
        return Err(bad("scalar color profile requires qualified interpretation"));
    }
    if samples != Some(1) || !matches!(photo, "MONOCHROME1" | "MONOCHROME2") {
        return Err(bad("non-scalar photometric interpretation"));
    }
    if floating && photo != "MONOCHROME2" {
        return Err(bad("floating pixels require MONOCHROME2"));
    }
    if floating && (padding.is_some() || padding_limit.is_some()) {
        return Err(bad("integer padding on floating pixels"));
    }
    let padding = if let Some(value) = padding {
        let parse = |(vr, bytes): (Option<&[u8]>, &[u8])| -> Result<i64, RasterDecodeError> {
            let expected = if signed == 1 { b"SS" } else { b"US" };
            if bytes.len() != 2 || vr.is_some_and(|vr| vr != expected) {
                return Err(bad("pixel padding VR/value"));
            }
            let word = word(bytes.try_into().unwrap(), syntax != Some("1.2.840.10008.1.2.2"));
            let value = if signed == 1 {
                i64::from(word as i16)
            } else {
                i64::from(word)
            };
            let (minimum, maximum) = if signed == 1 {
                (-(1i64 << (stored - 1)), (1i64 << (stored - 1)) - 1)
            } else {
                (0, (1i64 << stored) - 1)
            };
            if value < minimum || value > maximum {
                return Err(bad("pixel padding outside stored range"));
            }
            Ok(value)
        };
        let first = parse(value)?;
        let last = padding_limit.map(parse).transpose()?.unwrap_or(first);
        Some((first.min(last), first.max(last)))
    } else {
        if padding_limit.is_some() {
            return Err(bad("padding range without padding value"));
        }
        None
    };
    if slope.is_some() != intercept.is_some() {
        return Err(bad("incomplete rescale declaration"));
    }
    let (slope, intercept) = (slope.unwrap_or(1.), intercept.unwrap_or(0.));
    if slope == 0. {
        return Err(bad("zero rescale slope"));
    }
    let count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| bad("dimensions overflow"))?;
    let bytes = count
        .checked_mul(usize::from(bits / 8))
        .and_then(|n| n.checked_mul(pixel_frames))
        .ok_or_else(|| bad("pixel length overflow"))?;
    if !floating && bits > 8 && pixel_vr != b"OW" {
        return Err(bad("word pixels require OW VR"));
    }
    let pixel_little = syntax != Some("1.2.840.10008.1.2.2");
    if pixels.len()
        != bytes
            .checked_add(bytes % 2)
            .ok_or_else(|| bad("padded pixel length overflow"))?
    {
        return Err(bad("pixel data length"));
    }
    if (count as u64) * 16 > crate::raster::MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut output = match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(count * 4, 0f32)
            .map(|b| b.freeze().into())
            .map_err(crate::DecodeError::Memory)?,
        None => rrrah_core::PixelBuffer::from(Arc::new(vec![0f32; count * 4])),
    };
    let out = output.get_mut().expect("exclusive scalar output");
    let mask = (1u32 << stored) - 1;
    let first = pixel_index * count * usize::from(bits / 8);
    for (i, p) in out.chunks_exact_mut(4).enumerate() {
        if i % 4096 == 0 {
            request.check_cancelled()?;
        }
        let at = first + i * usize::from(bits / 8);
        let sample = if floating {
            if bits == 32 {
                f64::from(f32::from_bits(dword(
                    pixels[at..at + 4].try_into().unwrap(),
                    pixel_little,
                )))
            } else {
                let bytes = pixels[at..at + 8].try_into().unwrap();
                f64::from_bits(if pixel_little {
                    u64::from_le_bytes(bytes)
                } else {
                    u64::from_be_bytes(bytes)
                })
            }
        } else {
            let code = if bits == 8 {
                // OW swaps pairs globally, including across odd-size frame boundaries;
                // OB is byte ordered in both transfer syntaxes.
                u32::from(
                    pixels[if !pixel_little && pixel_vr == b"OW" {
                        at ^ 1
                    } else {
                        at
                    }],
                )
            } else {
                u32::from(word(pixels[at..at + 2].try_into().unwrap(), pixel_little))
            } & mask;
            (if signed == 1 && code & (1 << (stored - 1)) != 0 {
                i64::from(code) - (1i64 << stored)
            } else {
                i64::from(code)
            }) as f64
        };
        if !sample.is_finite() {
            return Err(bad(
                "nonfinite floating pixel needs explicit missing-value policy",
            ));
        }
        if padding.is_some_and(|(lo, hi)| sample >= lo as f64 && sample <= hi as f64) {
            return Err(bad("pixel padding needs explicit missing-value policy"));
        }
        let physical = if slope == 1. && intercept == 0. {
            sample
        } else {
            sample * slope + intercept
        };
        if !physical.is_finite() || (window.is_none() && physical.abs() > f64::from(f32::MAX)) {
            return Err(bad("rescale overflow"));
        }
        let value = if let Some(w) = window {
            let normalized =
                ((physical - f64::from(w.minimum())) / f64::from(w.maximum() - w.minimum())).clamp(0., 1.);
            (if photo == "MONOCHROME1" {
                1. - normalized
            } else {
                normalized
            }) as f32
        } else {
            physical as f32
        };
        p.copy_from_slice(&[value, value, value, 1.]);
    }
    request.check_cancelled()?;
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba32Float(output),
        if window.is_some() {
            RasterColorSpace::LinearSrgb
        } else {
            RasterColorSpace::Unspecified
        },
    )?
    .with_image_selection(request.image_index, frames)?
    .with_pixel_aspect(pixel_aspect)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
    }
    fn oracle(path: &std::path::Path) -> Vec<u32> {
        std::fs::read(path)
            .unwrap()
            .chunks_exact(4)
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
            .collect()
    }
    fn assert_oracle(frame: &DecodedRaster, expected: Vec<u32>) {
        let RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!()
        };
        assert_eq!(values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
    }
    #[test]
    fn pydicom_scalar_rescale_window_and_managed_frame_oracles() {
        let decode_file = |req: &DecodeRequest| -> Result<DecodedRaster, RasterDecodeError> {
            let bytes = crate::bounded_io::read_bounded(req)?;
            super::decode(&bytes, req, None)
        };
        for line in include_str!("../../../tests/fixtures/raster/dicom-manifest.tsv")
            .lines()
            .skip(1)
            .chain(
                include_str!("../../../tests/fixtures/raster/dicom-geometry-scalar-manifest.tsv")
                    .lines()
                    .skip(1),
            )
            .chain(
                include_str!("../../../tests/fixtures/raster/dicom-rle-scalar-manifest.tsv")
                    .lines()
                    .skip(1),
            )
        {
            let fields: Vec<_> = line.split('\t').collect();
            let path = root().join(fields[0]);
            let width = fields[1].parse::<u32>().unwrap();
            let height = fields[2].parse::<u32>().unwrap();
            let frames = fields[3].parse::<usize>().unwrap();
            let window = ScalarWindow::new(fields[4].parse().unwrap(), fields[5].parse().unwrap()).unwrap();
            let input = std::fs::metadata(&path).unwrap().len();
            let output = u64::from(width) * u64::from(height) * 16;
            let scratch = fields.get(7).map(|v| v.parse::<u64>().unwrap()).unwrap_or(0);
            let budget = rrrah_core::MemoryBudget::new(input + output + scratch);
            let mut request = DecodeRequest::new(path);
            request.memory_budget = Some(budget.clone());
            if fields[0].contains("-sequence-") || fields[0].contains("-geometry-") {
                let source = std::fs::read(&request.path).unwrap();
                for end in 0..source.len() {
                    assert!(
                        decode(&source[..end], &request, None).is_err(),
                        "{} truncation {end}",
                        fields[0]
                    );
                    assert_eq!(budget.used(), 0);
                }
            }
            for index in 0..frames {
                request.image_index = index;
                let frame = decode_file(&request).unwrap();
                assert_eq!((frame.width(), frame.height()), (width, height));
                assert_eq!((frame.image_index(), frame.image_count()), (index, frames));
                assert_eq!(
                    frame.pixel_aspect(),
                    fields
                        .get(6)
                        .filter(|v| **v != "-")
                        .map(|v| v.parse::<f32>().unwrap())
                );
                assert_eq!(frame.color_space(), &RasterColorSpace::Unspecified);
                assert_oracle(
                    &frame,
                    oracle(&root().join(format!("{}.{index}.raw-f32", fields[0]))),
                );
                assert_eq!(budget.used(), output);
                drop(frame);
                assert_eq!(budget.used(), 0);
                let frame = crate::decode_raster_with_window(&request, window).unwrap();
                assert_eq!(frame.color_space(), &RasterColorSpace::LinearSrgb);
                assert_oracle(
                    &frame,
                    oracle(&root().join(format!("{}.{index}.window-f32", fields[0]))),
                );
                assert_eq!(budget.used(), output);
                drop(frame);
                assert_eq!(budget.used(), 0);
            }
            assert_eq!(budget.peak(), input + output + scratch);
            request.image_index = frames;
            assert!(decode_file(&request).is_err());
            request.image_index = 0;
            request.memory_budget = Some(rrrah_core::MemoryBudget::new(input + output + scratch - 1));
            assert!(matches!(
                decode_file(&request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
        }
    }
    #[test]
    fn floating_layout_truncation_nonfinite_and_conflicting_pixels_release_memory() {
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("float.dcm");
        request.memory_budget = Some(budget.clone());
        for encoding in ["explicit", "big-endian", "implicit"] {
            for bits in [32, 64] {
                let name = format!("dicom-{encoding}-float{bits}.dcm");
                let bytes = std::fs::read(root().join(&name)).unwrap();
                let little = encoding != "big-endian";
                let implicit = encoding == "implicit";
                let tag = if bits == 32 { 8u16 } else { 9 };
                let tag_bytes: Vec<_> = [0x7fe0u16, tag]
                    .into_iter()
                    .flat_map(|v| if little { v.to_le_bytes() } else { v.to_be_bytes() })
                    .collect();
                let pixel = bytes.windows(4).position(|v| v == tag_bytes).unwrap();
                let payload = pixel + if implicit { 8 } else { 12 };
                for end in 0..bytes.len() {
                    assert!(
                        decode(&bytes[..end], &request, None).is_err(),
                        "{name} truncation {end}"
                    );
                    assert_eq!(budget.used(), 0);
                }
                for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    let mut invalid = bytes.clone();
                    let data = if bits == 32 {
                        let value = value as f32;
                        if little {
                            value.to_le_bytes().to_vec()
                        } else {
                            value.to_be_bytes().to_vec()
                        }
                    } else if little {
                        value.to_le_bytes().to_vec()
                    } else {
                        value.to_be_bytes().to_vec()
                    };
                    invalid[payload..payload + data.len()].copy_from_slice(&data);
                    assert!(matches!(
                        decode(&invalid, &request, None),
                        Err(RasterDecodeError::InvalidDicom(
                            "nonfinite floating pixel needs explicit missing-value policy"
                        ))
                    ));
                    assert_eq!(budget.used(), 0);
                }
                if bits == 64 {
                    // Retain double precision through windowing: converting to f32
                    // first would erase this 2^-30 increment and produce zero.
                    let mut precise = bytes.clone();
                    let value = 1. + 2f64.powi(-30);
                    precise[payload..payload + 8].copy_from_slice(&if little {
                        value.to_le_bytes()
                    } else {
                        value.to_be_bytes()
                    });
                    let window = ScalarWindow::new(1., 1. + f32::EPSILON).unwrap();
                    let frame = decode(&precise, &request, Some(window)).unwrap();
                    let RasterPixels::Rgba32Float(values) = frame.pixels() else {
                        panic!()
                    };
                    assert_eq!(values[0].to_bits(), (1f32 / 128.).to_bits());
                    drop(frame);
                    assert_eq!(budget.used(), 0);
                    precise[payload..payload + 8].copy_from_slice(&if little {
                        f64::MAX.to_le_bytes()
                    } else {
                        f64::MAX.to_be_bytes()
                    });
                    assert!(decode(&precise, &request, None).is_err());
                    assert_eq!(budget.used(), 0);
                    let frame =
                        decode(&precise, &request, Some(ScalarWindow::new(-1., 1.).unwrap())).unwrap();
                    let RasterPixels::Rgba32Float(values) = frame.pixels() else {
                        panic!()
                    };
                    assert_eq!(values[0], 1.);
                    drop(frame);
                    assert_eq!(budget.used(), 0);
                }
                // Floating pixels cannot coexist with integer Pixel Data, even empty.
                let mut mixed = bytes.clone();
                for v in [0x7fe0u16, 0x0010] {
                    mixed.extend(if little { v.to_le_bytes() } else { v.to_be_bytes() });
                }
                if !implicit {
                    mixed.extend_from_slice(b"OW\0\0");
                }
                mixed.extend_from_slice(&[0; 4]);
                assert!(matches!(
                    decode(&mixed, &request, None),
                    Err(RasterDecodeError::InvalidDicom("multiple pixel data elements"))
                ));
                assert_eq!(budget.used(), 0);
                // Bits Stored is prohibited for OF/OD, unlike integer storage.
                let mut field = Vec::new();
                for v in [0x0028u16, 0x0101] {
                    field.extend(if little { v.to_le_bytes() } else { v.to_be_bytes() });
                }
                if implicit {
                    field.extend_from_slice(&2u32.to_le_bytes());
                } else {
                    field.extend_from_slice(b"US");
                    field.extend(if little {
                        2u16.to_le_bytes()
                    } else {
                        2u16.to_be_bytes()
                    });
                }
                field.extend(if little {
                    16u16.to_le_bytes()
                } else {
                    16u16.to_be_bytes()
                });
                let mut invalid = bytes.clone();
                invalid.splice(pixel..pixel, field);
                assert!(matches!(
                    decode(&invalid, &request, None),
                    Err(RasterDecodeError::InvalidDicom("floating sample bit layout"))
                ));
                assert_eq!(budget.used(), 0);
            }
        }
        assert!(budget.peak() > 0);
    }
    #[test]
    fn rle_container_corruption_is_not_retryable_memory_pressure() {
        let bytes = std::fs::read(root().join("dicom-rle-signed12-rescale.dcm")).unwrap();
        let pixel = bytes
            .windows(12)
            .position(|v| v == [0xe0, 0x7f, 0x10, 0, b'O', b'B', 0, 0, 255, 255, 255, 255])
            .unwrap();
        let bot = pixel + 12;
        let first = bot + 16;
        let first_len = u32::from_le_bytes(bytes[first + 4..first + 8].try_into().unwrap()) as usize;
        let second = first + 8 + first_len;
        let second_len = u32::from_le_bytes(bytes[second + 4..second + 8].try_into().unwrap()) as usize;
        let delimiter = second + 8 + second_len;
        let budget = rrrah_core::MemoryBudget::new(0);
        for index in 0..2 {
            let mut request = DecodeRequest::new("corrupt-rle.dcm");
            request.image_index = index;
            request.memory_budget = Some(budget.clone());
            // The valid source reaches output admission; corrupt structure must
            // instead remain a permanent format error even with no free credit.
            assert!(matches!(
                decode(&bytes, &request, None),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            for end in 0..bytes.len() {
                assert!(
                    matches!(
                        decode(&bytes[..end], &request, None),
                        Err(RasterDecodeError::InvalidDicom(_))
                    ),
                    "frame {index}, truncation {end}"
                );
            }
            for (at, value) in [
                (bot + 8, 8u32), // offset must address the item, not payload
                (bot + 12, 0),   // second frame cannot reuse first fragment
                (first + 4, u32::MAX),
                (second + 4, second_len as u32 - 1),
                (delimiter + 4, 2),
                (if index == 0 { first } else { second } + 8, 0), // segment count
            ] {
                let mut damaged = bytes.clone();
                damaged[at..at + 4].copy_from_slice(&value.to_le_bytes());
                assert!(
                    matches!(
                        decode(&damaged, &request, None),
                        Err(RasterDecodeError::InvalidDicom(_))
                    ),
                    "frame {index}, damaged offset {at}"
                );
                assert_eq!(budget.used(), 0);
            }
        }
        assert_eq!(budget.peak(), 0);
    }
    #[test]
    fn encapsulated_rle_legacy_word_vr_preserves_selected_frame_bytes() {
        let mut bytes = std::fs::read(root().join("dicom-rle-signed12-rescale.dcm")).unwrap();
        let pixel = bytes
            .windows(12)
            .position(|v| v == [0xe0, 0x7f, 0x10, 0, b'O', b'B', 0, 0, 255, 255, 255, 255])
            .unwrap();
        for index in 0..2 {
            let mut request = DecodeRequest::new("rle.dcm");
            request.image_index = index;
            bytes[pixel + 5] = b'B';
            let canonical = decode(&bytes, &request, None).unwrap();
            bytes[pixel + 5] = b'W';
            let legacy = decode(&bytes, &request, None).unwrap();
            let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) =
                (canonical.pixels(), legacy.pixels())
            else {
                panic!("scalar precision lost")
            };
            assert_eq!(a.as_ref(), b.as_ref());
            assert_eq!(legacy.image_index(), index);
            assert_eq!(legacy.image_count(), 2);
        }
        bytes[pixel + 4..pixel + 6].copy_from_slice(b"UN");
        assert!(decode(&bytes, &DecodeRequest::new("rle.dcm"), None).is_err());
    }
    #[test]
    fn malformed_native_elements_transfer_syntax_and_truncations_fail() {
        let bytes = std::fs::read(root().join("dicom-signed12-rescale.dcm")).unwrap();
        let request = DecodeRequest::new("test.dcm");
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end], &request, None).is_err(), "truncation {end}");
        }
        let syntax = bytes
            .windows(19)
            .position(|v| v == b"1.2.840.10008.1.2.1")
            .unwrap();
        let mut invalid = bytes.clone();
        invalid[syntax + 18] = b'2';
        assert!(decode(&invalid, &request, None).is_err());
        let stored = bytes
            .windows(8)
            .position(|v| v == [0x28, 0, 1, 1, b'U', b'S', 2, 0])
            .unwrap();
        let mut invalid = bytes.clone();
        invalid[stored + 8..stored + 10].copy_from_slice(&17u16.to_le_bytes());
        assert!(decode(&invalid, &request, None).is_err());
        let pixel = bytes
            .windows(6)
            .position(|v| v == [0xe0, 0x7f, 0x10, 0, b'O', b'W'])
            .unwrap();
        let mut invalid = bytes.clone();
        invalid[pixel + 8..pixel + 12].fill(255);
        assert!(decode(&invalid, &request, None).is_err());
        let mut invalid = bytes.clone();
        invalid[140] ^= 1;
        assert!(decode(&invalid, &request, None).is_err()); // meta group length
        let mut cancelled = request;
        cancelled.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode(&bytes, &cancelled, None),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
    }
    #[test]
    fn rescale_overflow_and_unsupported_color_release_admitted_output() {
        let mut bytes = std::fs::read(root().join("dicom-signed12-rescale.dcm")).unwrap();
        let slope = bytes
            .windows(6)
            .position(|v| v == [0x28, 0, 0x53, 0x10, b'D', b'S'])
            .unwrap();
        let length = u16::from_le_bytes(bytes[slope + 6..slope + 8].try_into().unwrap()) as usize;
        bytes[slope + 6..slope + 8].copy_from_slice(&4u16.to_le_bytes());
        bytes.splice(slope + 8..slope + 8 + length, *b"1e39");
        let budget = rrrah_core::MemoryBudget::new(96);
        let mut request = DecodeRequest::new("scalar.dcm");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode(&bytes, &request, None),
            Err(RasterDecodeError::InvalidDicom("rescale overflow"))
        ));
        assert_eq!(budget.peak(), 96);
        assert_eq!(budget.used(), 0);
        let mut color = std::fs::read(root().join("dicom-signed12-rescale.dcm")).unwrap();
        let samples = color
            .windows(8)
            .position(|v| v == [0x28, 0, 2, 0, b'U', b'S', 2, 0])
            .unwrap();
        color[samples + 8..samples + 10].copy_from_slice(&3u16.to_le_bytes());
        assert!(matches!(
            decode(&color, &request, None),
            Err(RasterDecodeError::InvalidDicom(
                "non-scalar photometric interpretation"
            ))
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn big_endian_meta_boundary_vr_lengths_and_truncations_refuse_without_credit() {
        for name in [
            "dicom-big-endian-unsigned16.dcm",
            "dicom-big-endian-signed12-rescale.dcm",
            "dicom-big-endian-mono1-odd8-ob.dcm",
            "dicom-big-endian-mono1-odd8-ow.dcm",
            "dicom-big-endian-odd8-ow-multiframe.dcm",
        ] {
            let bytes = std::fs::read(root().join(name)).unwrap();
            let budget = rrrah_core::MemoryBudget::new(4096);
            let mut request = DecodeRequest::new(name);
            request.memory_budget = Some(budget.clone());
            for end in 0..bytes.len() {
                assert!(
                    decode(&bytes[..end], &request, None).is_err(),
                    "{name} truncation{end}"
                );
                assert_eq!(budget.used(), 0);
            }
            let pixel = bytes.windows(4).position(|v| v == [0x7f, 0xe0, 0, 0x10]).unwrap();
            let mut invalid = bytes.clone();
            invalid[pixel + 8..pixel + 12].fill(255);
            assert!(decode(&invalid, &request, None).is_err());
            assert_eq!(budget.used(), 0);
            let mut invalid = bytes.clone();
            invalid[pixel + 6] = 1;
            assert!(decode(&invalid, &request, None).is_err());
            assert_eq!(budget.used(), 0);
            let mut invalid = bytes.clone();
            invalid[140] ^= 1;
            assert!(decode(&invalid, &request, None).is_err());
            assert_eq!(budget.used(), 0);
            let syntax = bytes
                .windows(19)
                .position(|v| v == b"1.2.840.10008.1.2.2")
                .unwrap();
            let mut invalid = bytes.clone();
            invalid[syntax + 18] = b'1';
            assert!(decode(&invalid, &request, None).is_err());
            assert_eq!(budget.used(), 0);
            if name.contains("16") || name.contains("signed12") {
                let mut invalid = bytes.clone();
                invalid[pixel + 4..pixel + 6].copy_from_slice(b"OB");
                assert!(decode(&invalid, &request, None).is_err());
                assert_eq!(budget.used(), 0);
            }
        }
    }
    #[test]
    fn implicit_dictionary_refuses_standard_sequences_and_unknown_public_tags() {
        assert!(implicit_tags::KNOWN.windows(2).all(|p| p[0] < p[1]));
        assert!(implicit_tags::SEQUENCES.windows(2).all(|p| p[0] < p[1]));
        for &tag in implicit_tags::SEQUENCES {
            assert!(implicit_tags::KNOWN.binary_search(&tag).is_ok());
            assert_eq!(implicit_vr(tag), Some(b"SQ".as_slice()));
        }
        for tag in [
            0x00283000, 0x00283010, 0x52009229, 0x52009230, 0x50002600, 0x50fe2600,
        ] {
            assert_eq!(implicit_vr(tag), Some(b"SQ".as_slice()));
        }
        assert_eq!(implicit_vr(0x0028fffe), None);
        assert_eq!(implicit_vr(0x00191010), Some(b"UN".as_slice()));
    }
    #[test]
    fn implicit_truncation_sequence_lengths_private_bounds_and_public_unknown_refuse() {
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("implicit.dcm");
        request.memory_budget = Some(budget.clone());
        for name in [
            "dicom-implicit-signed12-rescale.dcm",
            "dicom-implicit-unsigned16.dcm",
            "dicom-implicit-mono1-odd8.dcm",
            "dicom-implicit-odd8-multiframe.dcm",
        ] {
            let bytes = std::fs::read(root().join(name)).unwrap();
            for end in 0..bytes.len() {
                assert!(
                    decode(&bytes[..end], &request, None).is_err(),
                    "{name} truncation{end}"
                );
                assert_eq!(budget.used(), 0);
            }
            let pixel = bytes.windows(4).position(|v| v == [0xe0, 0x7f, 0x10, 0]).unwrap();
            let mut invalid = bytes.clone();
            invalid[pixel + 4..pixel + 8].fill(255);
            assert!(decode(&invalid, &request, None).is_err());
            assert_eq!(budget.used(), 0);
            for tag in [
                [0x28, 0, 0x10, 0x30],
                [0xfe, 0x50, 0, 0x26],
                [0x28, 0, 0xfe, 0xff],
            ] {
                let mut invalid = bytes.clone();
                invalid.splice(pixel..pixel, tag.into_iter().chain([0, 0, 0, 0]));
                assert!(decode(&invalid, &request, None).is_err());
                assert_eq!(budget.used(), 0);
            }
            let row = bytes.windows(4).position(|v| v == [0x28, 0, 2, 0]).unwrap();
            // Private defined-length content is opaque, even when it resembles
            // nested pixel data. It cannot overwrite any root dataset fields.
            let payload = [0xe0, 0x7f, 0x10, 0, 2, 0, 0, 0, 0xff, 0xff];
            let mut private = bytes.clone();
            private.splice(
                row..row,
                [0x19, 0, 0x10, 0x10, 10, 0, 0, 0].into_iter().chain(payload),
            );
            let a = decode(&bytes, &request, None).unwrap();
            let b = decode(&private, &request, None).unwrap();
            let (RasterPixels::Rgba32Float(pa), RasterPixels::Rgba32Float(pb)) = (a.pixels(), b.pixels())
            else {
                panic!()
            };
            assert_eq!(
                pa.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                pb.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            );
            drop((a, b));
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn declared_padding_is_checked_in_stored_domain_for_each_syntax() {
        for encoding in ["explicit", "implicit", "big-endian"] {
            let name = match encoding {
                "explicit" => "dicom-signed12-rescale.dcm",
                "implicit" => "dicom-implicit-signed12-rescale.dcm",
                _ => "dicom-big-endian-signed12-rescale.dcm",
            };
            let bytes = std::fs::read(root().join(name)).unwrap();
            let little = encoding != "big-endian";
            let implicit = encoding == "implicit";
            let tag_bytes = if little {
                [0x28, 0, 0x52, 0x10]
            } else {
                [0, 0x28, 0x10, 0x52]
            };
            let insert = bytes.windows(4).position(|p| p == tag_bytes).unwrap();
            let field = |tag: u16, value: i16, vr: &[u8]| {
                let mut field = Vec::new();
                for v in [0x0028u16, tag] {
                    field.extend(if little { v.to_le_bytes() } else { v.to_be_bytes() });
                }
                if implicit {
                    field.extend_from_slice(&2u32.to_le_bytes());
                } else {
                    field.extend_from_slice(vr);
                    field.extend(if little {
                        2u16.to_le_bytes()
                    } else {
                        2u16.to_be_bytes()
                    });
                }
                field.extend(if little {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                });
                field
            };
            let budget = rrrah_core::MemoryBudget::new(4096);
            let mut request = DecodeRequest::new(name);
            request.memory_budget = Some(budget.clone());
            for (first, last, allowed) in [
                (-100, None, true),
                (-1, None, false),
                (20, Some(30), true),
                (-100, Some(-1), false),
                (30, Some(20), true),
            ] {
                let mut data = field(0x0120, first, b"SS");
                if let Some(last) = last {
                    data.extend(field(0x0121, last, b"SS"));
                }
                let mut modified = bytes.clone();
                modified.splice(insert..insert, data);
                let result = decode(&modified, &request, None);
                assert_eq!(result.is_ok(), allowed, "{encoding} {first}/{last:?}");
                if let Ok(frame) = result {
                    assert_oracle(&frame, oracle(&root().join(format!("{name}.0.raw-f32"))));
                    drop(frame);
                }
                assert_eq!(budget.used(), 0);
            }
            for data in [field(0x0121, 20, b"SS"), field(0x0120, 4096, b"SS")] {
                let mut modified = bytes.clone();
                modified.splice(insert..insert, data);
                assert!(decode(&modified, &request, None).is_err());
                assert_eq!(budget.used(), 0);
            }
            if !implicit {
                let mut modified = bytes.clone();
                modified.splice(insert..insert, field(0x0120, -100, b"US"));
                assert!(decode(&modified, &request, None).is_err());
                assert_eq!(budget.used(), 0);
            }
        }
    }
    #[test]
    #[ignore = "external packaged MR implicit source and independent pydicom oracles"]
    fn packaged_mr_implicit_matches_every_external_raw_and_window_sample() {
        packaged_scalar_oracle("RRRAH_DICOM_IMPLICIT", 64, ScalarWindow::new(0., 2048.).unwrap());
    }
    #[test]
    #[ignore = "external packaged CT source and independent pydicom oracles"]
    fn packaged_ct_metadata_sequence_and_absent_padding_match_every_sample() {
        packaged_scalar_oracle("RRRAH_DICOM_CT", 128, ScalarWindow::new(-1024., 2048.).unwrap());
    }
    #[test]
    #[ignore = "external packaged MR RLE source and independent scalar/window oracle"]
    fn packaged_mr_rle_matches_native_selected_scalar_pixels() {
        packaged_scalar_oracle_scratch(
            "RRRAH_DICOM_MR_RLE",
            64,
            ScalarWindow::new(0., 2048.).unwrap(),
            8192,
        );
    }
    fn packaged_scalar_oracle(prefix: &str, size: u32, window: ScalarWindow) {
        packaged_scalar_oracle_scratch(prefix, size, window, 0);
    }
    fn packaged_scalar_oracle_scratch(prefix: &str, size: u32, window: ScalarWindow, scratch: u64) {
        let output = u64::from(size) * u64::from(size) * 16;
        let decode_file = |req: &DecodeRequest| -> Result<DecodedRaster, RasterDecodeError> {
            let bytes = crate::bounded_io::read_bounded(req)?;
            super::decode(&bytes, req, None)
        };
        let source = std::path::PathBuf::from(std::env::var_os(format!("{prefix}_SOURCE")).expect("source"));
        let refs = std::path::PathBuf::from(std::env::var_os(format!("{prefix}_ORACLES")).expect("oracles"));
        let input = std::fs::metadata(&source).unwrap().len();
        let budget = rrrah_core::MemoryBudget::new(input + output + scratch);
        let mut request = DecodeRequest::new(source);
        request.memory_budget = Some(budget.clone());
        let raw = decode_file(&request).unwrap();
        assert_eq!(
            (raw.width(), raw.height(), raw.image_index(), raw.image_count()),
            (size, size, 0, 1)
        );
        assert_oracle(&raw, oracle(&refs.join("raw.f32")));
        assert_eq!(budget.used(), output);
        drop(raw);
        assert_eq!(budget.used(), 0);
        let display = crate::decode_raster_with_window(&request, window).unwrap();
        assert_oracle(&display, oracle(&refs.join("window.f32")));
        drop(display);
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.peak(), input + output + scratch);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(input + output + scratch - 1));
        assert!(decode_file(&request).is_err());
        assert_eq!(request.memory_budget.unwrap().used(), 0);
    }
}
