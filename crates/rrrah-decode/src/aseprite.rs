//! Selected Aseprite frames with borrowed cel streams and native sample blending.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone)]
pub struct AsepriteTag {
    pub name: String,
    pub first_frame: u16,
    pub last_frame: u16,
    /// 0 forward, 1 reverse, 2 ping-pong, 3 reverse ping-pong.
    pub direction: u8,
    /// Zero is unspecified (infinite in the editor, one cycle when exporting).
    pub repeats: u16,
}
#[derive(Debug, Clone)]
pub struct AsepriteImage {
    pub raster: DecodedRaster,
    pub delay_ms: u32,
    pub tags: Vec<AsepriteTag>,
    pub layer_names: Vec<String>,
}
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidAseprite(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.len() >= 134 && b.get(4..6) == Some(&[0xe0, 0xa5]) && b.get(132..134) == Some(&[0xfa, 0xf1])
}
fn word(b: &[u8]) -> u16 {
    u16::from_le_bytes(b.try_into().unwrap())
}
fn dword(b: &[u8]) -> u32 {
    u32::from_le_bytes(b.try_into().unwrap())
}
struct Scan<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> Scan<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RasterDecodeError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&n| n <= self.b.len())
            .ok_or_else(|| bad("truncated chunk"))?;
        let b = &self.b[self.at..end];
        self.at = end;
        Ok(b)
    }
    fn byte(&mut self) -> Result<u8, RasterDecodeError> {
        Ok(self.take(1)?[0])
    }
    fn word(&mut self) -> Result<u16, RasterDecodeError> {
        Ok(word(self.take(2)?))
    }
    fn string(&mut self) -> Result<String, RasterDecodeError> {
        let n = usize::from(self.word()?);
        let s = std::str::from_utf8(self.take(n)?).map_err(|_| bad("invalid UTF-8 name"))?;
        Ok(s.to_owned())
    }
    fn finish(self) -> Result<(), RasterDecodeError> {
        if self.at != self.b.len() {
            return Err(bad("trailing chunk data"));
        }
        Ok(())
    }
}
struct Layer {
    name: String,
    parent: Option<usize>,
    visible: bool,
    background: bool,
    group: bool,
    opacity: u8,
    blend: u16,
}
#[derive(Clone, Copy)]
enum CelData<'a> {
    Image {
        w: usize,
        h: usize,
        compressed: bool,
        data: &'a [u8],
    },
    Link(usize),
}
#[derive(Clone, Copy)]
struct Cel<'a> {
    x: i32,
    y: i32,
    opacity: u8,
    z: i32,
    data: CelData<'a>,
}
struct Inventory<'a> {
    w: usize,
    h: usize,
    depth: usize,
    transparent: u8,
    isolate_groups: bool,
    layers: Vec<Layer>,
    cels: Vec<BTreeMap<usize, Cel<'a>>>,
    delays: Vec<u32>,
    palette: [Option<[u8; 4]>; 256],
    color: RasterColorSpace,
    tags: Vec<AsepriteTag>,
    max_cel_bytes: usize,
}
fn palette_chunk(
    data: &[u8],
    kind: u16,
    palette: &mut [Option<[u8; 4]>; 256],
) -> Result<(), RasterDecodeError> {
    let mut s = Scan { b: data, at: 0 };
    if kind == 0x2019 {
        let h = s.take(20)?;
        let (size, first, last) = (
            dword(&h[..4]) as usize,
            dword(&h[4..8]) as usize,
            dword(&h[8..12]) as usize,
        );
        if size == 0 || size > 256 || first > last || last >= size {
            return Err(bad("palette range"));
        }
        palette[size..].fill(None);
        for p in &mut palette[first..=last] {
            let flags = s.word()?;
            if flags & !1 != 0 {
                return Err(bad("palette entry flags"));
            }
            *p = Some(s.take(4)?.try_into().unwrap());
            if flags & 1 != 0 {
                s.string()?;
            }
        }
    } else {
        let packets = s.word()?;
        let mut index = 0usize;
        for _ in 0..packets {
            index += usize::from(s.byte()?);
            let n = s.byte()?;
            let n = if n == 0 { 256 } else { usize::from(n) };
            if index + n > 256 {
                return Err(bad("legacy palette range"));
            }
            for p in &mut palette[index..index + n] {
                let rgb = s.take(3)?;
                let convert = |v: u8| -> Result<u8, RasterDecodeError> {
                    if kind == 0x11 {
                        if v > 63 {
                            return Err(bad("legacy six-bit palette code"));
                        }
                        Ok((v << 2) | (v >> 4))
                    } else {
                        Ok(v)
                    }
                };
                *p = Some([convert(rgb[0])?, convert(rgb[1])?, convert(rgb[2])?, 255]);
            }
            index += n;
        }
    }
    s.finish()
}
fn color_chunk(b: &[u8]) -> Result<RasterColorSpace, RasterDecodeError> {
    if b.len() < 16 {
        return Err(bad("color profile header"));
    }
    let (kind, flags, gamma) = (word(&b[..2]), word(&b[2..4]), dword(&b[4..8]));
    if flags & !1 != 0 {
        return Err(bad("color profile flags"));
    }
    if flags & 1 != 0 && !(kind == 1 && gamma == 65536) {
        return Err(bad("unsupported fixed gamma"));
    }
    match kind {
        0 if b.len() == 16 => Ok(RasterColorSpace::AssumedSrgb),
        1 if b.len() == 16 => Ok(if flags & 1 != 0 {
            RasterColorSpace::LinearSrgb
        } else {
            RasterColorSpace::Srgb
        }),
        2 if b.len() >= 20 => {
            let n = dword(&b[16..20]) as usize;
            if n == 0 || n > 16 * 1024 * 1024 || b.len() != 20 + n {
                return Err(bad("ICC bounds"));
            }
            Ok(RasterColorSpace::Icc(b[20..].to_vec()))
        }
        _ => Err(bad("color profile type/length")),
    }
}
fn inventory<'a>(b: &'a [u8], request: &DecodeRequest) -> Result<Inventory<'a>, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() < 128 || !has_magic(b) || dword(&b[..4]) as usize != b.len() {
        return Err(bad("file size/signature/header"));
    }
    let (frames, w, h, depth, flags) = (
        usize::from(word(&b[6..8])),
        usize::from(word(&b[8..10])),
        usize::from(word(&b[10..12])),
        usize::from(word(&b[12..14])),
        dword(&b[14..18]),
    );
    if frames == 0 || w == 0 || h == 0 {
        return Err(bad("empty sprite"));
    }
    if !matches!(depth, 8 | 16 | 32) || flags & !7 != 0 {
        return Err(bad("unsupported depth/header flags"));
    }
    if b[34] != 0 && b[35] != 0 && b[34] != b[35] {
        return Err(bad("non-square pixels unsupported"));
    }
    if w as u64 * h as u64 * 16 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if request.image_index >= frames {
        return Err(bad("frame index out of range"));
    }
    let mut result = Inventory {
        w,
        h,
        depth: depth / 8,
        transparent: b[28],
        isolate_groups: flags & 2 != 0,
        layers: Vec::new(),
        cels: Vec::with_capacity(frames),
        delays: Vec::with_capacity(frames),
        palette: [None; 256],
        color: RasterColorSpace::AssumedSrgb,
        tags: Vec::new(),
        max_cel_bytes: 0,
    };
    let mut outer = Scan { b, at: 128 };
    let mut groups: Vec<usize> = Vec::new();
    let mut chunk_total = 0usize;
    let mut profile_seen = false;
    let mut tags_seen = false;
    for frame in 0..frames {
        request.check_cancelled()?;
        let f = outer.take(16)?;
        let size = dword(&f[..4]) as usize;
        if size < 16 || word(&f[4..6]) != 0xf1fa {
            return Err(bad("frame header"));
        }
        let old = usize::from(word(&f[6..8]));
        let new = dword(&f[12..16]) as usize;
        let count = if new != 0 { new } else { old };
        chunk_total = chunk_total
            .checked_add(count)
            .filter(|&n| n <= 1_048_576)
            .ok_or_else(|| bad("chunk count limit"))?;
        let delay = word(&f[8..10]);
        result
            .delays
            .push(u32::from(if delay == 0 { word(&b[18..20]) } else { delay }));
        let mut s = Scan {
            b: outer.take(size - 16)?,
            at: 0,
        };
        let mut chunks = Vec::with_capacity(count.min(65536));
        for _ in 0..count {
            request.check_cancelled()?;
            let h = s.take(6)?;
            let n = dword(&h[..4]) as usize;
            if n < 6 {
                return Err(bad("chunk size"));
            }
            chunks.push((word(&h[4..6]), s.take(n - 6)?));
        }
        s.finish()?;
        let new_palette = chunks.iter().any(|&(kind, _)| kind == 0x2019);
        let mut frame_cels = BTreeMap::new();
        for (kind, data) in chunks {
            request.check_cancelled()?;
            match kind {
                0x2004 => {
                    if frame != 0 || result.layers.len() >= 4096 {
                        return Err(bad("layer layout/count"));
                    }
                    let mut s = Scan { b: data, at: 0 };
                    let h = s.take(16)?;
                    let (lf, typ, level, blend) = (
                        word(&h[..2]),
                        word(&h[2..4]),
                        usize::from(word(&h[4..6])),
                        word(&h[10..12]),
                    );
                    if lf & !127 != 0 || typ > 1 || blend > 18 {
                        return Err(bad("unsupported layer flags/type/blend"));
                    }
                    let name = s.string()?;
                    if flags & 4 != 0 {
                        s.take(16)?;
                    }
                    s.finish()?;
                    if level > 64 || level > groups.len() {
                        return Err(bad("layer hierarchy"));
                    }
                    groups.truncate(level);
                    let parent = groups.last().copied();
                    let visible =
                        lf & 1 != 0 && lf & 64 == 0 && parent.is_none_or(|p| result.layers[p].visible);
                    let group = typ == 1;
                    let opacity = if (group && flags & 2 != 0) || (!group && flags & 1 != 0) {
                        h[12]
                    } else {
                        255
                    };
                    let blend = if group && flags & 2 == 0 { 0 } else { blend };
                    let index = result.layers.len();
                    result.layers.push(Layer {
                        name,
                        parent,
                        visible,
                        background: lf & 8 != 0,
                        group,
                        opacity,
                        blend,
                    });
                    if group {
                        groups.push(index);
                    }
                }
                0x2005 => {
                    if data.len() < 16 {
                        return Err(bad("cel header"));
                    }
                    let layer = usize::from(word(&data[..2]));
                    if result.layers.get(layer).is_none_or(|l| l.group) {
                        return Err(bad("cel layer index/type"));
                    }
                    let cel_type = word(&data[7..9]);
                    let cel_data = match cel_type {
                        0 | 2 if data.len() >= 20 => {
                            let (cw, ch) =
                                (usize::from(word(&data[16..18])), usize::from(word(&data[18..20])));
                            let bytes = cw
                                .checked_mul(ch)
                                .and_then(|n| n.checked_mul(result.depth))
                                .filter(|&n| n as u64 <= MAX_RASTER_BYTES / 2)
                                .ok_or(RasterDecodeError::OutputTooLarge)?;
                            if cw == 0
                                || ch == 0
                                || (cel_type == 0 && data.len() != 20 + bytes)
                                || (cel_type == 2 && data.len() <= 20)
                            {
                                return Err(bad("cel dimensions/payload"));
                            }
                            result.max_cel_bytes = result.max_cel_bytes.max(bytes);
                            CelData::Image {
                                w: cw,
                                h: ch,
                                compressed: cel_type == 2,
                                data: &data[20..],
                            }
                        }
                        1 if data.len() == 18 => {
                            let target = usize::from(word(&data[16..18]));
                            if target >= frames || target == frame {
                                return Err(bad("invalid/self linked cel"));
                            }
                            CelData::Link(target)
                        }
                        _ => return Err(bad("unsupported cel type/length")),
                    };
                    let cel = Cel {
                        x: i32::from(i16::from_le_bytes(data[2..4].try_into().unwrap())),
                        y: i32::from(i16::from_le_bytes(data[4..6].try_into().unwrap())),
                        opacity: data[6],
                        z: i32::from(i16::from_le_bytes(data[9..11].try_into().unwrap())),
                        data: cel_data,
                    };
                    if frame_cels.insert(layer, cel).is_some() {
                        return Err(bad("duplicate layer cel"));
                    }
                }
                0x4 | 0x11 | 0x2019 => {
                    let apply = frame <= request.image_index && (kind == 0x2019 || !new_palette);
                    if apply {
                        palette_chunk(data, kind, &mut result.palette)?;
                    } else {
                        palette_chunk(data, kind, &mut [None; 256])?;
                    }
                }
                0x2007 => {
                    if profile_seen || frame != 0 {
                        return Err(bad("duplicate/noninitial color profile"));
                    }
                    result.color = color_chunk(data)?;
                    profile_seen = true;
                }
                0x2006 => {
                    if data.len() != 36 || dword(&data[..4]) != 0 {
                        return Err(bad("precise/scaled cel bounds unsupported"));
                    }
                }
                0x2018 => {
                    if tags_seen {
                        return Err(bad("duplicate tags chunk"));
                    }
                    tags_seen = true;
                    let mut s = Scan { b: data, at: 0 };
                    let count = s.word()?;
                    s.take(8)?;
                    for _ in 0..count {
                        let h = s.take(17)?;
                        let (first, last, direction) = (word(&h[..2]), word(&h[2..4]), h[4]);
                        if first > last || usize::from(last) >= frames || direction > 3 {
                            return Err(bad("tag range/direction"));
                        }
                        let name = s.string()?;
                        result.tags.push(AsepriteTag {
                            name,
                            first_frame: first,
                            last_frame: last,
                            direction,
                            repeats: word(&h[5..7]),
                        });
                    }
                    s.finish()?;
                }
                // Non-rendering metadata stays within already checked chunk bounds.
                0x2016 | 0x2017 | 0x2020 | 0x2022 => {}
                _ => return Err(bad("unsupported chunk type")),
            }
        }
        result.cels.push(frame_cels);
    }
    outer.finish()?;
    // Resolve the graph in place once. Forward references are legal; a link
    // copies only image storage, retaining the current cel's position/opacity/z.
    for frame in 0..frames {
        let keys: Vec<_> = result.cels[frame].keys().copied().collect();
        for layer in keys {
            request.check_cancelled()?;
            let mut path = Vec::new();
            let mut visited = std::collections::HashSet::new();
            let mut target = frame;
            let image = loop {
                request.check_cancelled()?;
                let cel = result.cels[target]
                    .get(&layer)
                    .ok_or_else(|| bad("missing linked cel"))?;
                match cel.data {
                    data @ CelData::Image { .. } => break data,
                    CelData::Link(next) => {
                        if !visited.insert(target) {
                            return Err(bad("cyclic linked cels"));
                        }
                        path.push(target);
                        target = next;
                    }
                }
            };
            for target in path {
                result.cels[target].get_mut(&layer).unwrap().data = image;
            }
        }
    }
    if result.layers.is_empty() {
        return Err(bad("missing layer layout"));
    }
    let depth = groups.len().max(
        result
            .layers
            .iter()
            .map(|l| {
                let mut n = 0;
                let mut p = l.parent;
                while let Some(i) = p {
                    n += 1;
                    p = result.layers[i].parent;
                }
                n
            })
            .max()
            .unwrap_or(0),
    );
    if depth > 64
        || (w as u64 * h as u64 * 4 * (depth as u64 + 2) + result.max_cel_bytes as u64) > MAX_RASTER_BYTES
    {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    Ok(result)
}
fn inflate_into(data: &[u8], out: &mut [u8], request: &DecodeRequest) -> Result<(), RasterDecodeError> {
    let mut decoder = flate2::Decompress::new(true);
    let mut extra = [0u8; 1];
    loop {
        request.check_cancelled()?;
        let before = (decoder.total_in(), decoder.total_out());
        let (input, output) = (before.0 as usize, before.1 as usize);
        let source = &data[input..data.len().min(input + 65536)];
        let target = if output < out.len() {
            let end = out.len().min(output + 65536);
            &mut out[output..end]
        } else {
            &mut extra[..]
        };
        let status = decoder
            .decompress(source, target, flate2::FlushDecompress::None)
            .map_err(|_| bad("invalid cel zlib stream"))?;
        if decoder.total_out() > out.len() as u64 {
            return Err(bad("cel expands beyond dimensions"));
        }
        if status == flate2::Status::StreamEnd {
            if decoder.total_out() != out.len() as u64 || decoder.total_in() != data.len() as u64 {
                return Err(bad("cel decoded size/trailing zlib data"));
            }
            return Ok(());
        }
        if before == (decoder.total_in(), decoder.total_out()) {
            return Err(bad("truncated/stalled cel zlib stream"));
        }
    }
}
fn unpack<'a>(
    data: &'a [u8],
    expected: usize,
    compressed: bool,
    request: &DecodeRequest,
) -> Result<std::borrow::Cow<'a, [u8]>, RasterDecodeError> {
    if !compressed {
        return Ok(std::borrow::Cow::Borrowed(data));
    }
    let mut out = vec![0; expected];
    inflate_into(data, &mut out, request)?;
    Ok(std::borrow::Cow::Owned(out))
}
fn multiply(a: u8, b: u8) -> u8 {
    let t = u32::from(a) * u32::from(b) + 128;
    ((t + (t >> 8)) >> 8) as u8
}
// Aseprite normal source-over rounds alpha to eight bits and truncates signed
// RGB division in the source sample domain, before display color preparation.
fn over(dst: &mut [u8], src: &[u8; 4], opacity: u8) {
    let alpha = multiply(src[3], opacity);
    if dst[3] == 0 {
        dst[..3].copy_from_slice(&src[..3]);
        dst[3] = alpha;
        return;
    }
    if src[3] == 0 {
        return;
    }
    let out_alpha = u32::from(alpha) + u32::from(dst[3]) - u32::from(multiply(dst[3], alpha));
    for c in 0..3 {
        dst[c] = (i32::from(dst[c])
            + (i32::from(src[c]) - i32::from(dst[c])) * i32::from(alpha) / out_alpha as i32)
            as u8;
    }
    dst[3] = out_alpha as u8;
}
fn render_cel(
    out: &mut [u8],
    layer: usize,
    cel: Cel<'_>,
    inv: &Inventory<'_>,
    request: &DecodeRequest,
    empty: bool,
) -> Result<bool, RasterDecodeError> {
    let opacity = multiply(inv.layers[layer].opacity, cel.opacity);
    if opacity == 0 {
        return Ok(false);
    }
    let mut payload = cel.data;
    while let CelData::Link(frame) = payload {
        request.check_cancelled()?;
        payload = inv.cels[frame][&layer].data;
    }
    let CelData::Image {
        w,
        h,
        compressed,
        data,
    } = payload
    else {
        unreachable!()
    };
    let (left, top) = (cel.x.max(0) as usize, cel.y.max(0) as usize);
    let (right, bottom) = (
        (cel.x + w as i32).max(0) as usize,
        (cel.y + h as i32).max(0) as usize,
    );
    let (right, bottom) = (right.min(inv.w), bottom.min(inv.h));
    if left >= right || top >= bottom {
        return Ok(false);
    }
    if empty && opacity == 255 && inv.depth == 4 && cel.x == 0 && cel.y == 0 && w == inv.w && h == inv.h {
        if compressed {
            inflate_into(data, out, request)?;
        } else {
            out.copy_from_slice(data);
        }
        request.check_cancelled()?;
        return Ok(true);
    }
    let decoded = unpack(data, w * h * inv.depth, compressed, request)?;
    for y in top..bottom {
        request.check_cancelled()?;
        for x in left..right {
            let i = ((y as i32 - cel.y) as usize * w + (x as i32 - cel.x) as usize) * inv.depth;
            let src = match inv.depth {
                4 => decoded[i..i + 4].try_into().unwrap(),
                2 => [decoded[i], decoded[i], decoded[i], decoded[i + 1]],
                1 => {
                    let index = decoded[i];
                    let mut p = inv.palette[usize::from(index)]
                        .ok_or_else(|| bad("used palette entry is undefined"))?;
                    if index == inv.transparent && !inv.layers[layer].background {
                        p[3] = 0;
                    }
                    p
                }
                _ => unreachable!(),
            };
            over(
                &mut out[(y * inv.w + x) * 4..(y * inv.w + x + 1) * 4],
                &src,
                opacity,
            );
        }
    }
    Ok(true)
}
fn render_group(
    out: &mut [u8],
    parent: Option<usize>,
    inv: &Inventory<'_>,
    request: &DecodeRequest,
    empty: &mut bool,
) -> Result<(), RasterDecodeError> {
    let mut order: Vec<_> = inv
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.parent == parent && l.visible)
        .map(|(index, _)| index)
        .collect();
    order.sort_by_key(|&index| {
        let z = inv.cels[request.image_index].get(&index).map_or(0, |c| c.z);
        (index as i32 + z, z)
    });
    for index in order {
        request.check_cancelled()?;
        let l = &inv.layers[index];
        if l.opacity == 0 {
            continue;
        }
        if !l.group && !inv.cels[request.image_index].contains_key(&index) {
            continue;
        }
        if l.blend != 0 {
            return Err(bad("non-normal layer blending unsupported"));
        }
        if l.group {
            if inv.isolate_groups {
                let mut group = vec![0; out.len()];
                render_group(&mut group, Some(index), inv, request, &mut true)?;
                for (n, (dst, src)) in out.chunks_exact_mut(4).zip(group.chunks_exact(4)).enumerate() {
                    if n % 65536 == 0 {
                        request.check_cancelled()?;
                    }
                    over(dst, src.try_into().unwrap(), l.opacity);
                }
                *empty = false;
            } else {
                render_group(out, Some(index), inv, request, empty)?;
            }
        } else if let Some(&cel) = inv.cels[request.image_index].get(&index) {
            if render_cel(out, index, cel, inv, request, *empty)? {
                *empty = false;
            }
        }
    }
    Ok(())
}
pub fn decode_aseprite(request: &DecodeRequest) -> Result<AsepriteImage, RasterDecodeError> {
    request.check_cancelled()?;
    decode(&read_bounded(request)?, request)
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<AsepriteImage, RasterDecodeError> {
    let inv = inventory(b, request)?;
    let mut rgba = vec![0; inv.w * inv.h * 4];
    render_group(&mut rgba, None, &inv, request, &mut true)?;
    Ok(AsepriteImage {
        raster: DecodedRaster::new(
            inv.w as u32,
            inv.h as u32,
            RasterPixels::Rgba8(Arc::new(rgba).into()),
            inv.color,
        )?
        .with_image_selection(request.image_index, inv.cels.len())?,
        delay_ms: inv.delays[request.image_index],
        tags: inv.tags,
        layer_names: inv.layers.into_iter().map(|l| l.name).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
    }
    fn chunks(b: &[u8]) -> Vec<(usize, usize, u16)> {
        let mut out = Vec::new();
        let mut frame = 128;
        while frame < b.len() {
            let end = frame + dword(&b[frame..frame + 4]) as usize;
            let mut at = frame + 16;
            while at < end {
                let n = dword(&b[at..at + 4]) as usize;
                out.push((at, frame, word(&b[at + 4..at + 6])));
                at += n;
            }
            frame = end;
        }
        out
    }
    #[test]
    fn selected_frame_oracles_and_metadata() {
        let root = root();
        let manifest = std::fs::read_to_string(root.join("aseprite-manifest.tsv")).unwrap();
        for row in manifest.lines().filter(|s| !s.starts_with('#')) {
            let c: Vec<_> = row.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(c[0]));
            request.image_index = c[1].parse().unwrap();
            let image = decode_aseprite(&request).unwrap_or_else(|e| panic!("{} frame {}: {e}", c[0], c[1]));
            assert_eq!(
                (image.raster.width(), image.raster.height()),
                (c[2].parse().unwrap(), c[3].parse().unwrap())
            );
            assert_eq!(
                (image.raster.image_index(), image.raster.image_count()),
                (request.image_index, c[4].parse().unwrap())
            );
            assert_eq!(image.delay_ms, c[5].parse::<u32>().unwrap());
            assert_eq!(image.tags.len(), 1);
            let tag = &image.tags[0];
            assert_eq!(
                (
                    &*tag.name,
                    tag.first_frame,
                    tag.last_frame,
                    tag.direction,
                    tag.repeats
                ),
                ("bounce", 0, 2, 2, 3)
            );
            assert!(!image.layer_names.is_empty());
            let RasterPixels::Rgba8(pixels) = image.raster.pixels() else {
                panic!()
            };
            assert_eq!(
                pixels.as_slice(),
                std::fs::read(root.join(c[7])).unwrap(),
                "{} frame {}",
                c[0],
                c[1]
            );
            let common = crate::decode_raster(&request).unwrap();
            let RasterPixels::Rgba8(common_pixels) = common.pixels() else {
                panic!()
            };
            assert_eq!(common_pixels.as_slice(), pixels.as_slice());
            assert!(crate::is_supported_image_path(&request.path));
        }
    }
    #[test]
    fn source_color_reaches_display_without_changing_alpha() {
        for name in ["linear", "icc", "assumed"] {
            let request = DecodeRequest::new(root().join(format!("ase-profile-{name}.aseprite")));
            let image = decode_aseprite(&request).unwrap();
            match name {
                "linear" => assert_eq!(image.raster.color_space(), &RasterColorSpace::LinearSrgb),
                "icc" => assert!(matches!(image.raster.color_space(), RasterColorSpace::Icc(_))),
                _ => assert_eq!(image.raster.color_space(), &RasterColorSpace::AssumedSrgb),
            }
            let frame = crate::prepare_raster_for_display(&image.raster).unwrap();
            let RasterPixels::Rgba32Float(pixels) = frame.pixels() else {
                panic!()
            };
            assert_eq!(pixels[3], 128. / 255.);
            if name == "linear" {
                assert_eq!(pixels[0], 20. / 255.);
            }
        }
    }
    #[test]
    fn unselected_unrelated_compressed_streams_are_not_inflated() {
        let path = root().join("ase-32-1-2019-1.aseprite");
        let mut request = DecodeRequest::new(&path);
        let source = std::fs::read(path).unwrap();
        let at = chunks(&source)
            .into_iter()
            .filter(|&(at, _, kind)| kind == 0x2005 && word(&source[at + 13..at + 15]) == 2)
            .find(|&(at, frame, _)| frame > 128 && word(&source[at + 6..at + 8]) == 1)
            .unwrap()
            .0;
        let mut bytes = source.clone();
        bytes[at + 26] = 0;
        assert!(decode(&bytes, &request).is_ok());
        request.image_index = 1;
        assert!(decode(&bytes, &request).is_err());
        // A structural error anywhere still rejects on first-frame selection.
        bytes = source;
        bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        request.image_index = 0;
        assert!(decode(&bytes, &request).is_err());
    }
    #[test]
    fn cyclic_links_unsupported_blend_and_gamma_reject() {
        let path = root().join("ase-forward-linked-cels.aseprite");
        let request = DecodeRequest::new(&path);
        let mut b = std::fs::read(path).unwrap();
        let at = chunks(&b)
            .into_iter()
            .find(|&(_, frame, kind)| frame == 128 && kind == 0x2005)
            .unwrap()
            .0;
        b[at + 22..at + 24].copy_from_slice(&1u16.to_le_bytes());
        assert!(decode(&b, &request).is_err(), "two-cel cycle");
        let path = root().join("ase-profile-linear.aseprite");
        let request = DecodeRequest::new(&path);
        let source = std::fs::read(path).unwrap();
        let at = chunks(&source)
            .into_iter()
            .find(|&(_, _, kind)| kind == 0x2007)
            .unwrap()
            .0;
        let mut b = source.clone();
        b[at + 10..at + 14].copy_from_slice(&131072u32.to_le_bytes());
        assert!(decode(&b, &request).is_err(), "custom gamma");
        let at = chunks(&source)
            .into_iter()
            .find(|&(_, _, kind)| kind == 0x2004)
            .unwrap()
            .0;
        let mut b = source;
        b[at + 16..at + 18].copy_from_slice(&1u16.to_le_bytes());
        assert!(decode(&b, &request).is_err(), "visible multiply blend");
        for extension in ["ASE", "ASEPRITE"] {
            assert!(crate::is_supported_image_path(std::path::Path::new(&format!(
                "file.{extension}"
            ))));
        }
    }
    #[test]
    fn truncated_and_invalid_fields_reject() {
        let path = root().join("ase-32-1-2019-1.aseprite");
        let mut request = DecodeRequest::new(&path);
        let source = std::fs::read(path).unwrap();
        for n in 0..source.len() {
            assert!(decode(&source[..n], &request).is_err(), "prefix {n}");
        }
        for (at, v) in [(6, 0u16), (8, 0), (10, 0), (12, 24), (132, 0)] {
            let mut b = source.clone();
            b[at..at + 2].copy_from_slice(&v.to_le_bytes());
            assert!(decode(&b, &request).is_err());
        }
        let mut b = source.clone();
        b[34] = 2;
        assert!(decode(&b, &request).is_err());
        let at = chunks(&source)
            .into_iter()
            .find(|&(_, frame, kind)| frame > 128 && kind == 0x2005)
            .unwrap()
            .0;
        let mut b = source.clone();
        b[at + 22..at + 24].copy_from_slice(&1u16.to_le_bytes());
        assert!(decode(&b, &request).is_err(), "self/cyclic link");
        request.image_index = 3;
        assert!(decode(&source, &request).is_err());
        request.image_index = 0;
        request.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        request.path = root().join("missing-cancelled.aseprite");
        assert!(matches!(
            decode_aseprite(&request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
    }
}
