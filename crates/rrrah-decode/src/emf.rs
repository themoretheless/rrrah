//! Bounded base-header EMF stock-object geometry subset.
use crate::{DecodeRequest, RasterDecodeError};
use std::fmt::Write;
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidEmf(reason)
}
fn word(b: &[u8], n: usize) -> u32 {
    u32::from_le_bytes(b[n..n + 4].try_into().unwrap())
}
fn signed(b: &[u8], n: usize) -> i64 {
    i32::from_le_bytes(b[n..n + 4].try_into().unwrap()) as i64
}
#[derive(Clone, Copy)]
struct Mapping {
    mode: u32,
    window_origin: [i64; 2],
    viewport_origin: [i64; 2],
    window_extent: [i64; 2],
    viewport_extent: [i64; 2],
}
impl Mapping {
    fn transform(&self, header: &[u8]) -> Result<[f64; 4], RasterDecodeError> {
        let mut scale = [1.0; 2];
        if self.mode == 8 {
            for axis in 0..2 {
                scale[axis] = self.viewport_extent[axis] as f64 / self.window_extent[axis] as f64;
            }
        } else if self.mode != 1 {
            let millimeters = match self.mode {
                2 => 0.1,
                3 => 0.01,
                4 => 0.254,
                5 => 0.0254,
                6 => 25.4 / 1440.0,
                _ => return Err(bad("mapping mode")),
            };
            for axis in 0..2 {
                let pixels = signed(header, 72 + axis * 4);
                let mm = signed(header, 80 + axis * 4);
                if pixels <= 0 || mm <= 0 {
                    return Err(bad("mapping device dimensions"));
                }
                scale[axis] = pixels as f64 / mm as f64 * millimeters;
            }
            scale[1] = -scale[1];
        }
        let origin = [signed(header, 8), signed(header, 12)];
        Ok([
            scale[0],
            scale[1],
            (self.viewport_origin[0] - origin[0]) as f64
                + (origin[0] - self.window_origin[0]) as f64 * scale[0],
            (self.viewport_origin[1] - origin[1]) as f64
                + (origin[1] - self.window_origin[1]) as f64 * scale[1],
        ])
    }
}
#[derive(Clone)]
enum Object {
    Brush(String),
    Pen(String, i64),
}
fn color(value: u32) -> Result<String, RasterDecodeError> {
    if value >> 24 != 0 {
        return Err(bad("palette color unsupported"));
    }
    Ok(format!(
        "#{:02x}{:02x}{:02x}",
        value & 255,
        (value >> 8) & 255,
        (value >> 16) & 255
    ))
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.len() >= 44 && word(b, 0) == 1 && word(b, 40) == 0x464d4520
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
    if b.len() < 108 || !has_magic(b) {
        return Err(bad("base header/signature"));
    }
    let header_size = word(b, 4) as usize;
    if header_size < 88
        || header_size % 4 != 0
        || header_size > b.len() - 20
        || word(b, 44) != 0x10000
        || word(b, 48) as usize != b.len()
        || word(b, 68) != 0
    {
        return Err(bad("header size/version/description/palette"));
    }
    let description_count = word(b, 60) as usize;
    let description_offset = word(b, 64) as usize;
    let mut fixed_end = header_size;
    if description_count != 0 && description_offset != 0 {
        let end = description_count
            .checked_mul(2)
            .and_then(|size| description_offset.checked_add(size))
            .filter(|&end| end <= header_size)
            .ok_or_else(|| bad("description bounds"))?;
        if description_offset < 88 || description_offset % 2 != 0 || b[end - 2..end] != [0, 0] {
            return Err(bad("description offset/terminator"));
        }
        fixed_end = fixed_end.min(description_offset);
    }
    if fixed_end >= 100 && (word(b, 88) != 0 || word(b, 92) != 0 || word(b, 96) != 0) {
        return Err(bad("pixel format/OpenGL header unsupported"));
    }
    let records = word(b, 52) as usize;
    if !(2..=65536).contains(&records) {
        return Err(bad("record count"));
    }
    let width = signed(b, 16) - signed(b, 8) + 1;
    let height = signed(b, 20) - signed(b, 12) + 1;
    if width <= 0 || height <= 0 {
        return Err(bad("header bounds"));
    }
    if width > 65536 || height > 65536 || width * height * 4 > crate::raster::MAX_RASTER_BYTES as i64 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\">");
    let handles = u16::from_le_bytes(b[56..58].try_into().unwrap()) as usize;
    if !(1..=4096).contains(&handles) {
        return Err(bad("object table limit"));
    }
    let mut objects = vec![None; handles];
    let mut fill = "#ffffff".to_owned();
    let mut stroke = "#000000".to_owned();
    let mut stroke_width = 1;
    let mut fill_rule = "evenodd";
    let mut current = (0i64, 0i64);
    let mut mapping = Mapping {
        mode: 1,
        window_origin: [0, 0],
        viewport_origin: [0, 0],
        window_extent: [1, 1],
        viewport_extent: [1, 1],
    };
    let mut brush_id = None;
    let mut pen_id = None;
    let mut saved = Vec::new();
    let mut at = header_size;
    for index in 1..records {
        request.check_cancelled()?;
        if at + 8 > b.len() {
            return Err(bad("truncated record header"));
        }
        let kind = word(b, at);
        let size = word(b, at + 4) as usize;
        if size < 8 || size % 4 != 0 {
            return Err(bad("record size/alignment"));
        }
        let end = at
            .checked_add(size)
            .filter(|&n| n <= b.len())
            .ok_or_else(|| bad("record bounds"))?;
        let data = &b[at..end];
        let drawing = matches!(kind, 2..=6 | 42 | 43 | 54 | 85..=89);
        if drawing {
            let [sx, sy, tx, ty] = mapping.transform(b)?;
            write!(svg, "<g transform=\"matrix({sx} 0 0 {sy} {tx} {ty})\">").unwrap();
        }
        match kind {
            17 if size == 12 => {
                let mode = word(data, 8);
                if !matches!(mode, 1..=6 | 8) {
                    return Err(bad("mapping mode"));
                }
                mapping.mode = mode;
                mapping.transform(b)?;
            }
            9..=12 if size == 16 => {
                let value = [signed(data, 8), signed(data, 12)];
                if matches!(kind, 9 | 11) && (value[0] == 0 || value[1] == 0) {
                    return Err(bad("mapping zero extent"));
                }
                match kind {
                    9 => mapping.window_extent = value,
                    10 => mapping.window_origin = value,
                    11 => mapping.viewport_extent = value,
                    12 => mapping.viewport_origin = value,
                    _ => unreachable!(),
                }
            }
            27 | 54 if size == 16 => {
                let point = (signed(data, 8), signed(data, 12));
                if kind == 54 {
                    let x1 = current.0 - signed(b, 8);
                    let y1 = current.1 - signed(b, 12);
                    let x2 = point.0 - signed(b, 8);
                    let y2 = point.1 - signed(b, 12);
                    write!(svg, "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{stroke}\" stroke-width=\"{stroke_width}\"/>").unwrap();
                }
                current = point;
            }
            19 if size == 12 => {
                fill_rule = match word(data, 8) {
                    1 => "evenodd",
                    2 => "nonzero",
                    _ => return Err(bad("polygon fill mode")),
                };
            }
            2 | 5 | 6 | 85 | 88 | 89 if size >= 28 => {
                let count = word(data, 24) as usize;
                let stride = if kind >= 85 { 4 } else { 8 };
                let continuing = !matches!(kind, 2 | 85);
                let bezier = !matches!(kind, 6 | 89);
                let valid_count = if bezier {
                    if continuing {
                        count >= 3 && count % 3 == 0
                    } else {
                        count >= 4 && (count - 1) % 3 == 0
                    }
                } else {
                    count >= 1
                };
                if !valid_count || count > 16384 || size != 28 + count * stride {
                    return Err(bad("curve/continuation point count/size"));
                }
                let point_at = |index: usize| {
                    let at = 28 + index * stride;
                    let coordinate = |at| {
                        if stride == 4 {
                            i16::from_le_bytes(data[at..at + 2].try_into().unwrap()) as i64
                        } else {
                            signed(data, at)
                        }
                    };
                    (coordinate(at), coordinate(at + stride / 2))
                };
                let first = if continuing { current } else { point_at(0) };
                write!(
                    svg,
                    "<path d=\"M {},{} ",
                    first.0 - signed(b, 8),
                    first.1 - signed(b, 12)
                )
                .unwrap();
                let start = usize::from(!continuing);
                for index in start..count {
                    if index % 4096 == 0 {
                        request.check_cancelled()?;
                    }
                    if !bezier || (index - start) % 3 == 0 {
                        svg.push_str(if bezier { "C " } else { "L " });
                    }
                    let point = point_at(index);
                    write!(svg, "{},{} ", point.0 - signed(b, 8), point.1 - signed(b, 12)).unwrap();
                }
                write!(
                    svg,
                    "\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{stroke_width}\"/>"
                )
                .unwrap();
                if continuing {
                    current = point_at(count - 1);
                }
            }
            3 | 4 | 86 | 87 if size >= 28 => {
                let count = word(data, 24) as usize;
                let stride = if kind >= 86 { 4 } else { 8 };
                if !(2..=16384).contains(&count) || size != 28 + count * stride {
                    return Err(bad("polygon point count/size"));
                }
                let polygon = kind == 3 || kind == 86;
                let tag = if polygon { "polygon" } else { "polyline" };
                write!(svg, "<{tag} points=\"").unwrap();
                for point in 0..count {
                    if point % 4096 == 0 {
                        request.check_cancelled()?;
                    }
                    let offset = 28 + point * stride;
                    let coordinate = |offset| {
                        if stride == 4 {
                            i16::from_le_bytes(data[offset..offset + 2].try_into().unwrap()) as i64
                        } else {
                            signed(data, offset)
                        }
                    };
                    let x = coordinate(offset) - signed(b, 8);
                    let y = coordinate(offset + stride / 2) - signed(b, 12);
                    write!(svg, "{x},{y} ").unwrap();
                }
                let brush = if polygon { fill.as_str() } else { "none" };
                write!(svg, "\" fill=\"{brush}\" fill-rule=\"{fill_rule}\" stroke=\"{stroke}\" stroke-width=\"{stroke_width}\"/>").unwrap();
            }
            33 if size == 8 => {
                if saved.len() >= 256 {
                    return Err(bad("saved context limit"));
                }
                saved.push((
                    fill.clone(),
                    stroke.clone(),
                    stroke_width,
                    fill_rule,
                    current,
                    brush_id,
                    pen_id,
                    mapping,
                ));
            }
            34 if size == 12 => {
                let relative = signed(data, 8);
                let target = saved.len() as i64 + relative;
                if relative >= 0 || target < 0 {
                    return Err(bad("saved context index"));
                }
                (
                    fill,
                    stroke,
                    stroke_width,
                    fill_rule,
                    current,
                    brush_id,
                    pen_id,
                    mapping,
                ) = saved[target as usize].clone();
                saved.truncate(target as usize);
            }
            14 if size == 20 => {
                if index != records - 1 || end != b.len() || word(data, 8) != 0 || word(data, 16) != 20 {
                    return Err(bad("EOF count/palette/size"));
                }
                svg.push_str("</svg>");
                return crate::svg::decode(svg.as_bytes(), request);
            }
            40 if size == 12 => {
                let id = word(data, 8) as usize;
                if id == 0 {
                    return Err(bad("reserved delete index"));
                }
                let slot = objects.get_mut(id).ok_or_else(|| bad("delete object index"))?;
                if slot.take().is_none() {
                    return Err(bad("delete uncreated object"));
                }
                if brush_id == Some(id) {
                    fill = "#ffffff".into();
                    brush_id = None;
                }
                if pen_id == Some(id) {
                    stroke = "#000000".into();
                    stroke_width = 1;
                    pen_id = None;
                }
                // Saved selections must not resurrect a deleted object or alias its reused index.
                for state in &mut saved {
                    if state.5 == Some(id) {
                        state.0 = "#ffffff".into();
                        state.5 = None;
                    }
                    if state.6 == Some(id) {
                        state.1 = "#000000".into();
                        state.2 = 1;
                        state.6 = None;
                    }
                }
            }
            38 | 39 if size == if kind == 38 { 28 } else { 24 } => {
                let id = word(data, 8) as usize;
                if id == 0 {
                    return Err(bad("reserved object index"));
                }
                let slot = objects.get_mut(id).ok_or_else(|| bad("object index"))?;
                if slot.is_some() {
                    return Err(bad("duplicate object index"));
                }
                let style = word(data, 12);
                *slot = Some(if kind == 39 {
                    Object::Brush(match style {
                        0 => color(word(data, 16))?,
                        1 => "none".into(),
                        _ => return Err(bad("brush style")),
                    })
                } else {
                    let width = signed(data, 16);
                    if width < 0 || signed(data, 20) != 0 {
                        return Err(bad("pen width"));
                    }
                    Object::Pen(
                        match style {
                            0 => color(word(data, 24))?,
                            5 => "none".into(),
                            _ => return Err(bad("pen style")),
                        },
                        width.max(1),
                    )
                });
            }
            37 if size == 12 => {
                let id = word(data, 8);
                let object = if id & 0x80000000 != 0 {
                    match id {
                        0x80000000 => Object::Brush("#ffffff".into()),
                        0x80000004 => Object::Brush("#000000".into()),
                        0x80000005 => Object::Brush("none".into()),
                        0x80000006 => Object::Pen("#ffffff".into(), 1),
                        0x80000007 => Object::Pen("#000000".into(), 1),
                        0x80000008 => Object::Pen("none".into(), 1),
                        _ => return Err(bad("stock object")),
                    }
                } else {
                    objects
                        .get(id as usize)
                        .and_then(Clone::clone)
                        .ok_or_else(|| bad("uncreated object"))?
                };
                match object {
                    Object::Brush(value) => {
                        fill = value;
                        brush_id = (id & 0x80000000 == 0).then_some(id as usize);
                    }
                    Object::Pen(value, width) => {
                        pen_id = (id & 0x80000000 == 0).then_some(id as usize);
                        stroke = value;
                        stroke_width = width;
                    }
                }
            }
            42 | 43 if size == 24 => {
                let left = signed(data, 8) - signed(b, 8);
                let top = signed(data, 12) - signed(b, 12);
                let right = signed(data, 16) - signed(b, 8);
                let bottom = signed(data, 20) - signed(b, 12);
                let x = left.min(right);
                let y = top.min(bottom);
                let w = (right - left).abs();
                let h = (bottom - top).abs();
                if kind == 43 {
                    write!(svg,"<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{stroke_width}\"/>").unwrap();
                } else {
                    write!(svg,"<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{stroke_width}\"/>",x as f64+w as f64/2.,y as f64+h as f64/2.,w as f64/2.,h as f64/2.).unwrap();
                }
            }
            _ => return Err(bad("unsupported or malformed record")),
        }
        if drawing {
            svg.push_str("</g>");
        }
        if svg.len() > 16 * 1024 * 1024 {
            return Err(RasterDecodeError::OutputTooLarge);
        }
        at = end;
    }
    Err(bad("missing EOF"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = vec![0u8; 88];
        for (offset, value) in [
            (0, 1u32),
            (4, 88),
            (16, 9),
            (20, 9),
            (40, 0x464d4520),
            (44, 0x10000),
            (52, 5),
            (56, 1),
            (72, 100),
            (76, 100),
            (80, 25),
            (84, 25),
        ] {
            b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for object in [0x80000004u32, 0x80000008] {
            for word in [37, 12, object] {
                b.extend(word.to_le_bytes());
            }
        }
        for word in [43u32, 24, 0, 0, 10, 10, 14, 20, 0, 0, 20] {
            b.extend(word.to_le_bytes());
        }
        let length = b.len() as u32;
        b[48..52].copy_from_slice(&length.to_le_bytes());
        b
    }
    #[test]
    fn continuation_updates_position_even_with_null_pen() {
        for source in [
            include_bytes!("../../../tests/fixtures/emf/lineto32.emf").as_slice(),
            include_bytes!("../../../tests/fixtures/emf/lineto16.emf").as_slice(),
            include_bytes!("../../../tests/fixtures/emf/bezierto32.emf").as_slice(),
            include_bytes!("../../../tests/fixtures/emf/bezierto16.emf").as_slice(),
        ] {
            let mut at = word(source, 4) as usize;
            while !matches!(word(source, at), 5 | 6 | 88 | 89) {
                at += word(source, at + 4) as usize;
            }
            let mut bytes = source.to_vec();
            let null = [37u32, 12, 0x80000008]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            bytes.splice(at..at, null);
            let eof = bytes.len() - 20;
            let reverse = [37u32, 12, 2, 54, 16, 0, 5]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            bytes.splice(eof..eof, reverse);
            let length = bytes.len() as u32;
            bytes[48..52].copy_from_slice(&length.to_le_bytes());
            bytes[52..56].copy_from_slice(&(word(source, 52) + 3).to_le_bytes());
            let image = decode(&bytes, &DecodeRequest::new("position.emf")).unwrap();
            let rrrah_core::RasterPixels::Rgba8(values) = image.pixels() else {
                panic!()
            };
            assert!(values.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
        }
    }
    #[test]
    fn bezier_and_continuation_arrays_require_complete_segments() {
        let request = DecodeRequest::new("curves.emf");
        for source in [
            include_bytes!("../../../tests/fixtures/emf/bezier32.emf").as_slice(),
            include_bytes!("../../../tests/fixtures/emf/bezierto16.emf").as_slice(),
            include_bytes!("../../../tests/fixtures/emf/lineto32.emf").as_slice(),
        ] {
            let mut at = word(source, 4) as usize;
            while !matches!(word(source, at), 2 | 88 | 6) {
                at += word(source, at + 4) as usize;
            }
            for count in [0u32, 2, 5, 16385, u32::MAX] {
                let mut broken = source.to_vec();
                broken[at + 24..at + 28].copy_from_slice(&count.to_le_bytes());
                assert!(decode(&broken, &request).is_err());
            }
        }
    }
    #[test]
    fn mapping_modes_extents_and_device_dimensions_are_validated() {
        let source = include_bytes!("../../../tests/fixtures/emf/mapping-anisotropic.emf");
        let request = DecodeRequest::new("mapping.emf");
        let mut at = word(source, 4) as usize;
        while word(source, at) != 17 {
            at += word(source, at + 4) as usize;
        }
        for mode in [0u32, 7, 9, u32::MAX] {
            let mut broken = source.to_vec();
            broken[at + 8..at + 12].copy_from_slice(&mode.to_le_bytes());
            assert!(decode(&broken, &request).is_err());
        }
        for offset in [at + 20, at + 24, at + 36, at + 40] {
            let mut broken = source.to_vec();
            broken[offset..offset + 4].fill(0);
            assert!(decode(&broken, &request).is_err());
        }
        let metric = include_bytes!("../../../tests/fixtures/emf/mapping-lowmetric.emf");
        for offset in [72, 76, 80, 84] {
            let mut broken = metric.to_vec();
            broken[offset..offset + 4].fill(0);
            assert!(decode(&broken, &request).is_err());
        }
    }
    #[test]
    fn description_is_bounded_metadata_not_extension_fields() {
        let source = include_bytes!("../../../tests/fixtures/emf/description88.emf");
        let request = DecodeRequest::new("description.emf");
        assert!(decode(source, &request).is_ok());
        for (offset, value) in [(60, u32::MAX), (64, 86), (64, 89), (64, u32::MAX)] {
            let mut broken = source.to_vec();
            broken[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&broken, &request).is_err());
        }
        let mut broken = source.to_vec();
        let end = word(source, 64) as usize + word(source, 60) as usize * 2;
        broken[end - 2] = 1;
        assert!(decode(&broken, &request).is_err());
    }
    #[test]
    fn extension_header_bounds_and_gl_metadata_are_explicit() {
        for source in [
            include_bytes!("../../../tests/fixtures/emf/header100.emf").as_slice(),
            include_bytes!("../../../tests/fixtures/emf/header108.emf").as_slice(),
        ] {
            let request = DecodeRequest::new("extended.emf");
            assert!(decode(source, &request).is_ok());
            for (offset, value) in [(4, 84u32), (4, u32::MAX), (88, 40), (92, 108), (96, 1)] {
                let mut broken = source.to_vec();
                broken[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                assert!(decode(&broken, &request).is_err());
            }
        }
    }
    #[test]
    fn deleted_brush_defaults_and_invalid_indices_are_rejected() {
        let source = include_bytes!("../../../tests/fixtures/emf/red.emf");
        let at = source.len() - 44;
        let request = DecodeRequest::new("deleted.emf");
        for id in [1u32, 0, 2, 0x80000000] {
            let mut bytes = source.to_vec();
            let deletion = [40u32, 12, id]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            bytes.splice(at..at, deletion);
            let length = bytes.len() as u32;
            bytes[48..52].copy_from_slice(&length.to_le_bytes());
            bytes[52..56].copy_from_slice(&(word(source, 52) + 1).to_le_bytes());
            if id == 1 {
                let image = decode(&bytes, &request).unwrap();
                let rrrah_core::RasterPixels::Rgba8(values) = image.pixels() else {
                    panic!()
                };
                assert!(values.chunks_exact(4).all(|p| p == [255, 255, 255, 255]));
                // Selecting the cleared slot without recreating it must fail.
                let selection = [37u32, 12, 1]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>();
                bytes.splice(at + 12..at + 12, selection);
                let length = bytes.len() as u32;
                bytes[48..52].copy_from_slice(&length.to_le_bytes());
                bytes[52..56].copy_from_slice(&(word(source, 52) + 2).to_le_bytes());
                assert!(decode(&bytes, &request).is_err());
            } else {
                assert!(decode(&bytes, &request).is_err());
            }
        }
    }
    #[test]
    fn polygon_lengths_modes_and_open_lines() {
        for source in [
            include_bytes!("../../../tests/fixtures/emf/polygon32.emf").as_slice(),
            include_bytes!("../../../tests/fixtures/emf/polygon16.emf").as_slice(),
        ] {
            let request = DecodeRequest::new("polygon.emf");
            let mut at = 88;
            while !matches!(word(source, at), 3 | 86) {
                at += word(source, at + 4) as usize;
            }
            for count in [0u32, 1, 3, 5, 16385, u32::MAX] {
                let mut broken = source.to_vec();
                broken[at + 24..at + 28].copy_from_slice(&count.to_le_bytes());
                assert!(decode(&broken, &request).is_err());
            }
            let mut line = source.to_vec();
            let kind = word(source, at) + 1;
            line[at..at + 4].copy_from_slice(&kind.to_le_bytes());
            let image = decode(&line, &request).unwrap();
            let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
                panic!()
            };
            // With a null pen, a polyline must not use the selected red brush.
            assert!(pixels.iter().all(|&value| value == 0));
            for mode in [1u32, 2, 0, 3] {
                let mut modified = source.to_vec();
                let record = [19u32, 12, mode]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>();
                modified.splice(at..at, record);
                let length = modified.len() as u32;
                modified[48..52].copy_from_slice(&length.to_le_bytes());
                modified[52..56].copy_from_slice(&(word(source, 52) + 1).to_le_bytes());
                assert_eq!(decode(&modified, &request).is_ok(), mode == 1 || mode == 2);
            }
        }
    }
    #[test]
    fn context_indices_and_stack_limit_are_bounded() {
        let source = include_bytes!("../../../tests/fixtures/emf/saved-context.emf");
        let request = DecodeRequest::new("saved.emf");
        assert!(decode(source, &request).is_ok());
        let mut at = 88;
        while word(source, at) != 34 {
            at += word(source, at + 4) as usize;
        }
        for relative in [0i32, 1, -2, i32::MIN] {
            let mut broken = source.to_vec();
            broken[at + 8..at + 12].copy_from_slice(&relative.to_le_bytes());
            assert!(decode(&broken, &request).is_err());
        }
        let mut overflow = source.to_vec();
        let saves = [33u32.to_le_bytes().as_slice(), 8u32.to_le_bytes().as_slice()]
            .concat()
            .repeat(256);
        overflow.splice(88..88, saves);
        let length = overflow.len() as u32;
        overflow[48..52].copy_from_slice(&length.to_le_bytes());
        overflow[52..56].copy_from_slice(&265u32.to_le_bytes());
        assert!(matches!(
            decode(&overflow, &request),
            Err(RasterDecodeError::InvalidEmf("saved context limit"))
        ));
    }
    #[test]
    fn custom_object_indices_colors_and_null_pen() {
        let source = include_bytes!("../../../tests/fixtures/emf/red.emf");
        let request = DecodeRequest::new("objects.emf");
        for (offset, value) in [
            (56, 4097u32),
            (96, 0),
            (96, 2),
            (100, 3),
            (104, 0x01000000),
            (120, 0),
        ] {
            let mut broken = source.to_vec();
            broken[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&broken, &request).is_err());
        }
        let mut custom = source[..source.len() - 44].to_vec();
        for value in [38u32, 28, 2, 5, 0, 0, 0xff0000, 37, 12, 2] {
            custom.extend(value.to_le_bytes());
        }
        custom.extend(&source[source.len() - 44..]);
        let length = custom.len() as u32;
        custom[48..52].copy_from_slice(&length.to_le_bytes());
        custom[52..56].copy_from_slice(&8u32.to_le_bytes());
        custom[56..58].copy_from_slice(&3u16.to_le_bytes());
        let image = decode(&custom, &request).unwrap();
        let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
            panic!()
        };
        assert!(pixels.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
    }
    #[test]
    fn stock_rectangle_exact_pixels_and_admission() {
        let source = fixture();
        let mut request = DecodeRequest::new("rectangle.emf");
        let budget = rrrah_core::MemoryBudget::new(400);
        request.memory_budget = Some(budget.clone());
        let image = decode(&source, &request).unwrap();
        let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
            panic!()
        };
        assert!(pixels.is_managed());
        assert!(pixels.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
        drop(image);
        assert_eq!(budget.used(), 0);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(399));
        assert!(decode(&source, &request).is_err());
        assert_eq!(request.memory_budget.as_ref().unwrap().peak(), 0);
    }
    #[test]
    fn truncated_records_eof_count_unknown_and_selection_refused() {
        let source = fixture();
        let mut request = DecodeRequest::new("bad.emf");
        for n in 0..source.len() {
            assert!(decode(&source[..n], &request).is_err(), "prefix {n}");
        }
        for (at, value) in [(52, 4u32), (88, 999), (92, 9), (source.len() - 4, 0)] {
            let mut broken = source.clone();
            broken[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&broken, &request).is_err());
        }
        request.image_index = 1;
        assert!(decode(&source, &request).is_err());
    }
}
