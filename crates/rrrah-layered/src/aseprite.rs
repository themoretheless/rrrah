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
pub fn has_magic(b: &[u8]) -> bool {
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
    // Temporary reverse edge while resolving a linked-cel chain in place.
    Resolving(Option<usize>),
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
    pixel_aspect: Option<f32>,
    transparent: u8,
    isolate_groups: bool,
    layers: Vec<Layer>,
    cels: Vec<BTreeMap<usize, Cel<'a>>>,
    delays: Vec<u32>,
    palette: [Option<[u8; 4]>; 256],
    color: RasterColorSpace,
    color_credit: Option<rrrah_core::Reservation>,
    tags: Vec<AsepriteTag>,
    max_cel_bytes: usize,
    // Declared last so frame tables are freed before their budget credit.
    _frame_index_credit: Option<rrrah_core::Reservation>,
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
fn color_chunk(
    b: &[u8],
    request: &DecodeRequest,
    credit: &mut Option<rrrah_core::Reservation>,
) -> Result<RasterColorSpace, RasterDecodeError> {
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
            *credit = reserve_pixels(request, n)?;
            Ok(RasterColorSpace::Icc(b[20..].to_vec()))
        }
        _ => Err(bad("color profile type/length")),
    }
}
// ASE header bytes 34/35; either zero declares square pixels.
fn pixel_aspect(width: u8, height: u8) -> Option<f32> {
    (width != 0 && height != 0 && width != height).then(|| f32::from(width) / f32::from(height))
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
    let pixel_aspect = pixel_aspect(b[34], b[35]);
    if w as u64 * h as u64 * 16 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if request.image_index >= frames {
        return Err(bad("frame index out of range"));
    }
    if frames > (b.len() - 128) / 16 {
        return Err(bad("frame count exceeds source bounds"));
    }
    let frame_index_bytes =
        frames * (std::mem::size_of::<BTreeMap<usize, Cel<'_>>>() + std::mem::size_of::<u32>());
    let frame_index_credit = reserve_pixels(request, frame_index_bytes)?;
    let mut result = Inventory {
        w,
        h,
        depth: depth / 8,
        pixel_aspect,
        transparent: b[28],
        isolate_groups: flags & 2 != 0,
        layers: Vec::new(),
        cels: Vec::with_capacity(frames),
        delays: Vec::with_capacity(frames),
        palette: [None; 256],
        color: RasterColorSpace::AssumedSrgb,
        color_credit: None,
        tags: Vec::new(),
        max_cel_bytes: 0,
        _frame_index_credit: frame_index_credit,
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
        let frame_bytes = outer.take(size - 16)?;
        // First borrowed pass validates framing and finds palette precedence;
        // the second parses in source order without a chunk descriptor table.
        let mut s = Scan {
            b: frame_bytes,
            at: 0,
        };
        let mut new_palette = false;
        for _ in 0..count {
            request.check_cancelled()?;
            let h = s.take(6)?;
            let n = dword(&h[..4]) as usize;
            if n < 6 {
                return Err(bad("chunk size"));
            }
            new_palette |= word(&h[4..6]) == 0x2019;
            s.take(n - 6)?;
        }
        s.finish()?;
        let mut s = Scan {
            b: frame_bytes,
            at: 0,
        };
        let mut frame_cels = BTreeMap::new();
        for _ in 0..count {
            request.check_cancelled()?;
            let h = s.take(6)?;
            let kind = word(&h[4..6]);
            let data = s.take(dword(&h[..4]) as usize - 6)?;
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
                    result.color = color_chunk(data, request, &mut result.color_credit)?;
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
    resolve_links(&mut result.cels, request)?;
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
// Reverse unresolved edges in place, then unwind with the resolved payload.
// Every link is visited at most twice, without path/key/hash allocations.
fn resolve_links(
    cels: &mut [BTreeMap<usize, Cel<'_>>],
    request: &DecodeRequest,
) -> Result<(), RasterDecodeError> {
    for frame in 0..cels.len() {
        let mut key = cels[frame].keys().next().copied();
        while let Some(layer) = key {
            request.check_cancelled()?;
            let mut previous = None;
            let mut target = frame;
            let image = loop {
                request.check_cancelled()?;
                let cel = cels
                    .get_mut(target)
                    .and_then(|frame| frame.get_mut(&layer))
                    .ok_or_else(|| bad("missing linked cel"))?;
                match cel.data {
                    image @ CelData::Image { .. } => break image,
                    CelData::Link(next) => {
                        cel.data = CelData::Resolving(previous);
                        previous = Some(target);
                        target = next;
                    }
                    CelData::Resolving(_) => return Err(bad("cyclic linked cels")),
                }
            };
            while let Some(target) = previous {
                request.check_cancelled()?;
                let cel = cels[target].get_mut(&layer).unwrap();
                let CelData::Resolving(next) = cel.data else {
                    unreachable!()
                };
                cel.data = image;
                previous = next;
            }
            key = cels[frame]
                .range((std::ops::Bound::Excluded(layer), std::ops::Bound::Unbounded))
                .next()
                .map(|(&key, _)| key);
        }
    }
    Ok(())
}
fn inflate_into(data: &[u8], out: &mut [u8], request: &DecodeRequest) -> Result<(), RasterDecodeError> {
    inflate_with_progress(data, out, request, |_, _| {})
}
fn inflate_with_progress(
    data: &[u8],
    out: &mut [u8],
    request: &DecodeRequest,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), RasterDecodeError> {
    let _decoder_credit = reserve_pixels(request, rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES as usize)?;
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
        progress(decoder.total_in(), decoder.total_out());
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
fn reserve_pixels(
    request: &DecodeRequest,
    bytes: usize,
) -> Result<Option<rrrah_core::Reservation>, RasterDecodeError> {
    request.check_cancelled()?;
    request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(bytes as u64))
        .transpose()
        .map_err(crate::DecodeError::from)
        .map_err(Into::into)
}
fn unpack<'a>(
    data: &'a [u8],
    expected: usize,
    compressed: bool,
    request: &DecodeRequest,
) -> Result<(std::borrow::Cow<'a, [u8]>, Option<rrrah_core::Reservation>), RasterDecodeError> {
    if !compressed {
        return Ok((std::borrow::Cow::Borrowed(data), None));
    }
    let reservation = reserve_pixels(request, expected)?;
    let mut out = vec![0; expected];
    inflate_into(data, &mut out, request)?;
    Ok((std::borrow::Cow::Owned(out), reservation))
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
    // Keep the tuple intact: fields drop in order, pixels before their credit.
    let storage = unpack(data, w * h * inv.depth, compressed, request)?;
    let decoded = storage.0.as_ref();
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
                let _reservation = reserve_pixels(request, out.len())?;
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
pub fn decode(b: &[u8], request: &DecodeRequest) -> Result<AsepriteImage, RasterDecodeError> {
    let inv = inventory(b, request)?;
    let reservation = reserve_pixels(request, inv.w * inv.h * 4)?;
    let mut rgba = vec![0; inv.w * inv.h * 4];
    render_group(&mut rgba, None, &inv, request, &mut true)?;
    request.check_cancelled()?;
    let pixels = match reservation {
        Some(reservation) => reservation
            .try_adopt(rgba)
            .map_err(crate::DecodeError::from)?
            .into(),
        None => Arc::new(rgba).into(),
    };
    let raster = DecodedRaster::new(inv.w as u32, inv.h as u32, RasterPixels::Rgba8(pixels), inv.color)?
        .with_pixel_aspect(inv.pixel_aspect)?
        .with_image_selection(request.image_index, inv.cels.len())?;
    let raster = match inv.color_credit {
        Some(credit) => raster
            .with_color_profile_reservation(credit)
            .map_err(crate::DecodeError::from)?,
        None => raster,
    };
    Ok(AsepriteImage {
        raster,
        delay_ms: inv.delays[request.image_index],
        tags: inv.tags,
        layer_names: inv.layers.into_iter().map(|l| l.name).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_frame_buffers_obey_peak_budget_and_keep_output_credit_until_last_owner() {
        let manifest = include_str!("../../../tests/fixtures/raster/aseprite-manifest.tsv");
        let mut cases = 0;
        let mut scratch_cases = 0;
        for row in manifest
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = row.split('\t').collect();
            let bytes = std::fs::read(root().join(fields[0])).unwrap();
            let mut request = DecodeRequest::new(fields[0]);
            request.image_index = fields[1].parse().unwrap();
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            request.memory_budget = Some(budget.clone());
            let image = decode(&bytes, &request).unwrap();
            let output = image.raster.capacity_bytes();
            let peak = budget.peak();
            assert_eq!(budget.used(), output);
            assert!(peak >= output);
            scratch_cases += usize::from(peak > output);
            let RasterPixels::Rgba8(values) = image.raster.pixels() else {
                panic!()
            };
            assert!(values.is_managed());
            let expected = values.as_slice().to_vec();
            let clone = image.raster.clone();
            drop(image);
            assert_eq!(budget.used(), output);
            drop(clone);
            assert_eq!(budget.used(), 0);
            for limit in [0, output - 1, peak - 1] {
                let budget = rrrah_core::MemoryBudget::new(limit);
                request.memory_budget = Some(budget.clone());
                assert!(
                    matches!(
                        decode(&bytes, &request),
                        Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
                    ),
                    "{} index={} limit={limit}",
                    fields[0],
                    request.image_index
                );
                assert_eq!(budget.used(), 0);
                assert!(budget.peak() <= limit);
            }
            let exact = rrrah_core::MemoryBudget::new(peak);
            request.memory_budget = Some(exact.clone());
            let image = decode(&bytes, &request).unwrap();
            let RasterPixels::Rgba8(values) = image.raster.pixels() else {
                panic!()
            };
            assert_eq!(values.as_slice(), expected);
            assert_eq!(exact.peak(), peak);
            drop(image);
            assert_eq!(exact.used(), 0);
            cases += 1;
        }
        assert!(scratch_cases > 0, "fixtures must exercise scratch admission");
        assert_eq!(cases, 63);
        eprintln!(
            "ASE_BUDGET cases={cases} scratch_cases={scratch_cases} refusals={} final=0",
            cases * 3
        );
    }

    #[test]
    fn corrupt_compressed_cels_release_admitted_buffers_and_allow_retry() {
        let manifest = include_str!("../../../tests/fixtures/raster/aseprite-manifest.tsv");
        let mut cases = 0;
        for row in manifest
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = row.split('\t').collect();
            let original = std::fs::read(root().join(fields[0])).unwrap();
            let compressed: Vec<_> = chunks(&original)
                .into_iter()
                .filter(|&(at, _, kind)| kind == 0x2005 && word(&original[at + 13..at + 15]) == 2)
                .map(|(at, _, _)| at)
                .collect();
            if compressed.is_empty() {
                continue;
            }
            let mut request = DecodeRequest::new(fields[0]);
            request.image_index = fields[1].parse().unwrap();
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            request.memory_budget = Some(budget.clone());
            for checksum in [false, true] {
                let mut broken = original.clone();
                for &at in &compressed {
                    if checksum {
                        let end = at + dword(&original[at..at + 4]) as usize;
                        broken[end - 1] ^= 0xff;
                    } else {
                        broken[at + 26] = 0;
                    }
                }
                let error = decode(&broken, &request).unwrap_err();
                assert!(
                    matches!(
                        error,
                        RasterDecodeError::InvalidAseprite("invalid cel zlib stream")
                    ),
                    "{error}"
                );
                assert_eq!(budget.used(), 0);
                assert!(budget.peak() > 0, "failure must occur after pixel admission");
                let retry = decode(&original, &request).unwrap();
                let RasterPixels::Rgba8(values) = retry.raster.pixels() else {
                    panic!()
                };
                assert_eq!(values.as_slice(), std::fs::read(root().join(fields[7])).unwrap());
                drop(retry);
                assert_eq!(budget.used(), 0);
                cases += 1;
            }
        }
        assert!(cases > 0);
        eprintln!("ASE_CORRUPT cases={cases} retries={cases} final=0");
    }

    #[test]
    fn large_metadata_chunk_count_does_not_allocate_descriptor_table() {
        let original = std::fs::read(root().join("ase-32-0-2019-1.aseprite")).unwrap();
        let mut request = DecodeRequest::new("many-chunks.aseprite");
        let baseline_budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        request.memory_budget = Some(baseline_budget.clone());
        let baseline = decode(&original, &request).unwrap();
        let peak = baseline_budget.peak();
        let RasterPixels::Rgba8(values) = baseline.raster.pixels() else {
            panic!()
        };
        let expected = values.as_slice().to_vec();
        drop(baseline);
        let size = dword(&original[128..132]) as usize;
        let count = if dword(&original[140..144]) == 0 {
            u32::from(word(&original[134..136]))
        } else {
            dword(&original[140..144])
        };
        let extra = 100_000u32;
        let mut bytes = original.clone();
        let chunks: Vec<_> = (0..extra).flat_map(|_| [6, 0, 0, 0, 0x16, 0x20]).collect();
        bytes.splice(128 + size..128 + size, chunks);
        bytes[128..132].copy_from_slice(&(size as u32 + extra * 6).to_le_bytes());
        bytes[140..144].copy_from_slice(&(count + extra).to_le_bytes());
        let length = bytes.len() as u32;
        bytes[..4].copy_from_slice(&length.to_le_bytes());
        let exact = rrrah_core::MemoryBudget::new(peak);
        request.memory_budget = Some(exact.clone());
        let image = decode(&bytes, &request).unwrap();
        let RasterPixels::Rgba8(values) = image.raster.pixels() else {
            panic!()
        };
        assert_eq!(values.as_slice(), expected);
        assert_eq!(exact.peak(), peak);
        drop(image);
        assert_eq!(exact.used(), 0);
        let denied = rrrah_core::MemoryBudget::new(0);
        request.memory_budget = Some(denied.clone());
        assert!(matches!(
            decode(&original, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(denied.peak(), 0);
        assert_eq!(denied.used(), 0);
        eprintln!("ASE_CHUNK_TABLE extra={extra} peak={peak} final=0");
    }

    #[test]
    fn linked_cel_graph_resolves_long_chains_and_rejects_cycles_without_scratch() {
        let samples = [7, 19, 31, 255];
        let image = CelData::Image {
            w: 1,
            h: 1,
            compressed: false,
            data: &samples,
        };
        let cel = |data| Cel {
            x: -3,
            y: 5,
            opacity: 173,
            z: -7,
            data,
        };
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("linked.aseprite");
        request.memory_budget = Some(budget.clone());
        for forward in [false, true] {
            let count = 50_000;
            let mut frames: Vec<BTreeMap<usize, Cel<'_>>> = (0..count)
                .map(|index| {
                    let data = if (forward && index == count - 1) || (!forward && index == 0) {
                        image
                    } else {
                        CelData::Link(if forward { index + 1 } else { index - 1 })
                    };
                    [(2, cel(data)), (9, cel(image))].into_iter().collect()
                })
                .collect();
            resolve_links(&mut frames, &request).unwrap();
            for frame in frames {
                for value in frame.values() {
                    assert_eq!((value.x, value.y, value.opacity, value.z), (-3, 5, 173, -7));
                    let CelData::Image {
                        data,
                        w,
                        h,
                        compressed,
                    } = value.data
                    else {
                        panic!()
                    };
                    assert_eq!((w, h, compressed), (1, 1, false));
                    assert_eq!(data, samples);
                }
            }
        }
        for links in [vec![0], vec![1, 0], vec![1, 2, 1]] {
            let mut frames: Vec<_> = links
                .into_iter()
                .map(|next| [(2, cel(CelData::Link(next)))].into_iter().collect())
                .collect();
            assert!(matches!(
                resolve_links(&mut frames, &request),
                Err(RasterDecodeError::InvalidAseprite("cyclic linked cels"))
            ));
        }
        let mut missing: Vec<_> = vec![
            [(2, cel(CelData::Link(1)))].into_iter().collect(),
            BTreeMap::new(),
        ];
        assert!(matches!(
            resolve_links(&mut missing, &request),
            Err(RasterDecodeError::InvalidAseprite("missing linked cel"))
        ));
        request.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(1)),
            0,
        ));
        let mut cancelled = vec![[(2, cel(image))].into_iter().collect()];
        assert!(matches!(
            resolve_links(&mut cancelled, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(budget.peak(), 0);
        assert_eq!(budget.used(), 0);
        eprintln!("ASE_LINK_GRAPH states=200000 cycles=3 missing=1 cancellation=1 scratch=0");
    }

    #[test]
    fn linked_cel_graph_matches_independent_exhaustive_walk_oracle() {
        let samples = [
            [1, 3, 5, 255],
            [7, 11, 13, 127],
            [17, 19, 23, 91],
            [29, 31, 37, 43],
        ];
        let request = DecodeRequest::new("graph.aseprite");
        let mut cases = 0;
        for count in 1..=5usize {
            let radix = count + 3; // links, two images, one missing target
            for encoded in 0..radix.pow(count as u32) {
                let mut remaining = encoded;
                let edges: Vec<_> = (0..count)
                    .map(|_| {
                        let edge = remaining % radix;
                        remaining /= radix;
                        edge
                    })
                    .collect();
                let expected: Vec<_> = (0..count)
                    .map(|start| {
                        let mut seen = vec![false; count];
                        let mut at = start;
                        loop {
                            if at >= count {
                                break None;
                            }
                            if seen[at] {
                                break None;
                            }
                            seen[at] = true;
                            match edges[at] {
                                edge if edge < count => at = edge,
                                edge if edge < count + 2 => break Some(edge - count),
                                _ => break None,
                            }
                        }
                    })
                    .collect();
                let mut frames: Vec<BTreeMap<usize, Cel<'_>>> = edges
                    .iter()
                    .enumerate()
                    .map(|(index, &edge)| {
                        [2usize, 9]
                            .into_iter()
                            .enumerate()
                            .map(|(channel, layer)| {
                                let data = if edge < count {
                                    CelData::Link(edge)
                                } else if edge < count + 2 {
                                    CelData::Image {
                                        w: 1,
                                        h: 1,
                                        compressed: false,
                                        data: &samples[channel * 2 + edge - count],
                                    }
                                } else {
                                    CelData::Link(count)
                                };
                                (
                                    layer,
                                    Cel {
                                        x: index as i32,
                                        y: -2,
                                        opacity: 173,
                                        z: -7,
                                        data,
                                    },
                                )
                            })
                            .collect()
                    })
                    .collect();
                let result = resolve_links(&mut frames, &request);
                assert_eq!(
                    result.is_ok(),
                    expected.iter().all(Option::is_some),
                    "count={count} graph={encoded}"
                );
                if result.is_ok() {
                    for (index, frame) in frames.iter().enumerate() {
                        for (channel, layer) in [2, 9].into_iter().enumerate() {
                            let cel = frame.get(&layer).unwrap();
                            let CelData::Image { data, .. } = cel.data else {
                                panic!()
                            };
                            assert_eq!(data, samples[channel * 2 + expected[index].unwrap()]);
                            assert_eq!((cel.x, cel.y, cel.opacity, cel.z), (index as i32, -2, 173, -7));
                        }
                    }
                }
                cases += 1;
            }
        }
        eprintln!("ASE_LINK_EXHAUSTIVE graphs={cases} max_frames=5 layers=2");
    }

    #[test]
    #[ignore = "manual paired linked-cel resolver microbenchmark"]
    fn linked_cel_resolver_paired_benchmark() {
        fn previous(cels: &mut [BTreeMap<usize, Cel<'_>>], request: &DecodeRequest) {
            for frame in 0..cels.len() {
                let keys: Vec<_> = cels[frame].keys().copied().collect();
                for layer in keys {
                    request.check_cancelled().unwrap();
                    let mut path = Vec::new();
                    let mut visited = std::collections::HashSet::new();
                    let mut target = frame;
                    let image = loop {
                        request.check_cancelled().unwrap();
                        match cels[target][&layer].data {
                            image @ CelData::Image { .. } => break image,
                            CelData::Link(next) => {
                                assert!(visited.insert(target));
                                path.push(target);
                                target = next;
                            }
                            CelData::Resolving(_) => unreachable!(),
                        }
                    };
                    for target in path {
                        cels[target].get_mut(&layer).unwrap().data = image;
                    }
                }
            }
        }
        let samples = [7, 19, 31, 255];
        let request = DecodeRequest::new("benchmark.aseprite");
        for forward in [false, true] {
            let count = 50_000;
            let input: Vec<BTreeMap<usize, Cel<'_>>> = (0..count)
                .map(|index| {
                    let data = if (forward && index == count - 1) || (!forward && index == 0) {
                        CelData::Image {
                            w: 1,
                            h: 1,
                            compressed: false,
                            data: &samples,
                        }
                    } else {
                        CelData::Link(if forward { index + 1 } else { index - 1 })
                    };
                    [(
                        2,
                        Cel {
                            x: 0,
                            y: 0,
                            opacity: 255,
                            z: 0,
                            data,
                        },
                    )]
                    .into_iter()
                    .collect()
                })
                .collect();
            for pair in 0..8 {
                let mut old = input.clone();
                let mut new = input.clone();
                let (old_ns, new_ns);
                if pair % 2 == 0 {
                    let start = std::time::Instant::now();
                    previous(&mut old, &request);
                    old_ns = start.elapsed().as_nanos();
                    let start = std::time::Instant::now();
                    resolve_links(&mut new, &request).unwrap();
                    new_ns = start.elapsed().as_nanos();
                } else {
                    let start = std::time::Instant::now();
                    resolve_links(&mut new, &request).unwrap();
                    new_ns = start.elapsed().as_nanos();
                    let start = std::time::Instant::now();
                    previous(&mut old, &request);
                    old_ns = start.elapsed().as_nanos();
                }
                for (old, new) in old.iter().zip(&new) {
                    let (CelData::Image { data: a, .. }, CelData::Image { data: b, .. }) =
                        (old[&2].data, new[&2].data)
                    else {
                        panic!()
                    };
                    assert_eq!(a, b);
                }
                eprintln!(
                    "ASE_LINK_PAIR forward={forward} count={count} pair={pair} old_ns={old_ns} new_ns={new_ns}"
                );
            }
        }
    }

    #[test]
    fn file_source_and_decode_peak_share_budget_and_release_source_before_return() {
        let manifest = include_str!("../../../tests/fixtures/raster/aseprite-manifest.tsv");
        let mut cases = 0;
        for row in manifest
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = row.split('\t').collect();
            let path = root().join(fields[0]);
            let bytes = std::fs::read(&path).unwrap();
            let mut request = DecodeRequest::new(path);
            request.image_index = fields[1].parse().unwrap();
            let borrowed_budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            request.memory_budget = Some(borrowed_budget.clone());
            drop(decode(&bytes, &request).unwrap());
            let needed = bytes.len() as u64 + borrowed_budget.peak();
            let exact = rrrah_core::MemoryBudget::new(needed);
            request.memory_budget = Some(exact.clone());
            let image = decode_aseprite(&request).unwrap();
            let output = image.raster.capacity_bytes();
            assert_eq!(exact.peak(), needed);
            assert_eq!(
                exact.used(),
                output,
                "source and parser tables must be released on return"
            );
            let RasterPixels::Rgba8(values) = image.raster.pixels() else {
                panic!()
            };
            assert_eq!(values.as_slice(), std::fs::read(root().join(fields[7])).unwrap());
            let clone = image.raster.clone();
            drop(image);
            assert_eq!(exact.used(), output);
            drop(clone);
            assert_eq!(exact.used(), 0);
            for limit in [bytes.len() as u64 - 1, needed - 1] {
                let denied = rrrah_core::MemoryBudget::new(limit);
                request.memory_budget = Some(denied.clone());
                assert!(
                    matches!(
                        decode_aseprite(&request),
                        Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
                    ),
                    "{} index={} limit={limit}",
                    fields[0],
                    request.image_index
                );
                assert_eq!(denied.used(), 0);
                assert!(denied.peak() <= limit);
            }
            cases += 1;
        }
        assert_eq!(cases, 63);
        eprintln!("ASE_FILE_BUDGET cases={cases} refusals={} final=0", cases * 2);
    }

    #[test]
    fn cel_decoder_state_is_admitted_before_output_writes_and_released() {
        use std::io::Write;
        let pixel = [7u8, 11, 13, 255];
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&pixel).unwrap();
        let compressed = encoder.finish().unwrap();
        let bytes = rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES;
        let mut request = DecodeRequest::new("cel.aseprite");
        let short = rrrah_core::MemoryBudget::new(bytes - 1);
        request.memory_budget = Some(short.clone());
        let mut output = [99u8; 4];
        assert!(matches!(inflate_into(&compressed, &mut output, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(
                rrrah_core::BufferError::Capacity { requested, .. }
            ))) if requested == bytes));
        assert_eq!(output, [99; 4]);
        assert_eq!((short.used(), short.peak()), (0, 0));
        let exact = rrrah_core::MemoryBudget::new(bytes);
        request.memory_budget = Some(exact.clone());
        inflate_into(&compressed, &mut output, &request).unwrap();
        assert_eq!(output, pixel);
        assert_eq!((exact.used(), exact.peak()), (0, bytes));
        let mut broken = compressed;
        *broken.last_mut().unwrap() ^= 255;
        assert!(inflate_into(&broken, &mut output, &request).is_err());
        assert_eq!(exact.used(), 0);
    }

    #[test]
    fn low_output_zlib_stream_checks_cancellation_between_bounded_input_chunks() {
        // Independently authored RFC1950/1951 stream: empty stored blocks,
        // then one final stored block with a four-byte pixel and Adler32.
        let pixel = [7u8, 11, 13, 255];
        let mut stream = vec![0x78, 0x01];
        for _ in 0..100_000 {
            stream.extend_from_slice(&[0, 0, 0, 255, 255]);
        }
        stream.extend_from_slice(&[1, 4, 0, 251, 255]);
        stream.extend_from_slice(&pixel);
        let (mut a, mut b) = (1u32, 0u32);
        for value in pixel {
            a = (a + u32::from(value)) % 65521;
            b = (b + a) % 65521;
        }
        stream.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut output = [0u8; 4];
        let mut request = DecodeRequest::new("empty-blocks.aseprite");
        let (mut previous_in, mut previous_out, mut chunks) = (0, 0, 0);
        inflate_with_progress(&stream, &mut output, &request, |input, output| {
            assert!(input - previous_in <= 65536);
            assert!(output - previous_out <= 65536);
            assert!(input > previous_in || output > previous_out);
            previous_in = input;
            previous_out = output;
            chunks += 1;
        })
        .unwrap();
        assert_eq!(output, pixel);
        assert_eq!(previous_in, stream.len() as u64);
        assert!(chunks >= 8);
        let generation = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let decoder_bytes = rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES;
        let decoder_budget = rrrah_core::MemoryBudget::new(decoder_bytes);
        request.memory_budget = Some(decoder_budget.clone());
        request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 0));
        let mut callbacks = 0;
        output = [0; 4];
        let error = inflate_with_progress(&stream, &mut output, &request, |input, output| {
            assert!(input > 0 && input <= 65536);
            assert_eq!(output, 0);
            assert_eq!(decoder_budget.used(), decoder_bytes);
            callbacks += 1;
            generation.store(1, std::sync::atomic::Ordering::Release);
        })
        .unwrap_err();
        assert!(matches!(
            error,
            RasterDecodeError::Source(crate::DecodeError::Cancelled)
        ));
        assert_eq!(callbacks, 1);
        assert_eq!(decoder_budget.used(), 0);
        assert_eq!(output, [0; 4]);
        request.cancellation = None;
        for prefix in [0, 1, 65535, 65536, 65537, stream.len() - 1] {
            assert!(inflate_into(&stream[..prefix], &mut output, &request).is_err());
        }
        inflate_into(&stream, &mut output, &request).unwrap();
        assert_eq!(output, pixel);
        eprintln!(
            "ASE_INFLATE_BOUNDED source_bytes={} chunks={chunks} cancelled_after_chunks=1 truncations=6 retry=1",
            stream.len()
        );
    }

    #[test]
    fn icc_chunk_admission_and_raster_metadata_clones_share_credit() {
        let bytes = std::fs::read(root().join("ase-profile-icc.aseprite")).unwrap();
        let at = chunks(&bytes)
            .into_iter()
            .find(|&(_, _, kind)| kind == 0x2007)
            .unwrap()
            .0;
        let end = at + dword(&bytes[at..at + 4]) as usize;
        let chunk = &bytes[at + 6..end];
        let profile = &chunk[20..];
        let mut request = DecodeRequest::new("profile.aseprite");
        let denied = rrrah_core::MemoryBudget::new(profile.len() as u64 - 1);
        request.memory_budget = Some(denied.clone());
        let mut credit = None;
        assert!(matches!(
            color_chunk(chunk, &request, &mut credit),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(denied.peak(), 0);
        assert!(credit.is_none());
        let exact = rrrah_core::MemoryBudget::new(profile.len() as u64);
        request.memory_budget = Some(exact.clone());
        let color = color_chunk(chunk, &request, &mut credit).unwrap();
        assert_eq!(color, RasterColorSpace::Icc(profile.to_vec()));
        assert_eq!(exact.used(), profile.len() as u64);
        drop(color);
        drop(credit);
        assert_eq!(exact.used(), 0);
        for index in 0..3 {
            request.image_index = index;
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            request.memory_budget = Some(budget.clone());
            let image = decode(&bytes, &request).unwrap();
            assert_eq!(
                image.raster.color_space(),
                &RasterColorSpace::Icc(profile.to_vec())
            );
            let retained = image.raster.capacity_bytes();
            assert_eq!(retained, 5 * 3 * 4 + profile.len() as u64);
            let clone = image.raster.clone().try_manage_pixels(&budget).unwrap();
            assert_eq!(budget.used(), retained, "ICC must not be admitted twice");
            assert!(std::ptr::eq(image.raster.color_space(), clone.color_space()));
            drop(image);
            assert_eq!(budget.used(), retained);
            drop(clone);
            assert_eq!(budget.used(), 0);
        }
        eprintln!(
            "ASE_ICC_CREDIT frames=3 profile_bytes={} shared_credit=true final=0",
            profile.len()
        );
    }

    #[test]
    fn pixel_aspect_header_domain_and_selected_frames() {
        for width in 0..=255u8 {
            for height in 0..=255u8 {
                let aspect = pixel_aspect(width, height).unwrap_or(1.0);
                let expected = if width == 0 || height == 0 {
                    1.0
                } else {
                    (f64::from(width) / f64::from(height)) as f32
                };
                assert_eq!(aspect, expected);
            }
        }
        let original = std::fs::read(root().join("ase-32-0-2019-1.aseprite")).unwrap();
        let mut request = DecodeRequest::new("aspect.aseprite");
        for index in 0..3 {
            request.image_index = index;
            let baseline = decode(&original, &request).unwrap();
            for (width, height) in [
                (2, 1),
                (1, 2),
                (255, 1),
                (1, 255),
                (7, 3),
                (0, 255),
                (255, 0),
                (0, 0),
                (3, 3),
            ] {
                let mut bytes = original.clone();
                bytes[34] = width;
                bytes[35] = height;
                let actual = decode(&bytes, &request).unwrap();
                assert_eq!(actual.raster.pixel_aspect(), pixel_aspect(width, height));
                assert_eq!(actual.delay_ms, baseline.delay_ms);
                assert_eq!(actual.raster.image_index(), index);
                let (RasterPixels::Rgba8(a), RasterPixels::Rgba8(b)) =
                    (actual.raster.pixels(), baseline.raster.pixels())
                else {
                    panic!()
                };
                assert_eq!(a.as_slice(), b.as_slice());
            }
        }
    }
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
            let RasterPixels::Rgba8(pixels) = image.raster.pixels() else {
                panic!()
            };
            assert_eq!(pixels[3], 128);
            if name == "linear" {
                assert_eq!(pixels[0], 20);
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
