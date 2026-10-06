//! Bounded placeable WMF geometry subset, translated to the shared SVG renderer.
//! MS-WMF header, placeable checksum, object-table and record layouts.
use crate::{DecodeRequest, RasterDecodeError};
use std::fmt::Write;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidWmf(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(&[0xd7, 0xcd, 0xc6, 0x9a])
}
#[derive(Clone)]
enum Object {
    Pen(String, f64),
    Brush(String),
}
fn color(b: &[u8]) -> Result<String, RasterDecodeError> {
    if b.len() != 4 || b[3] != 0 {
        return Err(bad("palette/color reference unsupported"));
    }
    Ok(format!("#{:02x}{:02x}{:02x}", b[0], b[1], b[2]))
}
pub(crate) fn decode(
    b: &[u8],
    request: &DecodeRequest,
) -> Result<rrrah_core::DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    if b.len() < 40 || !has_magic(b) {
        return Err(bad("placeable header required"));
    }
    let u16at = |n| u16::from_le_bytes(b[n..n + 2].try_into().unwrap());
    let i16at = |n| i16::from_le_bytes(b[n..n + 2].try_into().unwrap()) as f64;
    let u32at = |n| u32::from_le_bytes(b[n..n + 4].try_into().unwrap()) as usize;
    if u16at(4) != 0 || u32at(16) != 0 || (0..10).fold(0u16, |v, i| v ^ u16at(i * 2)) != u16at(20) {
        return Err(bad("placeable checksum/reserved fields"));
    }
    let bounds = [i16at(6), i16at(8), i16at(10) - i16at(6), i16at(12) - i16at(8)];
    let inch = u16at(14) as f64;
    if bounds[2] <= 0. || bounds[3] <= 0. || inch == 0. {
        return Err(bad("placeable dimensions"));
    }
    let width = (bounds[2] * 96. / inch).ceil().max(1.);
    let height = (bounds[3] * 96. / inch).ceil().max(1.);
    if width > 65536. || height > 65536. || width * height * 4. > crate::raster::MAX_RASTER_BYTES as f64 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if !matches!(u16at(22), 1 | 2)
        || u16at(24) != 9
        || !matches!(u16at(26), 0x100 | 0x300)
        || u32at(28).checked_mul(2).and_then(|n| n.checked_add(22)) != Some(b.len())
        || u16at(38) != 0
    {
        return Err(bad("metafile header/size"));
    }
    let count = u16at(32) as usize;
    if count > 4096 {
        return Err(bad("object count limit"));
    }
    let mut objects = vec![None; count];
    let mut pen = Object::Pen("#000000".into(), 0.);
    let mut brush = Object::Brush("#ffffff".into());
    let mut pen_index = None;
    let mut brush_index = None;
    let mut window = bounds;
    let mut viewport = [0., 0., width, height];
    let mut point = [0., 0.];
    let mut fill_rule = "evenodd";
    let mut saved = Vec::new();
    let mut at = 40;
    let mut maximum = 0;
    let mut ended = false;
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\">");
    for _ in 0..65536 {
        request.check_cancelled()?;
        if at + 6 > b.len() {
            return Err(bad("truncated record"));
        }
        let words = u32at(at);
        if words < 3 || words > u32at(34) {
            return Err(bad("record size/max record"));
        }
        maximum = maximum.max(words);
        let end = at
            .checked_add(words.checked_mul(2).ok_or_else(|| bad("record overflow"))?)
            .filter(|&n| n <= b.len())
            .ok_or_else(|| bad("record bounds"))?;
        let kind = u16at(at + 4);
        let data = &b[at + 6..end];
        let p = |n| i16::from_le_bytes(data[n..n + 2].try_into().unwrap()) as f64;
        let word = |n| u16::from_le_bytes(data[n..n + 2].try_into().unwrap());
        let map = |x: f64, y: f64| {
            [
                viewport[0] + (x - window[0]) * viewport[2] / window[2],
                viewport[1] + (y - window[1]) * viewport[3] / window[3],
            ]
        };
        match kind {
            0x001e if data.is_empty() => {
                if saved.len() >= 256 {
                    return Err(bad("saved context limit"));
                }
                saved.push((
                    pen.clone(),
                    brush.clone(),
                    window,
                    point,
                    fill_rule,
                    pen_index,
                    brush_index,
                    viewport,
                ));
            }
            0x0127 if data.len() == 2 => {
                let index = i16::from_le_bytes(data.try_into().unwrap()) as isize;
                let target = if index < 0 {
                    saved.len() as isize + index
                } else {
                    index - 1
                };
                if index == 0 || target < 0 || target as usize >= saved.len() {
                    return Err(bad("saved context index"));
                }
                (
                    pen,
                    brush,
                    window,
                    point,
                    fill_rule,
                    pen_index,
                    brush_index,
                    viewport,
                ) = saved[target as usize].clone();
                saved.truncate(target as usize);
            }
            0x01f0 if data.len() == 2 => {
                let index = word(0) as usize;
                if pen_index == Some(index)
                    || brush_index == Some(index)
                    || saved.iter().any(|s| s.5 == Some(index) || s.6 == Some(index))
                {
                    return Err(bad("deleting selected/saved object unsupported"));
                }
                let slot = objects.get_mut(index).ok_or_else(|| bad("delete object index"))?;
                if slot.take().is_none() {
                    return Err(bad("delete empty object"));
                }
            }
            0 if data.is_empty() => {
                if end != b.len() {
                    return Err(bad("trailing records"));
                }
                ended = true;
                break;
            }
            0x0103 if data.len() == 2 && word(0) == 8 => {}
            0x0106 if data.len() == 2 && matches!(word(0), 1 | 2) => {
                fill_rule = if word(0) == 1 { "evenodd" } else { "nonzero" };
            }
            0x020b if data.len() == 4 => {
                window[0] = p(2);
                window[1] = p(0);
            }
            0x020c if data.len() == 4 => {
                if p(0) == 0. || p(2) == 0. {
                    return Err(bad("zero window extents"));
                }
                window[2] = p(2);
                window[3] = p(0);
            }
            0x020d if data.len() == 4 => {
                viewport[0] = p(2);
                viewport[1] = p(0);
            }
            0x020e if data.len() == 4 => {
                if p(0) == 0. || p(2) == 0. {
                    return Err(bad("zero viewport extents"));
                }
                viewport[2] = p(2);
                viewport[3] = p(0);
            }
            0x02fa if data.len() == 10 => {
                let style = word(0);
                if !matches!(style, 0 | 5) || p(2) < 0. || p(4) != 0. {
                    return Err(bad("pen style/width"));
                }
                let slot = objects
                    .iter_mut()
                    .find(|v| v.is_none())
                    .ok_or_else(|| bad("object table full"))?;
                *slot = Some(Object::Pen(
                    if style == 5 {
                        "none".into()
                    } else {
                        color(&data[6..10])?
                    },
                    p(2),
                ));
            }
            0x02fc if data.len() == 8 => {
                let style = word(0);
                if !matches!(style, 0 | 1) {
                    return Err(bad("brush style"));
                }
                let slot = objects
                    .iter_mut()
                    .find(|v| v.is_none())
                    .ok_or_else(|| bad("object table full"))?;
                *slot = Some(Object::Brush(if style == 1 {
                    "none".into()
                } else {
                    color(&data[2..6])?
                }));
            }
            0x012d if data.len() == 2 => {
                let index = word(0);
                let object = if index & 0x8000 != 0 {
                    match index & 0x7fff {
                        0 => Object::Brush("#ffffff".into()),
                        4 => Object::Brush("#000000".into()),
                        5 => Object::Brush("none".into()),
                        6 => Object::Pen("#ffffff".into(), 0.),
                        7 => Object::Pen("#000000".into(), 0.),
                        8 => Object::Pen("none".into(), 0.),
                        _ => return Err(bad("stock object unsupported")),
                    }
                } else {
                    objects
                        .get(index as usize)
                        .and_then(|v| v.clone())
                        .ok_or_else(|| bad("object index"))?
                };
                match object {
                    Object::Pen(_, _) => {
                        pen = object;
                        pen_index = (index & 0x8000 == 0).then_some(index as usize);
                    }
                    Object::Brush(_) => {
                        brush = object;
                        brush_index = (index & 0x8000 == 0).then_some(index as usize);
                    }
                }
            }
            0x0214 if data.len() == 4 => point = [p(2), p(0)],
            0x0324 | 0x0325 if data.len() >= 2 => {
                let count = word(0) as usize;
                if !(2..=32767).contains(&count) || data.len() != 2 + count * 4 {
                    return Err(bad("polygon point count/length"));
                }
                let Object::Pen(ref stroke, sw) = pen else {
                    unreachable!()
                };
                let Object::Brush(ref fill) = brush else {
                    unreachable!()
                };
                let fill = if kind == 0x0325 { "none" } else { fill.as_str() };
                let sw = if sw == 0. {
                    1.
                } else {
                    sw * (viewport[2] / window[2]).abs()
                };
                let element = if kind == 0x0324 { "polygon" } else { "polyline" };
                write!(svg, "<{element} fill=\"{fill}\" fill-rule=\"{fill_rule}\" stroke=\"{stroke}\" stroke-width=\"{sw}\" points=\"").unwrap();
                for i in 0..count {
                    if i % 4096 == 0 {
                        request.check_cancelled()?;
                    }
                    let xy = map(p(2 + i * 4), p(4 + i * 4));
                    write!(svg, "{},{} ", xy[0], xy[1]).unwrap();
                }
                svg.push_str("\"/>");
            }
            0x041b | 0x0418 | 0x061c if data.len() == if kind == 0x061c { 12 } else { 8 } => {
                let offset = if kind == 0x061c { 4 } else { 0 };
                let a = map(p(offset + 6), p(offset + 4));
                let z = map(p(offset + 2), p(offset));
                let x = a[0].min(z[0]);
                let y = a[1].min(z[1]);
                let w = (z[0] - a[0]).abs();
                let h = (z[1] - a[1]).abs();
                let Object::Pen(ref stroke, sw) = pen else {
                    unreachable!()
                };
                let Object::Brush(ref fill) = brush else {
                    unreachable!()
                };
                let sw = if sw == 0. {
                    1.
                } else {
                    sw * (viewport[2] / window[2]).abs()
                };
                if kind == 0x061c {
                    let rx = (p(2) * viewport[2] / window[2]).abs().min(w) / 2.;
                    let ry = (p(0) * viewport[3] / window[3]).abs().min(h) / 2.;
                    write!(svg,"<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{rx}\" ry=\"{ry}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>").unwrap();
                } else if kind == 0x041b {
                    write!(svg,"<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>").unwrap();
                } else {
                    write!(svg,"<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>",x+w/2.,y+h/2.,w/2.,h/2.).unwrap();
                }
            }
            0x0213 if data.len() == 4 => {
                let a = map(point[0], point[1]);
                let z = map(p(2), p(0));
                let Object::Pen(ref stroke, sw) = pen else {
                    unreachable!()
                };
                let sw = if sw == 0. {
                    1.
                } else {
                    sw * (viewport[2] / window[2]).abs()
                };
                write!(svg,"<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>",a[0],a[1],z[0],z[1]).unwrap();
                point = [p(2), p(0)];
            }
            _ => return Err(bad("unsupported or malformed WMF record")),
        }
        if svg.len() > 16 * 1024 * 1024 {
            return Err(RasterDecodeError::OutputTooLarge);
        }
        at = end;
    }
    if !ended || maximum != u32at(34) {
        return Err(bad("missing EOF or incorrect maximum record"));
    }
    svg.push_str("</svg>");
    crate::svg::decode(svg.as_bytes(), request)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(0x9ac6cdd7u32.to_le_bytes());
        for v in [0u16, 0, 0, 10, 10, 96] {
            b.extend(v.to_le_bytes());
        }
        b.extend(0u32.to_le_bytes());
        let checksum = b
            .chunks_exact(2)
            .fold(0u16, |a, p| a ^ u16::from_le_bytes(p.try_into().unwrap()));
        b.extend(checksum.to_le_bytes());
        for v in [1u16, 9, 0x300] {
            b.extend(v.to_le_bytes());
        }
        b.extend(0u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(7u32.to_le_bytes());
        b.extend(0u16.to_le_bytes());
        let mut record = |kind: u16, args: &[u16]| {
            b.extend((3u32 + args.len() as u32).to_le_bytes());
            b.extend(kind.to_le_bytes());
            for v in args {
                b.extend(v.to_le_bytes());
            }
        };
        record(0x0103, &[8]);
        record(0x020b, &[0, 0]);
        record(0x020c, &[10, 10]);
        record(0x02fc, &[0, 255, 0, 0]);
        record(0x012d, &[0]);
        record(0x012d, &[0x8008]);
        record(0x041b, &[10, 10, 0, 0]);
        record(0, &[]);
        let words = ((b.len() - 22) / 2) as u32;
        b[28..32].copy_from_slice(&words.to_le_bytes());
        b
    }
    #[test]
    fn rounded_rectangle_clamps_ellipse_and_clears_corners() {
        let base = fixture();
        let request = DecodeRequest::new("rounded.wmf");
        for diameter in [0i16, 10, 20, -10] {
            let mut b = base[..base.len() - 20].to_vec();
            b.extend(9u32.to_le_bytes());
            b.extend(0x061cu16.to_le_bytes());
            for p in [diameter, diameter, 10, 10, 0, 0] {
                b.extend(p.to_le_bytes());
            }
            b.extend(&base[base.len() - 6..]);
            let size = ((b.len() - 22) / 2) as u32;
            b[28..32].copy_from_slice(&size.to_le_bytes());
            b[34..38].copy_from_slice(&9u32.to_le_bytes());
            let image = decode(&b, &request).unwrap();
            let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
                panic!()
            };
            assert_eq!(&pixels[220..224], &[255, 0, 0, 255]);
            if diameter == 0 {
                assert!(pixels.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
            } else {
                assert_eq!(pixels[3], 0);
            }
        }
    }
    #[test]
    fn viewport_translation_scale_and_reflection() {
        let base = fixture();
        let request = DecodeRequest::new("viewport.wmf");
        for (origin, extent, rect) in [
            ([0i16, 0], [20i16, 20], [5i16, 5, 0, 0]),
            ([10, 10], [-10, -10], [10, 10, 0, 0]),
            ([5, 5], [10, 10], [5, 5, -5, -5]),
        ] {
            let mut b = base[..base.len() - 20].to_vec();
            for (kind, args) in [
                (0x020du16, &origin[..]),
                (0x020e, &extent[..]),
                (0x041b, &rect[..]),
            ] {
                b.extend((3u32 + args.len() as u32).to_le_bytes());
                b.extend(kind.to_le_bytes());
                for p in args {
                    b.extend(p.to_le_bytes());
                }
            }
            b.extend(&base[base.len() - 6..]);
            let size = ((b.len() - 22) / 2) as u32;
            b[28..32].copy_from_slice(&size.to_le_bytes());
            let image = decode(&b, &request).unwrap();
            let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
                panic!()
            };
            assert!(pixels.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
        }
    }
    #[test]
    fn deletion_reuses_free_slots_and_refuses_live_objects() {
        let source = include_bytes!("../../../tests/fixtures/wmf/object-reuse.wmf");
        let request = DecodeRequest::new("reuse.wmf");
        assert!(decode(source, &request).is_ok());
        let mut at = 40;
        while u16::from_le_bytes(source[at + 4..at + 6].try_into().unwrap()) != 0x01f0 {
            at += u32::from_le_bytes(source[at..at + 4].try_into().unwrap()) as usize * 2;
        }
        for index in [1u16, 2, 3, 65535] {
            let mut broken = source.to_vec();
            broken[at + 6..at + 8].copy_from_slice(&index.to_le_bytes());
            assert!(decode(&broken, &request).is_err());
        }
    }
    #[test]
    fn saved_context_indices_and_stack_bounds() {
        let source = include_bytes!("../../../tests/fixtures/wmf/saved-context.wmf");
        let request = DecodeRequest::new("saved.wmf");
        let mut at = 40;
        while u16::from_le_bytes(source[at + 4..at + 6].try_into().unwrap()) != 0x0127 {
            at += u32::from_le_bytes(source[at..at + 4].try_into().unwrap()) as usize * 2;
        }
        for index in [0i16, -2, 2, i16::MIN, i16::MAX] {
            let mut broken = source.to_vec();
            broken[at + 6..at + 8].copy_from_slice(&index.to_le_bytes());
            assert!(decode(&broken, &request).is_err());
        }
        let mut absolute = source.to_vec();
        absolute[at + 6..at + 8].copy_from_slice(&1i16.to_le_bytes());
        let image = decode(&absolute, &request).unwrap();
        let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
            panic!()
        };
        assert!(pixels.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
        let mut overflow = source.to_vec();
        let saves = [3u32.to_le_bytes().as_slice(), 0x001eu16.to_le_bytes().as_slice()]
            .concat()
            .repeat(256);
        overflow.splice(40..40, saves);
        let size = ((overflow.len() - 22) / 2) as u32;
        overflow[28..32].copy_from_slice(&size.to_le_bytes());
        assert!(matches!(
            decode(&overflow, &request),
            Err(RasterDecodeError::InvalidWmf("saved context limit"))
        ));
    }
    #[test]
    fn polygon_counts_are_signed_bounded_and_exact() {
        let source = include_bytes!("../../../tests/fixtures/wmf/polygon.wmf");
        let request = DecodeRequest::new("polygon.wmf");
        assert!(decode(source, &request).is_ok());
        let mut at = 40;
        while u16::from_le_bytes(source[at + 4..at + 6].try_into().unwrap()) != 0x0324 {
            at += u32::from_le_bytes(source[at..at + 4].try_into().unwrap()) as usize * 2;
        }
        for count in [0u16, 1, 3, 5, 32768, 65535] {
            let mut broken = source.to_vec();
            broken[at + 6..at + 8].copy_from_slice(&count.to_le_bytes());
            assert!(decode(&broken, &request).is_err());
        }
        let mut polyline = source.to_vec();
        polyline[at + 4..at + 6].copy_from_slice(&0x0325u16.to_le_bytes());
        let image = decode(&polyline, &request).unwrap();
        let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
            panic!()
        };
        // A null pen must leave an open polyline empty; the selected brush is ignored.
        assert!(pixels.iter().all(|&v| v == 0));
    }
    #[test]
    fn rectangle_pixels_budget_and_last_owner() {
        let b = fixture();
        let mut request = DecodeRequest::new("fixture.wmf");
        let small = rrrah_core::MemoryBudget::new(399);
        request.memory_budget = Some(small.clone());
        assert!(decode(&b, &request).is_err());
        assert_eq!(small.peak(), 0);
        let budget = rrrah_core::MemoryBudget::new(400);
        request.memory_budget = Some(budget.clone());
        let image = decode(&b, &request).unwrap();
        let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
            panic!()
        };
        assert!(pixels.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
        let last = image.clone();
        drop(image);
        assert_eq!(budget.used(), 400);
        drop(last);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn truncation_checksum_unknown_record_and_index_are_refused() {
        let b = fixture();
        let mut request = DecodeRequest::new("fixture.wmf");
        for n in 0..b.len() {
            assert!(decode(&b[..n], &request).is_err(), "prefix {n}");
        }
        let mut bad_checksum = b.clone();
        bad_checksum[20] ^= 1;
        assert!(decode(&bad_checksum, &request).is_err());
        let mut unknown = b.clone();
        unknown[44..46].copy_from_slice(&0xffffu16.to_le_bytes());
        assert!(decode(&unknown, &request).is_err());
        request.image_index = 1;
        assert!(matches!(
            decode(&b, &request),
            Err(RasterDecodeError::Source(
                crate::DecodeError::UnsupportedImageIndex { index: 1 }
            ))
        ));
    }
}
