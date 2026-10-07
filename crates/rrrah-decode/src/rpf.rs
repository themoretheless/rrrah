//! Allocation-free inspection of Autodesk Rich Pixel headers and scanline tables.
//! This is structural inspection, not a decoder or proof of channel payload validity.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfInspectError {
    Truncated,
    Revision,
    Window,
    ChannelCount,
    ChannelDepth,
    ProgramFlags,
    RowOffset,
    NextImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpfWindow {
    pub left: i16,
    pub right: i16,
    pub bottom: i16,
    pub top: i16,
}
impl RpfWindow {
    pub fn width(self) -> u32 {
        (i32::from(self.right) - i32::from(self.left) + 1) as u32
    }
    pub fn height(self) -> u32 {
        (i32::from(self.top) - i32::from(self.bottom) + 1) as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpfHeader {
    pub window: RpfWindow,
    pub active_window: RpfWindow,
    pub color_channels: u16,
    pub matte_channels: u16,
    pub auxiliary_channels: u16,
    pub color_bits: u16,
    pub matte_bits: u16,
    pub auxiliary_bits: u16,
    pub storage_type: u16,
    pub matte_type: u16,
    pub auxiliary_type: u16,
    /// Bits follow Autodesk's ZEOUNRCBIGTVWM channel ordering.
    pub gbuffer_channels: u16,
    pub layer_data: bool,
    pub render_info: bool,
    pub node_names: bool,
    pub premultiplied_alpha: bool,
    pub next_image_offset: u32,
}

/// A borrowed table avoids allocating from dimensions supplied by an untrusted file.
#[derive(Debug)]
pub struct RpfInspection<'a> {
    pub header: RpfHeader,
    offsets: &'a [u8],
}
impl RpfInspection<'_> {
    /// Table order is file order; image orientation is a later decoding concern.
    pub fn row_offsets(&self) -> impl ExactSizeIterator<Item = u32> + '_ {
        self.offsets
            .chunks_exact(4)
            .map(|v| u32::from_be_bytes(v.try_into().unwrap()))
    }
}

pub fn inspect_rpf(bytes: &[u8]) -> Result<RpfInspection<'_>, RpfInspectError> {
    use RpfInspectError as E;
    let h = bytes.get(..740).ok_or(E::Truncated)?;
    let word = |at| u16::from_be_bytes([h[at], h[at + 1]]);
    if word(26) != 0xfffd {
        return Err(E::Revision);
    }
    let window = |at| RpfWindow {
        left: word(at) as i16,
        right: word(at + 2) as i16,
        bottom: word(at + 4) as i16,
        top: word(at + 6) as i16,
    };
    let full = window(0);
    let active = window(8);
    if [full, active]
        .iter()
        .any(|w| w.left > w.right || w.bottom > w.top)
        || active.left < full.left
        || active.right > full.right
        || active.bottom < full.bottom
        || active.top > full.top
    {
        return Err(E::Window);
    }
    let color_channels = word(20);
    let matte_channels = word(22);
    let auxiliary_channels = word(24);
    // Header fields are signed shorts. Preserve multispectral channel declarations.
    if color_channels == 0
        || [color_channels, matte_channels, auxiliary_channels]
            .iter()
            .any(|n| *n > i16::MAX as u16)
    {
        return Err(E::ChannelCount);
    }
    let color_bits = word(658);
    let matte_bits = word(662);
    if !matches!(color_bits, 8 | 16 | 32) || (matte_channels > 0 && !matches!(matte_bits, 8 | 16 | 32)) {
        return Err(E::ChannelDepth);
    }
    let program = &h[400..464];
    let program = &program[..program.iter().position(|v| *v == 0).unwrap_or(program.len())];
    let open = program.iter().position(|v| *v == b'(').ok_or(E::ProgramFlags)?;
    let close = program[open + 1..]
        .iter()
        .position(|v| *v == b')')
        .ok_or(E::ProgramFlags)?
        + open
        + 1;
    let mut header = RpfHeader {
        window: full,
        active_window: active,
        color_channels,
        matte_channels,
        auxiliary_channels,
        color_bits,
        matte_bits,
        auxiliary_bits: word(666),
        storage_type: word(18),
        matte_type: word(660),
        auxiliary_type: word(664),
        gbuffer_channels: 0,
        layer_data: false,
        render_info: false,
        node_names: false,
        premultiplied_alpha: true,
        next_image_offset: u32::from_be_bytes(h[736..740].try_into().unwrap()),
    };
    for flag in &program[open + 1..close] {
        if let Some(i) = b"ZEOUNRCBIGTVWM".iter().position(|v| v == flag) {
            header.gbuffer_channels |= 1 << i;
        } else {
            match flag {
                b'L' => header.layer_data = true,
                b'P' => header.render_info = true,
                b'D' => header.node_names = true,
                b'A' => header.premultiplied_alpha = false,
                b' ' | b'\t' => (),
                _ => return Err(E::ProgramFlags),
            }
        }
    }
    if (header.layer_data || header.node_names) && header.gbuffer_channels == 0 {
        return Err(E::ProgramFlags);
    }
    let table_end = 740 + active.height() as usize * 4;
    let offsets = bytes.get(740..table_end).ok_or(E::Truncated)?;
    let next = header.next_image_offset as usize;
    if next != 0 && (next < table_end || next >= bytes.len()) {
        return Err(E::NextImage);
    }
    let payload_end = if next == 0 { bytes.len() } else { next };
    for row in offsets.chunks_exact(4) {
        let offset = u32::from_be_bytes(row.try_into().unwrap()) as usize;
        if offset < table_end || offset >= payload_end {
            return Err(E::RowOffset);
        }
    }
    Ok(RpfInspection { header, offsets })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 754];
        for (at, value) in [
            (2, 1u16),
            (6, 1),
            (10, 1),
            (14, 1),
            (20, 3),
            (22, 1),
            (26, 0xfffd),
            (658, 32),
            (662, 32),
        ] {
            b[at..at + 2].copy_from_slice(&value.to_be_bytes());
        }
        let program = b"3ds max : ( Z E O U N R C B I G T V W M L P D A )";
        b[400..400 + program.len()].copy_from_slice(program);
        b[740..744].copy_from_slice(&752u32.to_be_bytes());
        b[744..748].copy_from_slice(&748u32.to_be_bytes());
        b
    }
    #[test]
    fn preserves_all_channel_flags_and_unsorted_rows() {
        let b = fixture();
        let view = inspect_rpf(&b).unwrap();
        assert_eq!(view.header.gbuffer_channels, 0x3fff);
        assert!(view.header.layer_data && view.header.render_info && view.header.node_names);
        assert!(!view.header.premultiplied_alpha);
        assert_eq!(view.header.active_window.width(), 2);
        assert_eq!(view.row_offsets().collect::<Vec<_>>(), [752, 748]);
    }
    #[test]
    fn every_truncated_header_or_table_is_refused() {
        let b = fixture();
        for end in 0..748 {
            assert!(inspect_rpf(&b[..end]).is_err(), "{end}");
        }
    }
    #[test]
    fn malformed_dimensions_counts_flags_offsets_and_next_are_refused() {
        let b = fixture();
        for (at, replacement, expected) in [
            (26, 0xfffeu16, RpfInspectError::Revision),
            (8, 2, RpfInspectError::Window),
            (20, 0, RpfInspectError::ChannelCount),
            (24, 0xffff, RpfInspectError::ChannelCount),
            (658, 12, RpfInspectError::ChannelDepth),
        ] {
            let mut bad = b.clone();
            bad[at..at + 2].copy_from_slice(&replacement.to_be_bytes());
            assert_eq!(inspect_rpf(&bad).unwrap_err(), expected);
        }
        for offset in [0u32, 740, 747, 754, u32::MAX] {
            let mut bad = b.clone();
            bad[740..744].copy_from_slice(&offset.to_be_bytes());
            assert_eq!(inspect_rpf(&bad).unwrap_err(), RpfInspectError::RowOffset);
        }
        let mut bad = b.clone();
        bad[736..740].copy_from_slice(&753u32.to_be_bytes());
        bad[740..744].copy_from_slice(&753u32.to_be_bytes());
        assert_eq!(inspect_rpf(&bad).unwrap_err(), RpfInspectError::RowOffset);
        bad[736..740].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(inspect_rpf(&bad).unwrap_err(), RpfInspectError::NextImage);
        bad = b.clone();
        bad[412] = b'?';
        assert_eq!(inspect_rpf(&bad).unwrap_err(), RpfInspectError::ProgramFlags);
    }
}

/// Per-sample widths in Autodesk's G-buffer channel order.
pub const RPF_GBUFFER_SAMPLE_BYTES: [usize; 14] = [4, 1, 2, 8, 4, 4, 1, 3, 2, 3, 3, 8, 3, 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfPlaneError {
    Layout,
    Truncated,
    RunOverflow,
    TrailingBytes,
    Cancelled,
}

/// Decode a G-buffer channel or layer channel into caller-owned storage.
/// Autodesk writes one record per byte plane, highest byte first, reconstructing
/// producer memory bytes in reverse plane order. This preserves packed normals,
/// RGBE, vectors and identifiers without converting them into display colors.
/// Each record uses a big-endian u16 length for image rows or u32 for layer rows.
/// Validation finishes before writes; cancellation during writes may leave a partial output.
pub fn decode_rpf_byte_planes(
    input: &[u8],
    output: &mut [u8],
    sample_bytes: usize,
    layer_lengths: bool,
    mut cancelled: impl FnMut() -> bool,
) -> Result<usize, RpfPlaneError> {
    use RpfPlaneError as E;
    if !(1..=8).contains(&sample_bytes) || output.len() % sample_bytes != 0 {
        return Err(E::Layout);
    }
    let samples = output.len() / sample_bytes;
    let prefix = if layer_lengths { 4 } else { 2 };
    let mut total = 0usize;
    // Two passes avoid publishing partially decoded output on malformed records.
    for write in [false, true] {
        let mut at = 0usize;
        for plane in (0..sample_bytes).rev() {
            if cancelled() {
                return Err(E::Cancelled);
            }
            let length_bytes = input.get(at..at + prefix).ok_or(E::Truncated)?;
            let length = if layer_lengths {
                u32::from_be_bytes(length_bytes.try_into().unwrap()) as usize
            } else {
                u16::from_be_bytes(length_bytes.try_into().unwrap()) as usize
            };
            at += prefix;
            let end = at.checked_add(length).ok_or(E::Truncated)?;
            let record = input.get(at..end).ok_or(E::Truncated)?;
            let mut pos = 0usize;
            let mut x = 0usize;
            while x < samples {
                if cancelled() {
                    return Err(E::Cancelled);
                }
                let control = *record.get(pos).ok_or(E::Truncated)? as i8;
                pos += 1;
                let run = if control >= 0 {
                    control as usize + 1
                } else {
                    control.unsigned_abs() as usize
                };
                if run > samples - x {
                    return Err(E::RunOverflow);
                }
                if control >= 0 {
                    let value = *record.get(pos).ok_or(E::Truncated)?;
                    pos += 1;
                    if write {
                        for i in x..x + run {
                            output[i * sample_bytes + plane] = value;
                        }
                    }
                } else {
                    let values = record.get(pos..pos + run).ok_or(E::Truncated)?;
                    if write {
                        for (i, value) in values.iter().enumerate() {
                            output[(x + i) * sample_bytes + plane] = *value;
                        }
                    }
                    pos += run;
                }
                x += run;
            }
            if pos != record.len() {
                return Err(E::TrailingBytes);
            }
            at = end;
        }
        total = at;
    }
    Ok(total)
}

#[cfg(test)]
mod plane_tests {
    use super::*;
    #[test]
    fn all_gbuffer_sizes_restore_exact_memory_bytes_for_both_record_lengths() {
        for size in RPF_GBUFFER_SAMPLE_BYTES {
            for layer in [false, true] {
                let expected: Vec<_> = (0..129 * size).map(|i| (i * 37) as u8).collect();
                let mut input = Vec::new();
                for plane in (0..size).rev() {
                    let mut record = vec![128]; // signed -128: 128 literal samples
                    record.extend((0..128).map(|i| expected[i * size + plane]));
                    record.extend([0, expected[128 * size + plane]]); // one repeated sample
                    if layer {
                        input.extend((record.len() as u32).to_be_bytes());
                    } else {
                        input.extend((record.len() as u16).to_be_bytes());
                    }
                    input.extend(record);
                }
                input.extend([99, 88]); // next channel is left unread
                let mut output = vec![0; expected.len()];
                assert_eq!(
                    decode_rpf_byte_planes(&input, &mut output, size, layer, || false).unwrap(),
                    input.len() - 2
                );
                assert_eq!(output, expected);
            }
        }
    }
    #[test]
    fn malformed_records_leave_destination_unchanged() {
        for input in [vec![0], vec![0, 2, 3, 7], vec![0, 2, 255, 7], vec![0, 3, 0, 7, 8]] {
            let mut output = [42u8; 2];
            assert!(decode_rpf_byte_planes(&input, &mut output, 1, false, || false).is_err());
            assert_eq!(output, [42, 42]);
        }
        let mut output = [42u8; 1];
        assert_eq!(
            decode_rpf_byte_planes(&[], &mut output, 1, true, || true),
            Err(RpfPlaneError::Cancelled)
        );
        assert_eq!(output, [42]);
    }
}

/// Decode one main color/matte record. Float32 payloads are big-endian IEEE-754,
/// uncompressed, and retain their exact bits (including HDR and non-finite values).
/// Integer normalization follows Autodesk's reader: u8/256 and u16/65536.
/// A 16-bit record contains two RLE planes inside a single u16-length record.
/// No color space or alpha conversion is inferred here.
pub fn decode_rpf_color_channel(
    input: &[u8],
    output: &mut [f32],
    bits: u16,
    mut cancelled: impl FnMut() -> bool,
) -> Result<usize, RpfPlaneError> {
    use RpfPlaneError as E;
    if !matches!(bits, 8 | 16 | 32) {
        return Err(E::Layout);
    }
    if cancelled() {
        return Err(E::Cancelled);
    }
    let length = u16::from_be_bytes(input.get(..2).ok_or(E::Truncated)?.try_into().unwrap()) as usize;
    let payload = input.get(2..2 + length).ok_or(E::Truncated)?;
    if bits == 32 {
        if output.len().checked_mul(4) != Some(length) {
            return Err(E::Layout);
        }
        for (dst, src) in output.iter_mut().zip(payload.chunks_exact(4)) {
            if cancelled() {
                return Err(E::Cancelled);
            }
            *dst = f32::from_bits(u32::from_be_bytes(src.try_into().unwrap()));
        }
    } else {
        // Validate both planes before writing either one.
        for write in [false, true] {
            let mut at = 0usize;
            for plane in 0..bits / 8 {
                let mut x = 0usize;
                while x < output.len() {
                    if cancelled() {
                        return Err(E::Cancelled);
                    }
                    let control = *payload.get(at).ok_or(E::Truncated)? as i8;
                    at += 1;
                    let n = if control >= 0 {
                        control as usize + 1
                    } else {
                        control.unsigned_abs() as usize
                    };
                    if n > output.len() - x {
                        return Err(E::RunOverflow);
                    }
                    let count = if control >= 0 { 1 } else { n };
                    let values = payload.get(at..at + count).ok_or(E::Truncated)?;
                    if write {
                        for i in 0..n {
                            let value = values[if control >= 0 { 0 } else { i }] as f32;
                            if plane == 0 {
                                output[x + i] = value / 256.0;
                            } else {
                                output[x + i] += value / 65536.0;
                            }
                        }
                    }
                    at += count;
                    x += n;
                }
            }
            if at != payload.len() {
                return Err(E::TrailingBytes);
            }
        }
    }
    Ok(2 + length)
}

#[cfg(test)]
mod color_tests {
    use super::*;
    #[test]
    fn float_bits_hdr_signed_zero_and_nan_are_preserved() {
        let values = [
            0u32,
            0x80000000,
            1.0f32.to_bits(),
            64.0f32.to_bits(),
            (-4.0f32).to_bits(),
            0x7fc01234,
            0x7f800000,
        ];
        let mut input = ((values.len() * 4) as u16).to_be_bytes().to_vec();
        for value in values {
            input.extend(value.to_be_bytes());
        }
        input.push(9);
        let mut output = [0.0; 7];
        assert_eq!(
            decode_rpf_color_channel(&input, &mut output, 32, || false).unwrap(),
            input.len() - 1
        );
        assert_eq!(output.map(f32::to_bits), values);
    }
    #[test]
    fn integer_planes_share_one_record_and_match_sdk_normalization() {
        let mut out = [0.0; 3];
        decode_rpf_color_channel(&[0, 4, 253, 0, 128, 255], &mut out, 8, || false).unwrap();
        assert_eq!(out, [0.0, 0.5, 255.0 / 256.0]);
        decode_rpf_color_channel(&[0, 8, 253, 0, 128, 255, 253, 255, 1, 255], &mut out, 16, || {
            false
        })
        .unwrap();
        assert_eq!(out, [255.0 / 65536.0, 32769.0 / 65536.0, 65535.0 / 65536.0]);
    }
    #[test]
    fn malformed_low_plane_or_float_length_does_not_modify_output() {
        let mut out = [42.0; 2];
        assert!(decode_rpf_color_channel(&[0, 4, 1, 255, 255, 0], &mut out, 16, || false).is_err());
        assert_eq!(out, [42.0; 2]);
        assert_eq!(
            decode_rpf_color_channel(&[0, 4, 0, 0, 0, 0], &mut out, 32, || false),
            Err(RpfPlaneError::Layout)
        );
        assert_eq!(out, [42.0; 2]);
        assert_eq!(
            decode_rpf_color_channel(&[], &mut out, 32, || true),
            Err(RpfPlaneError::Cancelled)
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfLayerError {
    Count,
    CountLimit,
    ChannelMask,
    Plane(RpfPlaneError),
}

/// Borrowed, validated encoded records. Buffers can be admitted to the root memory
/// budget using `records * sample_bytes` before any channel is expanded.
#[derive(Debug)]
pub struct RpfLayerRecords<'a> {
    pub records: u32,
    pub x_planes: &'a [u8],
    pub channels: [Option<&'a [u8]>; 14],
    pub consumed: usize,
}

impl RpfLayerRecords<'_> {
    /// Exact byte count of expanded X coordinates and present channel arrays.
    /// Container overhead and other image rows require separate budget admission.
    pub fn expanded_bytes(&self) -> u64 {
        let stride = 2 + self
            .channels
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_some())
            .map(|(i, _)| RPF_GBUFFER_SAMPLE_BYTES[i] as u64)
            .sum::<u64>();
        u64::from(self.records) * stride
    }
}

/// Inspect the layer suffix of a modern Autodesk scanline. Unlike record lengths,
/// the signed layer count is stored in native Windows little-endian order.
/// Caller supplies a record cap; no allocation depends on the file's count.
pub fn inspect_rpf_layer_records(
    input: &[u8],
    channel_mask: u16,
    max_records: u32,
    mut cancelled: impl FnMut() -> bool,
) -> Result<RpfLayerRecords<'_>, RpfLayerError> {
    use RpfLayerError as E;
    if channel_mask & !0x3fff != 0 || channel_mask == 0 {
        return Err(E::ChannelMask);
    }
    if cancelled() {
        return Err(E::Plane(RpfPlaneError::Cancelled));
    }
    let count = i32::from_le_bytes(input.get(..4).ok_or(E::Count)?.try_into().unwrap());
    if count < 0 {
        return Err(E::Count);
    }
    let count = count as u32;
    if count > max_records {
        return Err(E::CountLimit);
    }
    let mut view = RpfLayerRecords {
        records: count,
        x_planes: &[],
        channels: [None; 14],
        consumed: 4,
    };
    if count == 0 {
        return Ok(view);
    }
    let mut take = |size: usize| -> Result<&[u8], E> {
        let start = view.consumed;
        let mut at = start;
        for _ in 0..size {
            if cancelled() {
                return Err(E::Plane(RpfPlaneError::Cancelled));
            }
            let len = u32::from_be_bytes(
                input
                    .get(at..at + 4)
                    .ok_or(E::Plane(RpfPlaneError::Truncated))?
                    .try_into()
                    .unwrap(),
            ) as usize;
            at += 4;
            let end = at.checked_add(len).ok_or(E::Plane(RpfPlaneError::Truncated))?;
            let encoded = input.get(at..end).ok_or(E::Plane(RpfPlaneError::Truncated))?;
            let mut pos = 0usize;
            let mut decoded = 0u32;
            while decoded < count {
                if cancelled() {
                    return Err(E::Plane(RpfPlaneError::Cancelled));
                }
                let control = *encoded.get(pos).ok_or(E::Plane(RpfPlaneError::Truncated))? as i8;
                pos += 1;
                let n = if control >= 0 {
                    control as u32 + 1
                } else {
                    control.unsigned_abs() as u32
                };
                if n > count - decoded {
                    return Err(E::Plane(RpfPlaneError::RunOverflow));
                }
                let bytes = if control >= 0 { 1 } else { n as usize };
                encoded
                    .get(pos..pos + bytes)
                    .ok_or(E::Plane(RpfPlaneError::Truncated))?;
                pos += bytes;
                decoded += n;
            }
            if pos != encoded.len() {
                return Err(E::Plane(RpfPlaneError::TrailingBytes));
            }
            at = end;
        }
        view.consumed = at;
        Ok(&input[start..at])
    };
    view.x_planes = take(2)?;
    for (i, size) in RPF_GBUFFER_SAMPLE_BYTES.iter().enumerate() {
        if channel_mask & (1 << i) != 0 {
            view.channels[i] = Some(take(*size)?);
        }
    }
    Ok(view)
}

#[cfg(test)]
mod layer_tests {
    use super::*;
    fn fixture(mask: u16) -> Vec<u8> {
        let mut b = 3i32.to_le_bytes().to_vec();
        for size in std::iter::once(2).chain(
            RPF_GBUFFER_SAMPLE_BYTES
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, n)| *n),
        ) {
            for plane in (0..size).rev() {
                b.extend(4u32.to_be_bytes());
                b.extend([253, plane as u8, plane as u8 + 1, plane as u8 + 2]);
            }
        }
        b
    }
    #[test]
    fn complete_layer_inventory_is_borrowed_and_decodes_exactly() {
        let mut b = fixture(0x3fff);
        let size = b.len();
        b.extend([9, 8]);
        let view = inspect_rpf_layer_records(&b, 0x3fff, 3, || false).unwrap();
        assert_eq!(view.records, 3);
        assert_eq!(
            view.expanded_bytes(),
            3 * (2 + RPF_GBUFFER_SAMPLE_BYTES.iter().sum::<usize>()) as u64
        );
        assert_eq!(view.consumed, size);
        for (i, channel) in view.channels.iter().enumerate() {
            let size = RPF_GBUFFER_SAMPLE_BYTES[i];
            let mut output = vec![0; 3 * size];
            let channel = channel.unwrap();
            assert_eq!(
                decode_rpf_byte_planes(channel, &mut output, size, true, || false).unwrap(),
                channel.len()
            );
            for sample in 0..3 {
                for plane in 0..size {
                    assert_eq!(output[sample * size + plane], (sample + plane) as u8);
                }
            }
        }
        let mut x = [0; 6];
        decode_rpf_byte_planes(view.x_planes, &mut x, 2, true, || false).unwrap();
        assert_eq!(x, [0, 1, 1, 2, 2, 3]);
    }
    #[test]
    fn counts_limits_zero_truncation_and_cancellation() {
        assert_eq!(
            inspect_rpf_layer_records(&(-1i32).to_le_bytes(), 1, 10, || false).unwrap_err(),
            RpfLayerError::Count
        );
        assert_eq!(
            inspect_rpf_layer_records(&i32::MAX.to_le_bytes(), 1, 10, || false).unwrap_err(),
            RpfLayerError::CountLimit
        );
        let zero = 0i32.to_le_bytes();
        let v = inspect_rpf_layer_records(&zero, 1, 0, || false).unwrap();
        assert_eq!(v.consumed, 4);
        assert!(v.channels.iter().all(Option::is_none));
        let b = fixture(1);
        for n in 0..b.len() {
            assert!(inspect_rpf_layer_records(&b[..n], 1, 3, || false).is_err(), "{n}");
        }
        assert_eq!(
            inspect_rpf_layer_records(&b, 1, 2, || false).unwrap_err(),
            RpfLayerError::CountLimit
        );
        let mut calls = 0;
        assert_eq!(
            inspect_rpf_layer_records(&b, 1, 3, || {
                calls += 1;
                calls == 4
            })
            .unwrap_err(),
            RpfLayerError::Plane(RpfPlaneError::Cancelled)
        );
        assert_eq!(
            inspect_rpf_layer_records(&b, 0x8000, 3, || false).unwrap_err(),
            RpfLayerError::ChannelMask
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfLayerCoordinateError {
    Plane(RpfPlaneError),
    OutOfBounds { record: u32, x: u16 },
    Unsorted { record: u32, previous: u16, x: u16 },
}

struct PlaneValues<'a> {
    data: &'a [u8],
    at: usize,
    remaining: usize,
    repeat: Option<u8>,
}
impl PlaneValues<'_> {
    fn next(&mut self) -> Result<u8, RpfPlaneError> {
        if self.remaining == 0 {
            let c = *self.data.get(self.at).ok_or(RpfPlaneError::Truncated)? as i8;
            self.at += 1;
            self.remaining = if c >= 0 {
                c as usize + 1
            } else {
                c.unsigned_abs() as usize
            };
            self.repeat = if c >= 0 {
                let value = *self.data.get(self.at).ok_or(RpfPlaneError::Truncated)?;
                self.at += 1;
                Some(value)
            } else {
                None
            };
        }
        self.remaining -= 1;
        if let Some(value) = self.repeat {
            Ok(value)
        } else {
            let value = *self.data.get(self.at).ok_or(RpfPlaneError::Truncated)?;
            self.at += 1;
            Ok(value)
        }
    }
    fn exhausted(&self) -> bool {
        self.remaining == 0 && self.at == self.data.len()
    }
}

impl RpfLayerRecords<'_> {
    /// Validate relative X coordinates before allocating layer arrays. Repeated X
    /// values represent multiple layers at a pixel and must remain in producer order.
    pub fn validate_coordinates(
        &self,
        width: u32,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<(), RpfLayerCoordinateError> {
        use RpfLayerCoordinateError as E;
        let mut at = 0usize;
        let mut take = || -> Result<PlaneValues<'_>, E> {
            let size = u32::from_be_bytes(
                self.x_planes
                    .get(at..at + 4)
                    .ok_or(E::Plane(RpfPlaneError::Truncated))?
                    .try_into()
                    .unwrap(),
            ) as usize;
            at += 4;
            let end = at.checked_add(size).ok_or(E::Plane(RpfPlaneError::Truncated))?;
            let data = self
                .x_planes
                .get(at..end)
                .ok_or(E::Plane(RpfPlaneError::Truncated))?;
            at = end;
            Ok(PlaneValues {
                data,
                at: 0,
                remaining: 0,
                repeat: None,
            })
        };
        if self.records == 0 {
            if cancelled() {
                return Err(E::Plane(RpfPlaneError::Cancelled));
            }
            return if self.x_planes.is_empty() {
                Ok(())
            } else {
                Err(E::Plane(RpfPlaneError::TrailingBytes))
            };
        }
        let mut high = take()?;
        let mut low = take()?;
        if at != self.x_planes.len() {
            return Err(E::Plane(RpfPlaneError::TrailingBytes));
        }
        let mut previous = 0;
        for record in 0..self.records {
            if cancelled() {
                return Err(E::Plane(RpfPlaneError::Cancelled));
            }
            let x = u16::from_be_bytes([high.next().map_err(E::Plane)?, low.next().map_err(E::Plane)?]);
            if u32::from(x) >= width {
                return Err(E::OutOfBounds { record, x });
            }
            if record > 0 && x < previous {
                return Err(E::Unsorted { record, previous, x });
            }
            previous = x;
        }
        if !high.exhausted() || !low.exhausted() {
            return Err(E::Plane(RpfPlaneError::TrailingBytes));
        }
        Ok(())
    }
}

#[cfg(test)]
mod coordinate_tests {
    use super::*;
    fn x_data(values: &[u16]) -> Vec<u8> {
        let mut out = Vec::new();
        for shift in [8, 0] {
            out.extend(((values.len() + 1) as u32).to_be_bytes());
            out.push((-(values.len() as i8)) as u8);
            out.extend(values.iter().map(|v| (v >> shift) as u8));
        }
        out
    }
    #[test]
    fn duplicate_layers_and_coordinates_above_one_byte_remain_ordered() {
        let data = x_data(&[0, 7, 7, 255, 256, 65535]);
        let view = RpfLayerRecords {
            records: 6,
            x_planes: &data,
            channels: [None; 14],
            consumed: 0,
        };
        assert_eq!(view.validate_coordinates(65536, || false), Ok(()));
        assert_eq!(
            view.validate_coordinates(65535, || false),
            Err(RpfLayerCoordinateError::OutOfBounds { record: 5, x: 65535 })
        );
        assert_eq!(
            view.validate_coordinates(0, || false),
            Err(RpfLayerCoordinateError::OutOfBounds { record: 0, x: 0 })
        );
        let mut calls = 0;
        assert_eq!(
            view.validate_coordinates(65536, || {
                calls += 1;
                calls == 3
            }),
            Err(RpfLayerCoordinateError::Plane(RpfPlaneError::Cancelled))
        );
    }
    #[test]
    fn descending_and_malformed_streams_are_rejected() {
        let data = x_data(&[256, 255]);
        let view = RpfLayerRecords {
            records: 2,
            x_planes: &data,
            channels: [None; 14],
            consumed: 0,
        };
        assert_eq!(
            view.validate_coordinates(300, || false),
            Err(RpfLayerCoordinateError::Unsorted {
                record: 1,
                previous: 256,
                x: 255
            })
        );
        for end in 0..data.len() {
            let view = RpfLayerRecords {
                records: 2,
                x_planes: &data[..end],
                channels: [None; 14],
                consumed: 0,
            };
            assert!(view.validate_coordinates(300, || false).is_err());
        }
    }
}

#[derive(Debug)]
pub enum RpfLayerDecodeError {
    Coordinates(RpfLayerCoordinateError),
    Plane(RpfPlaneError),
    Memory(rrrah_core::BufferError),
}

/// One root-budget-owned allocation contains all expanded layer payloads.
/// Clones share ownership; cache eviction does not release memory still in use.
#[derive(Debug, Clone)]
pub struct RpfDecodedLayers {
    pub records: u32,
    bytes: rrrah_core::SharedBuffer<u8>,
    x_end: usize,
    ranges: [Option<(usize, usize)>; 14],
}
impl RpfDecodedLayers {
    pub fn x_bytes(&self) -> &[u8] {
        &self.bytes[..self.x_end]
    }
    pub fn channel_bytes(&self, channel: usize) -> Option<&[u8]> {
        let (start, end) = self.ranges.get(channel).copied().flatten()?;
        Some(&self.bytes[start..end])
    }
    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }
}

impl RpfLayerRecords<'_> {
    /// Validate coordinates, admit the entire payload once, then expand directly
    /// into its final allocation. Errors and cancellation release the reservation.
    pub fn decode_with_budget(
        &self,
        width: u32,
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<RpfDecodedLayers, RpfLayerDecodeError> {
        use RpfLayerDecodeError as E;
        self.validate_coordinates(width, &mut cancelled)
            .map_err(E::Coordinates)?;
        let total = usize::try_from(self.expanded_bytes())
            .map_err(|_| E::Memory(rrrah_core::BufferError::Overflow))?;
        let mut bytes = budget.try_buffer(total, 0u8).map_err(E::Memory)?;
        let x_end = self.records as usize * 2;
        let mut at = x_end;
        let mut ranges = [None; 14];
        if self.records > 0 {
            let consumed =
                decode_rpf_byte_planes(self.x_planes, &mut bytes[..x_end], 2, true, &mut cancelled)
                    .map_err(E::Plane)?;
            if consumed != self.x_planes.len() {
                return Err(E::Plane(RpfPlaneError::TrailingBytes));
            }
            for (i, encoded) in self.channels.iter().enumerate() {
                if let Some(encoded) = encoded {
                    let size = self.records as usize * RPF_GBUFFER_SAMPLE_BYTES[i];
                    let end = at + size;
                    let consumed = decode_rpf_byte_planes(
                        encoded,
                        &mut bytes[at..end],
                        RPF_GBUFFER_SAMPLE_BYTES[i],
                        true,
                        &mut cancelled,
                    )
                    .map_err(E::Plane)?;
                    if consumed != encoded.len() {
                        return Err(E::Plane(RpfPlaneError::TrailingBytes));
                    }
                    ranges[i] = Some((at, end));
                    at = end;
                }
            }
        }
        if cancelled() {
            return Err(E::Plane(RpfPlaneError::Cancelled));
        }
        Ok(RpfDecodedLayers {
            records: self.records,
            bytes: bytes.freeze(),
            x_end,
            ranges,
        })
    }
}

#[cfg(test)]
mod layer_budget_tests {
    use super::*;
    #[test]
    fn root_admission_aliases_failure_cancellation_and_retry() {
        // Two layers at X=1, depth producer memory bits 1.0f32 and 2.0f32.
        let mut x = Vec::new();
        for v in [0u8, 1] {
            x.extend(2u32.to_be_bytes());
            x.extend([1, v]);
        }
        let mut z = Vec::new();
        for pair in [[63u8, 64], [128, 0], [0, 0], [0, 0]] {
            z.extend(3u32.to_be_bytes());
            z.extend([254, pair[0], pair[1]]);
        }
        let view = RpfLayerRecords {
            records: 2,
            x_planes: &x,
            channels: std::array::from_fn(|i| if i == 0 { Some(z.as_slice()) } else { None }),
            consumed: 0,
        };
        let root = rrrah_core::MemoryBudget::new(12);
        let decoded = view.decode_with_budget(2, &root, || false).unwrap();
        assert_eq!(decoded.byte_len(), 12);
        assert_eq!(root.used(), 12);
        assert_eq!(decoded.x_bytes(), [1, 0, 1, 0]);
        assert_eq!(decoded.channel_bytes(0).unwrap(), [0, 0, 128, 63, 0, 0, 0, 64]);
        assert!(decoded.channel_bytes(1).is_none());
        assert!(decoded.channel_bytes(14).is_none());
        let alias = decoded.clone();
        drop(decoded);
        assert_eq!(root.used(), 12);
        assert!(matches!(
            view.decode_with_budget(2, &root, || false),
            Err(RpfLayerDecodeError::Memory(_))
        ));
        drop(alias);
        assert_eq!(root.used(), 0);
        let tiny = rrrah_core::MemoryBudget::new(11);
        assert!(matches!(
            view.decode_with_budget(2, &tiny, || false),
            Err(RpfLayerDecodeError::Memory(_))
        ));
        assert_eq!(tiny.used(), 0);
        assert_eq!(tiny.peak(), 0);
        let mut calls = 0;
        assert!(
            view.decode_with_budget(2, &root, || {
                calls += 1;
                calls == 10
            })
            .is_err()
        );
        assert_eq!(root.used(), 0);
        drop(view.decode_with_budget(2, &root, || false).unwrap());
        assert_eq!(root.used(), 0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfNameError {
    Truncated,
    CountLimit,
    ByteLimit,
    MissingTerminator,
    Cancelled,
}

/// Node names retain their producer code-page bytes. Interpretation requires the
/// producing pipeline's code page; invalid UTF-8 is not replaced or discarded.
#[derive(Debug)]
pub struct RpfNodeNames<'a> {
    pub count: u32,
    pub raw: &'a [u8],
    pub consumed: usize,
    names_end: usize,
}
impl RpfNodeNames<'_> {
    pub fn names(&self) -> impl Iterator<Item = &[u8]> {
        self.raw[..self.names_end]
            .split(|b| *b == 0)
            .take(self.count as usize)
    }
    /// Preserve bytes beyond the declared names (some producers pad this block).
    pub fn trailing_bytes(&self) -> &[u8] {
        &self.raw[self.names_end..]
    }
}

pub fn inspect_rpf_node_names(
    input: &[u8],
    max_names: u32,
    max_bytes: u32,
    mut cancelled: impl FnMut() -> bool,
) -> Result<RpfNodeNames<'_>, RpfNameError> {
    use RpfNameError as E;
    if cancelled() {
        return Err(E::Cancelled);
    }
    let count = u32::from_le_bytes(input.get(..4).ok_or(E::Truncated)?.try_into().unwrap());
    let length = u32::from_le_bytes(input.get(4..8).ok_or(E::Truncated)?.try_into().unwrap());
    if count > max_names {
        return Err(E::CountLimit);
    }
    if length > max_bytes {
        return Err(E::ByteLimit);
    }
    let end = 8usize.checked_add(length as usize).ok_or(E::Truncated)?;
    let raw = input.get(8..end).ok_or(E::Truncated)?;
    // A name requires at least its terminator; refuse impossible counts immediately.
    if count > length {
        return Err(E::MissingTerminator);
    }
    let mut found = 0;
    let mut names_end = 0;
    if count > 0 {
        for (i, byte) in raw.iter().enumerate() {
            if i % 128 == 0 && cancelled() {
                return Err(E::Cancelled);
            }
            if *byte == 0 {
                found += 1;
                if found == count {
                    names_end = i + 1;
                    break;
                }
            }
        }
        if found != count {
            return Err(E::MissingTerminator);
        }
    }
    Ok(RpfNodeNames {
        count,
        raw,
        consumed: end,
        names_end,
    })
}

#[cfg(test)]
mod name_tests {
    use super::*;
    fn record(count: u32, raw: &[u8]) -> Vec<u8> {
        let mut b = count.to_le_bytes().to_vec();
        b.extend((raw.len() as u32).to_le_bytes());
        b.extend(raw);
        b
    }
    #[test]
    fn producer_bytes_empty_names_padding_and_next_block_are_preserved() {
        let mut bytes = record(3, &[0xff, 0x80, 0, 0, b'N', 0, 9, 8]);
        let size = bytes.len();
        bytes.extend([7, 6]);
        let names = inspect_rpf_node_names(&bytes, 3, 8, || false).unwrap();
        assert_eq!(
            names.names().collect::<Vec<_>>(),
            [&[0xff, 0x80][..], &[][..], &[b'N'][..]]
        );
        assert_eq!(names.trailing_bytes(), [9, 8]);
        assert_eq!(names.consumed, size);
        let zero = record(0, &[9]);
        let names = inspect_rpf_node_names(&zero, 0, 1, || false).unwrap();
        assert_eq!(names.names().count(), 0);
        assert_eq!(names.trailing_bytes(), [9]);
    }
    #[test]
    fn truncation_caps_terminators_and_cancellation() {
        let bytes = record(2, b"one\0two\0");
        for n in 0..bytes.len() {
            assert!(inspect_rpf_node_names(&bytes[..n], 2, 8, || false).is_err());
        }
        assert_eq!(
            inspect_rpf_node_names(&bytes, 1, 8, || false).unwrap_err(),
            RpfNameError::CountLimit
        );
        assert_eq!(
            inspect_rpf_node_names(&bytes, 2, 7, || false).unwrap_err(),
            RpfNameError::ByteLimit
        );
        let missing = record(2, b"one\0two");
        assert_eq!(
            inspect_rpf_node_names(&missing, 2, 8, || false).unwrap_err(),
            RpfNameError::MissingTerminator
        );
        assert_eq!(
            inspect_rpf_node_names(&bytes, 2, 8, || true).unwrap_err(),
            RpfNameError::Cancelled
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfRenderInfoError {
    Truncated,
    VersionOrProjection,
}

/// Exact producer metadata bits; no matrix inversion or color interpretation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpfRenderInfo {
    pub version: Option<u16>,
    pub projection: u32,
    pub projection_scale_origin_bits: [u32; 4],
    pub field_render: u32,
    pub field_odd: u32,
    pub render_times: [i32; 2],
    /// worldToCam[0..2], camToWorld[0..2], each 12 float bits + flags.
    pub matrix_words: [[u32; 13]; 4],
    /// left, top, right, bottom; absent in the original record layout.
    pub region: Option<[i32; 4]>,
    pub consumed: usize,
}

pub fn inspect_rpf_render_info(input: &[u8]) -> Result<RpfRenderInfo, RpfRenderInfoError> {
    use RpfRenderInfoError as E;
    let first = u16::from_le_bytes(input.get(..2).ok_or(E::Truncated)?.try_into().unwrap());
    let modern = first == 1000;
    let prefix = if modern { 2 } else { 0 };
    let size = if modern { 260 } else { 244 };
    let body = input.get(prefix..prefix + size).ok_or(E::Truncated)?;
    let word = |at| u32::from_le_bytes(body[at..at + 4].try_into().unwrap());
    let projection = word(0);
    if projection > 1 {
        return Err(E::VersionOrProjection);
    }
    Ok(RpfRenderInfo {
        version: modern.then_some(1000),
        projection,
        projection_scale_origin_bits: std::array::from_fn(|i| word(4 + i * 4)),
        field_render: word(20),
        field_odd: word(24),
        render_times: [word(28) as i32, word(32) as i32],
        matrix_words: std::array::from_fn(|m| std::array::from_fn(|i| word(36 + m * 52 + i * 4))),
        region: modern.then(|| std::array::from_fn(|i| word(244 + i * 4) as i32)),
        consumed: prefix + size,
    })
}

#[cfg(test)]
mod render_info_tests {
    use super::*;
    #[test]
    fn exact_modern_and_legacy_fields_and_next_block_boundary() {
        for modern in [false, true] {
            let mut b = Vec::new();
            if modern {
                b.extend(1000u16.to_le_bytes());
            }
            let words = if modern { 65 } else { 61 };
            for i in 0..words {
                b.extend((if i == 0 { 1u32 } else { 0x80000000 | i as u32 }).to_le_bytes());
            }
            let size = b.len();
            b.extend([9, 8]);
            let info = inspect_rpf_render_info(&b).unwrap();
            assert_eq!(info.consumed, size);
            assert_eq!(info.projection, 1);
            assert_eq!(info.version, modern.then_some(1000));
            assert_eq!(
                info.projection_scale_origin_bits,
                [0x80000001, 0x80000002, 0x80000003, 0x80000004]
            );
            assert_eq!(info.render_times, [0x80000007u32 as i32, 0x80000008u32 as i32]);
            for m in 0..4 {
                for i in 0..13 {
                    assert_eq!(info.matrix_words[m][i], 0x80000000 | (9 + m * 13 + i) as u32);
                }
            }
            assert_eq!(
                info.region,
                modern.then_some([
                    0x8000003du32 as i32,
                    0x8000003eu32 as i32,
                    0x8000003fu32 as i32,
                    0x80000040u32 as i32
                ])
            );
            for end in 0..size {
                assert!(inspect_rpf_render_info(&b[..end]).is_err());
            }
        }
        let mut b = vec![0; 262];
        b[..2].copy_from_slice(&1000u16.to_le_bytes());
        b[2..6].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(
            inspect_rpf_render_info(&b).unwrap_err(),
            RpfRenderInfoError::VersionOrProjection
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfFileError {
    Header(RpfInspectError),
    RenderInfo(RpfRenderInfoError),
    Names(RpfNameError),
    RowOrder,
    MetadataOverlap,
    Cancelled,
}

/// Joined borrowed file view for Autodesk's top-to-bottom writer layout.
/// Rows and metadata retain their encoded bytes until root-budget admission.
#[derive(Debug)]
pub struct RpfFileView<'a> {
    pub inspection: RpfInspection<'a>,
    pub render_info: Option<RpfRenderInfo>,
    pub node_names: Option<RpfNodeNames<'a>>,
    pub metadata_padding: &'a [u8],
    bytes: &'a [u8],
    payload_end: usize,
}
impl<'a> RpfFileView<'a> {
    /// Row index is top-to-bottom; no scan or temporary sorted offset array.
    pub fn row_bytes(&self, y: u32) -> Option<&'a [u8]> {
        let height = (self.inspection.offsets.len() / 4) as u32;
        if y >= height {
            return None;
        }
        let i = (height - 1 - y) as usize;
        let offsets = self.inspection.offsets;
        let offset = |i: usize| u32::from_be_bytes(offsets[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
        let start = offset(i);
        let end = if i == 0 { self.payload_end } else { offset(i - 1) };
        self.bytes.get(start..end)
    }
}

pub fn inspect_rpf_file(
    bytes: &[u8],
    max_names: u32,
    max_name_bytes: u32,
    mut cancelled: impl FnMut() -> bool,
) -> Result<RpfFileView<'_>, RpfFileError> {
    use RpfFileError as E;
    if cancelled() {
        return Err(E::Cancelled);
    }
    let inspection = inspect_rpf(bytes).map_err(E::Header)?;
    let mut previous = usize::MAX;
    let mut first_row = 0usize;
    for offset in inspection.row_offsets() {
        if cancelled() {
            return Err(E::Cancelled);
        }
        let offset = offset as usize;
        if offset >= previous {
            return Err(E::RowOrder);
        }
        previous = offset;
        first_row = offset;
    }
    let table_end = 740 + inspection.offsets.len();
    let metadata = bytes.get(table_end..first_row).ok_or(E::MetadataOverlap)?;
    let mut at = 0;
    let render_info = if inspection.header.render_info {
        let info = inspect_rpf_render_info(metadata).map_err(E::RenderInfo)?;
        at += info.consumed;
        Some(info)
    } else {
        None
    };
    let node_names = if inspection.header.node_names {
        let names = inspect_rpf_node_names(&metadata[at..], max_names, max_name_bytes, &mut cancelled)
            .map_err(E::Names)?;
        at += names.consumed;
        Some(names)
    } else {
        None
    };
    if cancelled() {
        return Err(E::Cancelled);
    }
    let payload_end = if inspection.header.next_image_offset == 0 {
        bytes.len()
    } else {
        inspection.header.next_image_offset as usize
    };
    Ok(RpfFileView {
        inspection,
        render_info,
        node_names,
        metadata_padding: &metadata[at..],
        bytes,
        payload_end,
    })
}

#[cfg(test)]
mod file_tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 748];
        for (at, value) in [
            (2, 1u16),
            (6, 1),
            (10, 1),
            (14, 1),
            (20, 3),
            (26, 0xfffd),
            (658, 32),
        ] {
            b[at..at + 2].copy_from_slice(&value.to_be_bytes());
        }
        let program = b"3ds max : ( Z P D )";
        b[400..400 + program.len()].copy_from_slice(program);
        b.extend(1000u16.to_le_bytes());
        b.extend([0; 260]);
        b.extend(1u32.to_le_bytes());
        b.extend(4u32.to_le_bytes());
        b.extend(b"one\0");
        b.extend([77, 88]); // preserved metadata padding
        let top = b.len() as u32;
        b.extend([1, 2, 3]);
        let bottom = b.len() as u32;
        b.extend([4, 5]);
        b[740..744].copy_from_slice(&bottom.to_be_bytes());
        b[744..748].copy_from_slice(&top.to_be_bytes());
        b
    }
    #[test]
    fn joins_metadata_and_constant_time_oriented_row_ranges() {
        let b = fixture();
        let file = inspect_rpf_file(&b, 1, 4, || false).unwrap();
        assert_eq!(file.render_info.as_ref().unwrap().version, Some(1000));
        assert_eq!(
            file.node_names.as_ref().unwrap().names().collect::<Vec<_>>(),
            [b"one"]
        );
        assert_eq!(file.metadata_padding, [77, 88]);
        assert_eq!(file.row_bytes(0).unwrap(), [1, 2, 3]);
        assert_eq!(file.row_bytes(1).unwrap(), [4, 5]);
        assert!(file.row_bytes(2).is_none());
    }
    #[test]
    fn duplicate_reversed_or_metadata_overlapping_rows_refuse() {
        let original = fixture();
        let mut b = original.clone();
        let top: [u8; 4] = b[744..748].try_into().unwrap();
        b[740..744].copy_from_slice(&top);
        assert_eq!(
            inspect_rpf_file(&b, 1, 4, || false).unwrap_err(),
            RpfFileError::RowOrder
        );
        b = original.clone();
        b[740..748].rotate_left(4);
        assert_eq!(
            inspect_rpf_file(&b, 1, 4, || false).unwrap_err(),
            RpfFileError::RowOrder
        );
        b = original.clone();
        b[744..748].copy_from_slice(&748u32.to_be_bytes());
        assert!(matches!(
            inspect_rpf_file(&b, 1, 4, || false),
            Err(RpfFileError::RenderInfo(_))
        ));
        assert_eq!(
            inspect_rpf_file(&original, 0, 4, || false).unwrap_err(),
            RpfFileError::Names(RpfNameError::CountLimit)
        );
        assert_eq!(
            inspect_rpf_file(&original, 1, 4, || true).unwrap_err(),
            RpfFileError::Cancelled
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfRowError {
    Plane(RpfPlaneError),
    Layers(RpfLayerError),
    Coordinates(RpfLayerCoordinateError),
    Depth,
    TrailingBytes,
}

#[derive(Debug)]
pub struct RpfRowView<'a> {
    /// Color, matte and auxiliary records in header order, with u16 length prefixes.
    pub main_records: &'a [u8],
    pub gbuffer: [Option<&'a [u8]>; 14],
    pub layers: Option<RpfLayerRecords<'a>>,
}
impl RpfRowView<'_> {
    pub fn main_channels(&self) -> impl Iterator<Item = &[u8]> {
        let mut remaining = self.main_records;
        std::iter::from_fn(move || {
            let prefix = remaining.get(..2)?;
            let length = u16::from_be_bytes(prefix.try_into().unwrap()) as usize;
            let record = remaining.get(..2 + length)?;
            remaining = &remaining[2 + length..];
            Some(record)
        })
    }
}

fn validate_rpf_plane(
    encoded: &[u8],
    samples: u32,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<usize, RpfPlaneError> {
    let mut at = 0usize;
    let mut x = 0u32;
    while x < samples {
        if cancelled() {
            return Err(RpfPlaneError::Cancelled);
        }
        let control = *encoded.get(at).ok_or(RpfPlaneError::Truncated)? as i8;
        at += 1;
        let count = if control >= 0 {
            control as u32 + 1
        } else {
            control.unsigned_abs() as u32
        };
        if count > samples - x {
            return Err(RpfPlaneError::RunOverflow);
        }
        let bytes = if control >= 0 { 1 } else { count as usize };
        encoded.get(at..at + bytes).ok_or(RpfPlaneError::Truncated)?;
        at += bytes;
        x += count;
    }
    Ok(at)
}

/// Validate a complete modern row without expanding samples. Explicit depth/type
/// interpretation remains the caller's responsibility; auxiliary records are retained.
pub fn inspect_rpf_row<'a>(
    input: &'a [u8],
    header: &RpfHeader,
    max_layer_records: u32,
    mut cancelled: impl FnMut() -> bool,
) -> Result<RpfRowView<'a>, RpfRowError> {
    use RpfRowError as E;
    let width = header.active_window.width();
    let mut at = 0usize;
    for (count, depth) in [
        (header.color_channels, header.color_bits),
        (header.matte_channels, header.matte_bits),
        (header.auxiliary_channels, header.auxiliary_bits),
    ] {
        if count > 0 && !matches!(depth, 8 | 16 | 32) {
            return Err(E::Depth);
        }
        for _ in 0..count {
            if cancelled() {
                return Err(E::Plane(RpfPlaneError::Cancelled));
            }
            let length = u16::from_be_bytes(
                input
                    .get(at..at + 2)
                    .ok_or(E::Plane(RpfPlaneError::Truncated))?
                    .try_into()
                    .unwrap(),
            ) as usize;
            at += 2;
            let payload = input
                .get(at..at + length)
                .ok_or(E::Plane(RpfPlaneError::Truncated))?;
            if depth == 32 {
                if length as u64 != u64::from(width) * 4 {
                    return Err(E::Plane(RpfPlaneError::Layout));
                }
            } else {
                let mut used = 0;
                for _ in 0..depth / 8 {
                    used += validate_rpf_plane(&payload[used..], width, &mut cancelled).map_err(E::Plane)?;
                }
                if used != length {
                    return Err(E::Plane(RpfPlaneError::TrailingBytes));
                }
            }
            at += length;
        }
    }
    let main_records = &input[..at];
    let mut gbuffer = [None; 14];
    for (i, size) in RPF_GBUFFER_SAMPLE_BYTES.iter().enumerate() {
        if header.gbuffer_channels & (1 << i) == 0 {
            continue;
        }
        let start = at;
        for _ in 0..*size {
            if cancelled() {
                return Err(E::Plane(RpfPlaneError::Cancelled));
            }
            let length = u16::from_be_bytes(
                input
                    .get(at..at + 2)
                    .ok_or(E::Plane(RpfPlaneError::Truncated))?
                    .try_into()
                    .unwrap(),
            ) as usize;
            at += 2;
            let encoded = input
                .get(at..at + length)
                .ok_or(E::Plane(RpfPlaneError::Truncated))?;
            if validate_rpf_plane(encoded, width, &mut cancelled).map_err(E::Plane)? != length {
                return Err(E::Plane(RpfPlaneError::TrailingBytes));
            }
            at += length;
        }
        gbuffer[i] = Some(&input[start..at]);
    }
    let layers = if header.layer_data {
        let view = inspect_rpf_layer_records(
            &input[at..],
            header.gbuffer_channels,
            max_layer_records,
            &mut cancelled,
        )
        .map_err(E::Layers)?;
        view.validate_coordinates(width, &mut cancelled)
            .map_err(E::Coordinates)?;
        at += view.consumed;
        Some(view)
    } else {
        None
    };
    if at != input.len() {
        return Err(E::TrailingBytes);
    }
    if cancelled() {
        return Err(E::Plane(RpfPlaneError::Cancelled));
    }
    Ok(RpfRowView {
        main_records,
        gbuffer,
        layers,
    })
}

#[cfg(test)]
mod row_tests {
    use super::*;
    #[test]
    fn complete_color_matte_aux_gbuffer_layer_row_and_all_truncations() {
        let mut header_bytes = vec![0; 746];
        for (at, value) in [
            (20, 3u16),
            (22, 1),
            (24, 1),
            (26, 0xfffd),
            (658, 32),
            (662, 16),
            (666, 8),
        ] {
            header_bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
        }
        let program = b"3ds max : ( Z L )";
        header_bytes[400..400 + program.len()].copy_from_slice(program);
        header_bytes[740..744].copy_from_slice(&744u32.to_be_bytes());
        let header = inspect_rpf(&header_bytes).unwrap().header;
        let mut row = Vec::new();
        for value in [2.0f32, -1.0, 64.0] {
            row.extend(4u16.to_be_bytes());
            row.extend(value.to_bits().to_be_bytes());
        }
        row.extend([0, 4, 0, 128, 0, 1]); // 16-bit matte, two planes
        row.extend([0, 2, 0, 7]); // 8-bit auxiliary
        for value in [63u8, 128, 0, 0] {
            row.extend([0, 2, 0, value]);
        }
        row.extend(1i32.to_le_bytes());
        for value in [0u8, 0, 64, 0, 0, 0] {
            row.extend(2u32.to_be_bytes());
            row.extend([0, value]);
        }
        let view = inspect_rpf_row(&row, &header, 1, || false).unwrap();
        assert_eq!(view.main_channels().count(), 5);
        assert_eq!(view.layers.as_ref().unwrap().records, 1);
        let mut depth = [0; 4];
        decode_rpf_byte_planes(view.gbuffer[0].unwrap(), &mut depth, 4, false, || false).unwrap();
        assert_eq!(depth, 1.0f32.to_le_bytes());
        let mut value = [0.0];
        decode_rpf_color_channel(view.main_channels().next().unwrap(), &mut value, 32, || false).unwrap();
        assert_eq!(value, [2.0]);
        for end in 0..row.len() {
            assert!(
                inspect_rpf_row(&row[..end], &header, 1, || false).is_err(),
                "{end}"
            );
        }
        assert!(matches!(
            inspect_rpf_row(&row, &header, 0, || false),
            Err(RpfRowError::Layers(RpfLayerError::CountLimit))
        ));
        row.push(0);
        assert_eq!(
            inspect_rpf_row(&row, &header, 1, || false).unwrap_err(),
            RpfRowError::TrailingBytes
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpfImageAllocationPlan {
    pub width: u32,
    pub height: u32,
    pub main_channel_count: u32,
    pub main_float_bytes: u64,
    pub gbuffer_bytes: u64,
    pub layer_bytes: u64,
    pub layer_records: u64,
    /// Packed per-row layer offset (u64) and count (u32).
    pub layer_index_bytes: u64,
    /// Original header, offsets and producer metadata retained for interpretation.
    pub metadata_bytes: u64,
    pub total_bytes: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfPlanError {
    Row { y: u32, error: RpfRowError },
    MissingRow,
    Overflow,
    LayerRecordLimit,
    ByteLimit,
    Cancelled,
}
impl RpfFileView<'_> {
    /// Full-file preflight. It scans every channel and layer before any output
    /// allocation, and applies both per-row and whole-image layer count caps.
    /// Counts describe a packed output representation, not allocator overhead.
    pub fn allocation_plan(
        &self,
        max_row_layers: u32,
        max_total_layers: u64,
        max_output_bytes: u64,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<RpfImageAllocationPlan, RpfPlanError> {
        use RpfPlanError as E;
        let header = &self.inspection.header;
        let width = header.active_window.width();
        let height = (self.inspection.offsets.len() / 4) as u32;
        let main_channel_count = u32::from(header.color_channels)
            + u32::from(header.matte_channels)
            + u32::from(header.auxiliary_channels);
        let pixels = u64::from(width) * u64::from(height);
        let main_float_bytes = pixels
            .checked_mul(u64::from(main_channel_count))
            .and_then(|v| v.checked_mul(4))
            .ok_or(E::Overflow)?;
        let gbuffer_stride: u64 = RPF_GBUFFER_SAMPLE_BYTES
            .iter()
            .enumerate()
            .filter(|(i, _)| header.gbuffer_channels & (1 << i) != 0)
            .map(|(_, s)| *s as u64)
            .sum();
        let gbuffer_bytes = pixels.checked_mul(gbuffer_stride).ok_or(E::Overflow)?;
        let layer_index_bytes = if header.layer_data {
            u64::from(height) * 12
        } else {
            0
        };
        let metadata_bytes = self.inspection.row_offsets().last().ok_or(E::MissingRow)? as u64;
        let base = main_float_bytes
            .checked_add(gbuffer_bytes)
            .and_then(|v| v.checked_add(layer_index_bytes))
            .and_then(|v| v.checked_add(metadata_bytes))
            .ok_or(E::Overflow)?;
        if base > max_output_bytes {
            return Err(E::ByteLimit);
        }
        let mut layer_bytes = 0u64;
        let mut layer_records = 0u64;
        for y in 0..height {
            if cancelled() {
                return Err(E::Cancelled);
            }
            let row = self.row_bytes(y).ok_or(E::MissingRow)?;
            let view = inspect_rpf_row(row, header, max_row_layers, &mut cancelled)
                .map_err(|error| E::Row { y, error })?;
            if let Some(layers) = view.layers {
                layer_records = layer_records
                    .checked_add(u64::from(layers.records))
                    .ok_or(E::Overflow)?;
                if layer_records > max_total_layers {
                    return Err(E::LayerRecordLimit);
                }
                layer_bytes = layer_bytes
                    .checked_add(layers.expanded_bytes())
                    .ok_or(E::Overflow)?;
                if base.checked_add(layer_bytes).ok_or(E::Overflow)? > max_output_bytes {
                    return Err(E::ByteLimit);
                }
            }
        }
        if cancelled() {
            return Err(E::Cancelled);
        }
        Ok(RpfImageAllocationPlan {
            width,
            height,
            main_channel_count,
            main_float_bytes,
            gbuffer_bytes,
            layer_bytes,
            layer_records,
            layer_index_bytes,
            metadata_bytes,
            total_bytes: base + layer_bytes,
        })
    }
}

#[cfg(test)]
mod allocation_plan_tests {
    use super::*;
    pub(super) fn fixture() -> Vec<u8> {
        let mut b = vec![0; 748];
        for (at, value) in [(6, 1u16), (14, 1), (20, 3), (26, 0xfffd), (658, 32)] {
            b[at..at + 2].copy_from_slice(&value.to_be_bytes());
        }
        let program = b"3ds max : ( Z L )";
        b[400..400 + program.len()].copy_from_slice(program);
        let mut starts = Vec::new();
        for value in [2.0f32, 64.0] {
            starts.push(b.len() as u32);
            for _ in 0..3 {
                b.extend(4u16.to_be_bytes());
                b.extend(value.to_bits().to_be_bytes());
            }
            for v in [63u8, 128, 0, 0] {
                b.extend([0, 2, 0, v]);
            }
            b.extend(1i32.to_le_bytes());
            for v in [0u8, 0, 63, 128, 0, 0] {
                b.extend(2u32.to_be_bytes());
                b.extend([0, v]);
            }
        }
        b[740..744].copy_from_slice(&starts[1].to_be_bytes());
        b[744..748].copy_from_slice(&starts[0].to_be_bytes());
        b
    }
    #[test]
    fn all_rows_count_toward_global_byte_and_layer_caps() {
        let b = fixture();
        let view = inspect_rpf_file(&b, 0, 0, || false).unwrap();
        let p = view.allocation_plan(1, 2, 816, || false).unwrap();
        assert_eq!((p.width, p.height, p.main_channel_count), (1, 2, 3));
        assert_eq!(
            (
                p.main_float_bytes,
                p.gbuffer_bytes,
                p.layer_bytes,
                p.layer_index_bytes,
                p.metadata_bytes
            ),
            (24, 8, 12, 24, 748)
        );
        assert_eq!(p.total_bytes, 816);
        assert_eq!(p.layer_records, 2);
        assert_eq!(
            view.allocation_plan(1, 2, 815, || false),
            Err(RpfPlanError::ByteLimit)
        );
        assert_eq!(
            view.allocation_plan(1, 1, 816, || false),
            Err(RpfPlanError::LayerRecordLimit)
        );
        assert!(matches!(
            view.allocation_plan(0, 2, 816, || false),
            Err(RpfPlanError::Row { y: 0, .. })
        ));
        assert_eq!(
            view.allocation_plan(1, 2, 816, || true),
            Err(RpfPlanError::Cancelled)
        );
        let mut bad = b.clone();
        *bad.last_mut().unwrap() = 1; // legal changed depth value
        let last = bad.len();
        bad[last - 2] = 1; // run expands to two samples, declared one
        let view = inspect_rpf_file(&bad, 0, 0, || false).unwrap();
        assert!(matches!(
            view.allocation_plan(1, 2, 816, || false),
            Err(RpfPlanError::Row { y: 1, .. })
        ));
    }
}

#[derive(Debug)]
pub enum RpfImageDecodeError {
    Plan(RpfPlanError),
    Row(RpfRowError),
    Plane(RpfPlaneError),
    Memory(rrrah_core::BufferError),
}
/// Entire decoded image shares one managed allocation. Main samples are packed
/// little-endian f32, G-buffer and layer channels preserve producer memory bytes.
/// This is a data decoder; display color/alpha interpretation is still explicit.
#[derive(Debug, Clone)]
pub struct RpfDecodedImage {
    plan: RpfImageAllocationPlan,
    header: RpfHeader,
    bytes: rrrah_core::SharedBuffer<u8>,
    gbuffer_ranges: [Option<(usize, usize)>; 14],
    layer_index_start: usize,
}
impl RpfDecodedImage {
    pub fn plan(&self) -> RpfImageAllocationPlan {
        self.plan
    }
    pub fn header(&self) -> RpfHeader {
        self.header
    }
    pub fn packed_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn metadata_bytes(&self) -> &[u8] {
        &self.bytes[..self.plan.metadata_bytes as usize]
    }
    pub fn main_sample(&self, channel: u32, pixel: u64) -> Option<f32> {
        let pixels = u64::from(self.plan.width) * u64::from(self.plan.height);
        if channel >= self.plan.main_channel_count || pixel >= pixels {
            return None;
        }
        let at = self.plan.metadata_bytes + (u64::from(channel) * pixels + pixel) * 4;
        let sample = self.bytes.get(at as usize..at as usize + 4)?;
        Some(f32::from_bits(u32::from_le_bytes(sample.try_into().unwrap())))
    }
    pub fn gbuffer_channel(&self, channel: usize) -> Option<&[u8]> {
        let (start, end) = self.gbuffer_ranges.get(channel).copied().flatten()?;
        Some(&self.bytes[start..end])
    }
    /// Packed X arrays followed by present channel arrays, in G-buffer order.
    pub fn layer_row(&self, y: u32) -> Option<(u32, &[u8])> {
        if !self.header.layer_data || y >= self.plan.height {
            return None;
        }
        let at = self.layer_index_start + y as usize * 12;
        let start = u64::from_le_bytes(self.bytes[at..at + 8].try_into().unwrap()) as usize;
        let count = u32::from_le_bytes(self.bytes[at + 8..at + 12].try_into().unwrap());
        let stride: usize = 2 + RPF_GBUFFER_SAMPLE_BYTES
            .iter()
            .enumerate()
            .filter(|(i, _)| self.header.gbuffer_channels & (1 << i) != 0)
            .map(|(_, size)| *size)
            .sum::<usize>();
        Some((count, &self.bytes[start..start + count as usize * stride]))
    }
}

// The complete preflight has already validated the record. Write straight into
// final packed output, avoiding a width-dependent temporary float vector.
fn write_rpf_main_record(
    record: &[u8],
    output: &mut [u8],
    depth: u16,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), RpfPlaneError> {
    let payload = &record[2..];
    if depth == 32 {
        for (src, dst) in payload.chunks_exact(4).zip(output.chunks_exact_mut(4)) {
            if cancelled() {
                return Err(RpfPlaneError::Cancelled);
            }
            dst.copy_from_slice(&u32::from_be_bytes(src.try_into().unwrap()).to_le_bytes());
        }
    } else {
        let samples = (output.len() / 4) as u32;
        let high_end = validate_rpf_plane(payload, samples, cancelled)?;
        let mut high = PlaneValues {
            data: &payload[..high_end],
            at: 0,
            remaining: 0,
            repeat: None,
        };
        let mut low = PlaneValues {
            data: &payload[high_end..],
            at: 0,
            remaining: 0,
            repeat: None,
        };
        for dst in output.chunks_exact_mut(4) {
            if cancelled() {
                return Err(RpfPlaneError::Cancelled);
            }
            let mut value = high.next()? as f32 / 256.0;
            if depth == 16 {
                value += low.next()? as f32 / 65536.0;
            }
            dst.copy_from_slice(&value.to_bits().to_le_bytes());
        }
    }
    Ok(())
}

impl RpfFileView<'_> {
    pub fn decode_with_budget(
        &self,
        budget: &rrrah_core::MemoryBudget,
        max_row_layers: u32,
        max_total_layers: u64,
        max_output_bytes: u64,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<RpfDecodedImage, RpfImageDecodeError> {
        use RpfImageDecodeError as E;
        let plan = self
            .allocation_plan(max_row_layers, max_total_layers, max_output_bytes, &mut cancelled)
            .map_err(E::Plan)?;
        let total =
            usize::try_from(plan.total_bytes).map_err(|_| E::Memory(rrrah_core::BufferError::Overflow))?;
        let mut bytes = budget.try_buffer(total, 0u8).map_err(E::Memory)?;
        let metadata_end = plan.metadata_bytes as usize;
        bytes[..metadata_end].copy_from_slice(&self.bytes[..metadata_end]);
        let pixels = plan.width as usize * plan.height as usize;
        let main_end = metadata_end + plan.main_float_bytes as usize;
        let mut at = main_end;
        let mut gbuffer_ranges = [None; 14];
        for (i, size) in RPF_GBUFFER_SAMPLE_BYTES.iter().enumerate() {
            if self.inspection.header.gbuffer_channels & (1 << i) != 0 {
                let end = at + pixels * size;
                gbuffer_ranges[i] = Some((at, end));
                at = end;
            }
        }
        let layer_index_start = at;
        at += plan.layer_index_bytes as usize;
        let header = self.inspection.header;
        for y in 0..plan.height {
            let row = self.row_bytes(y).ok_or(E::Plan(RpfPlanError::MissingRow))?;
            let view = inspect_rpf_row(row, &header, max_row_layers, &mut cancelled).map_err(E::Row)?;
            for (channel, record) in view.main_channels().enumerate() {
                let depth = if channel < header.color_channels as usize {
                    header.color_bits
                } else if channel
                    < (u32::from(header.color_channels) + u32::from(header.matte_channels)) as usize
                {
                    header.matte_bits
                } else {
                    header.auxiliary_bits
                };
                let start = metadata_end + (channel * pixels + y as usize * plan.width as usize) * 4;
                write_rpf_main_record(
                    record,
                    &mut bytes[start..start + plan.width as usize * 4],
                    depth,
                    &mut cancelled,
                )
                .map_err(E::Plane)?;
            }
            for (i, encoded) in view.gbuffer.iter().enumerate() {
                if let Some(encoded) = encoded {
                    let (base, _) = gbuffer_ranges[i].unwrap();
                    let size = plan.width as usize * RPF_GBUFFER_SAMPLE_BYTES[i];
                    let start = base + y as usize * size;
                    decode_rpf_byte_planes(
                        encoded,
                        &mut bytes[start..start + size],
                        RPF_GBUFFER_SAMPLE_BYTES[i],
                        false,
                        &mut cancelled,
                    )
                    .map_err(E::Plane)?;
                }
            }
            if let Some(layers) = view.layers {
                let index = layer_index_start + y as usize * 12;
                bytes[index..index + 8].copy_from_slice(&(at as u64).to_le_bytes());
                bytes[index + 8..index + 12].copy_from_slice(&layers.records.to_le_bytes());
                if layers.records > 0 {
                    let size = layers.records as usize * 2;
                    decode_rpf_byte_planes(
                        layers.x_planes,
                        &mut bytes[at..at + size],
                        2,
                        true,
                        &mut cancelled,
                    )
                    .map_err(E::Plane)?;
                    at += size;
                    for (i, encoded) in layers.channels.iter().enumerate() {
                        if let Some(encoded) = encoded {
                            let size = layers.records as usize * RPF_GBUFFER_SAMPLE_BYTES[i];
                            decode_rpf_byte_planes(
                                encoded,
                                &mut bytes[at..at + size],
                                RPF_GBUFFER_SAMPLE_BYTES[i],
                                true,
                                &mut cancelled,
                            )
                            .map_err(E::Plane)?;
                            at += size;
                        }
                    }
                }
            }
        }
        if cancelled() {
            return Err(E::Plane(RpfPlaneError::Cancelled));
        }
        debug_assert_eq!(at, total);
        Ok(RpfDecodedImage {
            plan,
            header,
            bytes: bytes.freeze(),
            gbuffer_ranges,
            layer_index_start,
        })
    }
}

#[cfg(test)]
mod image_budget_tests {
    use super::*;
    #[test]
    fn complete_image_hdr_channels_layers_and_last_owner() {
        let b = super::allocation_plan_tests::fixture();
        let file = inspect_rpf_file(&b, 0, 0, || false).unwrap();
        let root = rrrah_core::MemoryBudget::new(816);
        let image = file.decode_with_budget(&root, 1, 2, 816, || false).unwrap();
        assert_eq!(image.packed_bytes().len(), 816);
        assert_eq!(root.used(), 816);
        assert_eq!(image.metadata_bytes(), &b[..748]);
        for channel in 0..3 {
            assert_eq!(image.main_sample(channel, 0), Some(2.0));
            assert_eq!(image.main_sample(channel, 1), Some(64.0));
        }
        assert!(image.main_sample(3, 0).is_none());
        assert!(image.main_sample(0, 2).is_none());
        assert_eq!(image.gbuffer_channel(0).unwrap(), [0, 0, 128, 63, 0, 0, 128, 63]);
        for y in 0..2 {
            assert_eq!(image.layer_row(y).unwrap(), (1, &[0, 0, 0, 0, 128, 63][..]));
        }
        let alias = image.clone();
        drop(image);
        assert_eq!(root.used(), 816);
        drop(alias);
        assert_eq!(root.used(), 0);
        let tiny = rrrah_core::MemoryBudget::new(815);
        assert!(matches!(
            file.decode_with_budget(&tiny, 1, 2, 816, || false),
            Err(RpfImageDecodeError::Memory(_))
        ));
        assert_eq!(tiny.peak(), 0);
        let cancel_root = rrrah_core::MemoryBudget::new(816);
        assert!(
            file.decode_with_budget(&cancel_root, 1, 2, 816, || cancel_root.used() > 0)
                .is_err()
        );
        assert_eq!(cancel_root.peak(), 816);
        assert_eq!(cancel_root.used(), 0);
        drop(
            file.decode_with_budget(&cancel_root, 1, 2, 816, || false)
                .unwrap(),
        );
        assert_eq!(cancel_root.used(), 0);
    }
}

#[cfg(test)]
mod sdk_plane_oracle_tests {
    use super::*;
    fn unhex(s: &str) -> Vec<u8> {
        s.as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let digit = |v: u8| if v <= b'9' { v - b'0' } else { v - b'a' + 10 };
                digit(pair[0]) * 16 + digit(pair[1])
            })
            .collect()
    }
    #[test]
    fn official_sdk_encoded_runs_restore_all_samples_for_image_and_layer_records() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/rpf/sdk-plane-oracle.json")).unwrap();
        let cases = manifest["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 98);
        for case in cases {
            let width = case["width"].as_u64().unwrap() as usize;
            let size = case["sample_bytes"].as_u64().unwrap() as usize;
            let expected = unhex(case["expected_hex"].as_str().unwrap());
            let encoded = unhex(case["encoded_hex"].as_str().unwrap());
            let mut output = vec![0; width * size];
            assert_eq!(
                decode_rpf_byte_planes(&encoded, &mut output, size, false, || false).unwrap(),
                encoded.len()
            );
            assert_eq!(output, expected, "channel {} width {width}", case["channel"]);
            // Layer records carry identical plane bodies with u32 length prefixes.
            let mut layers = Vec::new();
            let mut at = 0;
            for _ in 0..size {
                let n = u16::from_be_bytes(encoded[at..at + 2].try_into().unwrap()) as usize;
                at += 2;
                layers.extend((n as u32).to_be_bytes());
                layers.extend(&encoded[at..at + n]);
                at += n;
            }
            assert_eq!(
                decode_rpf_byte_planes(&layers, &mut output, size, true, || false).unwrap(),
                layers.len()
            );
            assert_eq!(output, expected);
        }
    }
}

/// Fixed-size reconstruction metadata retained alongside a swap handle. It owns
/// no image allocation. The digest binds this layout to the exact packed bytes.
#[derive(Debug, Clone)]
pub struct RpfImageSnapshot {
    plan: RpfImageAllocationPlan,
    header: RpfHeader,
    gbuffer_ranges: [Option<(usize, usize)>; 14],
    layer_index_start: usize,
    digest: [u8; 32],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfSnapshotError {
    Length,
    Corrupt,
    Cancelled,
}
fn rpf_packed_digest(
    bytes: &[u8],
    cancelled: &mut impl FnMut() -> bool,
) -> Result<[u8; 32], RpfSnapshotError> {
    let mut hash = blake3::Hasher::new();
    for block in bytes.chunks(64 * 1024) {
        if cancelled() {
            return Err(RpfSnapshotError::Cancelled);
        }
        hash.update(block);
    }
    if cancelled() {
        return Err(RpfSnapshotError::Cancelled);
    }
    Ok(*hash.finalize().as_bytes())
}
impl RpfDecodedImage {
    pub fn snapshot(
        &self,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<RpfImageSnapshot, RpfSnapshotError> {
        Ok(RpfImageSnapshot {
            plan: self.plan,
            header: self.header,
            gbuffer_ranges: self.gbuffer_ranges,
            layer_index_start: self.layer_index_start,
            digest: rpf_packed_digest(&self.bytes, &mut cancelled)?,
        })
    }
}
impl RpfImageSnapshot {
    pub fn packed_byte_len(&self) -> u64 {
        self.plan.total_bytes
    }
    /// Attach a managed restored buffer only after length and digest verification.
    /// Failure drops the supplied owner; other aliases retain their own reservation.
    pub fn restore(
        &self,
        bytes: rrrah_core::SharedBuffer<u8>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<RpfDecodedImage, RpfSnapshotError> {
        if bytes.len() as u64 != self.plan.total_bytes {
            return Err(RpfSnapshotError::Length);
        }
        if rpf_packed_digest(&bytes, &mut cancelled)? != self.digest {
            return Err(RpfSnapshotError::Corrupt);
        }
        Ok(RpfDecodedImage {
            plan: self.plan,
            header: self.header,
            bytes,
            gbuffer_ranges: self.gbuffer_ranges,
            layer_index_start: self.layer_index_start,
        })
    }
}

#[cfg(test)]
mod actual_swap_tests {
    use super::*;
    #[test]
    fn disk_swap_restores_all_bytes_after_source_drop_and_memory_pressure() {
        let b = super::allocation_plan_tests::fixture();
        let file = inspect_rpf_file(&b, 0, 0, || false).unwrap();
        let root = rrrah_core::MemoryBudget::new(816);
        let image = file.decode_with_budget(&root, 1, 2, 816, || false).unwrap();
        let snapshot = image.snapshot(|| false).unwrap();
        let expected = image.packed_bytes().to_vec();
        let store = rrrah_swap::SwapStore::new(
            &std::env::temp_dir(),
            rrrah_swap::SwapLimits {
                max_bytes: 816,
                max_objects: Some(1),
            },
        )
        .unwrap();
        let handle = store
            .write_chunks(snapshot.packed_byte_len(), [image.packed_bytes()], || false)
            .unwrap();
        drop(image);
        assert_eq!(root.used(), 0);
        let pressure = root.try_buffer(1, 0u8).unwrap();
        for _ in 0..3 {
            assert!(store.restore(&handle, &root, || false).is_err());
            assert_eq!(store.usage().unwrap().objects, 1);
        }
        drop(pressure);
        for _ in 0..3 {
            let buffer = store.restore(&handle, &root, || false).unwrap();
            let image = snapshot.restore(buffer, || false).unwrap();
            assert_eq!(image.packed_bytes(), expected);
            assert_eq!(root.used(), 816);
            assert_eq!(image.main_sample(0, 1), Some(64.0));
            assert_eq!(image.layer_row(1).unwrap(), (1, &[0, 0, 0, 0, 128, 63][..]));
            drop(image);
            assert_eq!(root.used(), 0);
        }
        let buffer = store.restore(&handle, &root, || false).unwrap();
        assert_eq!(
            snapshot.restore(buffer, || true).unwrap_err(),
            RpfSnapshotError::Cancelled
        );
        assert_eq!(root.used(), 0);
        let mut wrong = root.try_buffer(816, 0u8).unwrap();
        wrong[..].copy_from_slice(&expected);
        wrong[800] ^= 1;
        assert_eq!(
            snapshot.restore(wrong.freeze(), || false).unwrap_err(),
            RpfSnapshotError::Corrupt
        );
        assert_eq!(root.used(), 0);
        let short = root.try_buffer(815, 0u8).unwrap();
        assert_eq!(
            snapshot.restore(short.freeze(), || false).unwrap_err(),
            RpfSnapshotError::Length
        );
        assert_eq!(root.used(), 0);
        drop(handle);
        assert_eq!(store.usage().unwrap().objects, 0);
        assert_eq!(store.usage().unwrap().bytes, 0);
    }
}

#[derive(Debug)]
pub enum RpfDisplayError {
    ChannelSelection,
    InvalidSample,
    ZeroAlphaEmission,
    Cancelled,
    Memory(rrrah_core::BufferError),
    Raster(rrrah_core::RasterError),
}
impl RpfDecodedImage {
    /// Select RGB and an optional matte channel explicitly. Source data remains
    /// intact; the raster uses straight alpha and the caller's declared color space.
    pub fn to_raster_with_interpretation(
        &self,
        rgb: [u16; 3],
        matte: Option<u16>,
        alpha_mode: crate::RlaAlphaMode,
        color_space: rrrah_core::RasterColorSpace,
        budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::DecodedRaster, RpfDisplayError> {
        use RpfDisplayError as E;
        if rgb.iter().any(|i| *i >= self.header.color_channels)
            || matte.is_some_and(|i| i >= self.header.matte_channels)
        {
            return Err(E::ChannelSelection);
        }
        if cancelled() {
            return Err(E::Cancelled);
        }
        let count = self.plan.width as usize * self.plan.height as usize;
        let length = count
            .checked_mul(4)
            .ok_or(E::Memory(rrrah_core::BufferError::Overflow))?;
        let mut output = budget.try_buffer(length, 0.0f32).map_err(E::Memory)?;
        for (i, pixel) in output.chunks_exact_mut(4).enumerate() {
            if cancelled() {
                return Err(E::Cancelled);
            }
            let alpha = matte.map_or(1.0, |m| {
                self.main_sample(u32::from(self.header.color_channels) + u32::from(m), i as u64)
                    .unwrap()
            });
            if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
                return Err(E::InvalidSample);
            }
            for c in 0..3 {
                let value = self.main_sample(u32::from(rgb[c]), i as u64).unwrap();
                if !value.is_finite() {
                    return Err(E::InvalidSample);
                }
                pixel[c] = if matte.is_some() && alpha_mode == crate::RlaAlphaMode::Premultiplied {
                    if alpha == 0.0 {
                        if value != 0.0 {
                            return Err(E::ZeroAlphaEmission);
                        }
                        0.0
                    } else {
                        value / alpha
                    }
                } else {
                    value
                };
                if !pixel[c].is_finite() {
                    return Err(E::InvalidSample);
                }
            }
            pixel[3] = alpha;
        }
        if cancelled() {
            return Err(E::Cancelled);
        }
        rrrah_core::DecodedRaster::new(
            self.plan.width,
            self.plan.height,
            rrrah_core::RasterPixels::Rgba32Float(output.freeze().into()),
            color_space,
        )
        .map_err(E::Raster)
    }
}
#[cfg(test)]
mod display_tests {
    use super::*;
    #[test]
    fn explicit_selection_preserves_hdr_and_both_allocations_share_root_cap() {
        let b = super::allocation_plan_tests::fixture();
        let file = inspect_rpf_file(&b, 0, 0, || false).unwrap();
        let root = rrrah_core::MemoryBudget::new(848);
        let image = file.decode_with_budget(&root, 1, 2, 816, || false).unwrap();
        let raster = image
            .to_raster_with_interpretation(
                [2, 1, 0],
                None,
                crate::RlaAlphaMode::Straight,
                rrrah_core::RasterColorSpace::LinearSrgb,
                &root,
                || false,
            )
            .unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(p) = raster.pixels() else {
            panic!("float raster expected")
        };
        assert_eq!(&p[..], [2.0, 2.0, 2.0, 1.0, 64.0, 64.0, 64.0, 1.0]);
        assert_eq!(root.used(), 848);
        assert!(matches!(
            image.to_raster_with_interpretation(
                [0, 1, 2],
                None,
                crate::RlaAlphaMode::Straight,
                rrrah_core::RasterColorSpace::LinearSrgb,
                &root,
                || false
            ),
            Err(RpfDisplayError::Memory(_))
        ));
        drop(raster);
        assert_eq!(root.used(), 816);
        assert!(matches!(
            image.to_raster_with_interpretation(
                [0, 1, 3],
                None,
                crate::RlaAlphaMode::Straight,
                rrrah_core::RasterColorSpace::LinearSrgb,
                &root,
                || false
            ),
            Err(RpfDisplayError::ChannelSelection)
        ));
        assert!(matches!(
            image.to_raster_with_interpretation(
                [0, 1, 2],
                None,
                crate::RlaAlphaMode::Straight,
                rrrah_core::RasterColorSpace::LinearSrgb,
                &root,
                || true
            ),
            Err(RpfDisplayError::Cancelled)
        ));
        drop(image);
        assert_eq!(root.used(), 0);
    }
}

#[cfg(test)]
mod alpha_display_tests {
    use super::*;
    fn fixture(alpha: [f32; 2], red: [f32; 2]) -> Vec<u8> {
        let mut b = vec![0; 744];
        for (at, value) in [
            (2, 1u16),
            (10, 1),
            (20, 3),
            (22, 1),
            (26, 0xfffd),
            (658, 32),
            (662, 32),
        ] {
            b[at..at + 2].copy_from_slice(&value.to_be_bytes());
        }
        let program = b"3ds max : ( )";
        b[400..400 + program.len()].copy_from_slice(program);
        b[740..744].copy_from_slice(&744u32.to_be_bytes());
        for values in [red, [0.25, 2.0], [0.5, 4.0], alpha] {
            b.extend(8u16.to_be_bytes());
            for v in values {
                b.extend(v.to_bits().to_be_bytes());
            }
        }
        b
    }
    #[test]
    fn straight_and_associated_hdr_colors_have_explicit_numeric_results() {
        let b = fixture([0.25, 0.5], [0.125, 1.0]);
        let file = inspect_rpf_file(&b, 0, 0, || false).unwrap();
        let root = rrrah_core::MemoryBudget::new(808);
        let image = file.decode_with_budget(&root, 0, 0, 776, || false).unwrap();
        for (mode, expected) in [
            (
                crate::RlaAlphaMode::Straight,
                [0.125, 0.25, 0.5, 0.25, 1.0, 2.0, 4.0, 0.5],
            ),
            (
                crate::RlaAlphaMode::Premultiplied,
                [0.5, 1.0, 2.0, 0.25, 2.0, 4.0, 8.0, 0.5],
            ),
        ] {
            let raster = image
                .to_raster_with_interpretation(
                    [0, 1, 2],
                    Some(0),
                    mode,
                    rrrah_core::RasterColorSpace::LinearSrgb,
                    &root,
                    || false,
                )
                .unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(p) = raster.pixels() else {
                panic!("float expected")
            };
            assert_eq!(&p[..], expected);
            drop(raster);
            assert_eq!(root.used(), 776);
        }
        drop(image);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn emission_nonfinite_alpha_and_division_overflow_refuse_without_leak() {
        for (alpha, red, emission) in [
            ([0.0, 0.5], [0.125, 1.0], true),
            ([1.1, 0.5], [0.125, 1.0], false),
            ([f32::NAN, 0.5], [0.125, 1.0], false),
            ([0.25, 0.5], [f32::NAN, 1.0], false),
            ([f32::MIN_POSITIVE, 0.5], [f32::MAX, 1.0], false),
        ] {
            let b = fixture(alpha, red);
            let file = inspect_rpf_file(&b, 0, 0, || false).unwrap();
            let root = rrrah_core::MemoryBudget::new(808);
            let image = file.decode_with_budget(&root, 0, 0, 776, || false).unwrap();
            let result = image.to_raster_with_interpretation(
                [0, 1, 2],
                Some(0),
                crate::RlaAlphaMode::Premultiplied,
                rrrah_core::RasterColorSpace::LinearSrgb,
                &root,
                || false,
            );
            if emission {
                assert!(matches!(result, Err(RpfDisplayError::ZeroAlphaEmission)));
            } else {
                assert!(matches!(result, Err(RpfDisplayError::InvalidSample)));
            }
            assert_eq!(root.used(), 776);
            drop(image);
            assert_eq!(root.used(), 0);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpfAspectError {
    InvalidText,
    InvalidValue,
}
impl RpfDecodedImage {
    /// Autodesk's header stores whole-window image aspect, not pixel aspect.
    /// Missing producer metadata remains missing, without a square-pixel fallback.
    pub fn declared_image_aspect(&self) -> Result<Option<f64>, RpfAspectError> {
        let raw = &self.metadata_bytes()[572..580];
        let raw = &raw[..raw.iter().position(|b| *b == 0).unwrap_or(raw.len())];
        let text = std::str::from_utf8(raw)
            .map_err(|_| RpfAspectError::InvalidText)?
            .trim();
        if text.is_empty() {
            return Ok(None);
        }
        let value: f64 = text.parse().map_err(|_| RpfAspectError::InvalidText)?;
        if !value.is_finite() || value <= 0.0 {
            return Err(RpfAspectError::InvalidValue);
        }
        Ok(Some(value))
    }
    /// Reverses the documented Autodesk writer formula using the full window,
    /// so cropping the active region does not change the underlying pixel shape.
    pub fn producer_pixel_aspect(&self) -> Result<Option<f64>, RpfAspectError> {
        let Some(image_aspect) = self.declared_image_aspect()? else {
            return Ok(None);
        };
        let value =
            image_aspect * f64::from(self.header.window.height()) / f64::from(self.header.window.width());
        if !value.is_finite() || value <= 0.0 {
            return Err(RpfAspectError::InvalidValue);
        }
        Ok(Some(value))
    }
}
#[cfg(test)]
mod aspect_tests {
    use super::*;
    fn image(text: &[u8], cropped: bool) -> RpfDecodedImage {
        let mut b = super::allocation_plan_tests::fixture();
        if cropped {
            b[2..4].copy_from_slice(&1u16.to_be_bytes());
        }
        b[572..580].fill(0);
        b[572..572 + text.len()].copy_from_slice(text);
        let file = inspect_rpf_file(&b, 0, 0, || false).unwrap();
        file.decode_with_budget(&rrrah_core::MemoryBudget::new(816), 1, 2, 816, || false)
            .unwrap()
    }
    #[test]
    fn image_ratio_and_pixel_ratio_are_distinct_and_crop_uses_full_window() {
        assert_eq!(
            image(b"1e308", false).producer_pixel_aspect(),
            Err(RpfAspectError::InvalidValue)
        );
        let full = image(b"1.50000", false);
        assert_eq!(full.declared_image_aspect().unwrap(), Some(1.5));
        assert_eq!(full.producer_pixel_aspect().unwrap(), Some(3.0));
        let cropped = image(b"1.50000", true);
        assert_eq!(cropped.header().active_window.width(), 1);
        assert_eq!(cropped.producer_pixel_aspect().unwrap(), Some(1.5));
        assert_eq!(image(b"", false).producer_pixel_aspect().unwrap(), None);
        assert_eq!(image(b"  \t", false).declared_image_aspect().unwrap(), None);
        for text in [b"0".as_slice(), b"-1", b"NaN", b"inf"] {
            assert_eq!(
                image(text, false).declared_image_aspect(),
                Err(RpfAspectError::InvalidValue)
            );
        }
        assert_eq!(
            image(&[255], false).declared_image_aspect(),
            Err(RpfAspectError::InvalidText)
        );
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RpfDecodeLimits {
    pub max_node_names: u32,
    pub max_node_name_bytes: u32,
    pub max_row_layers: u32,
    pub max_total_layers: u64,
    pub max_output_bytes: u64,
}
#[derive(Debug)]
pub enum RpfReadError {
    Source(crate::DecodeError),
    File(RpfFileError),
    Image(RpfImageDecodeError),
}
/// Read the first Autodesk-layout image with bounded source and decoded buffers
/// sharing the supplied root budget. Does not infer display color interpretation.
/// Additional linked images require separately qualified traversal and are not selected here.
pub fn decode_rpf_file_with_budget(
    request: &crate::DecodeRequest,
    budget: &rrrah_core::MemoryBudget,
    limits: RpfDecodeLimits,
) -> Result<RpfDecodedImage, RpfReadError> {
    use RpfReadError as E;
    request.check_cancelled().map_err(E::Source)?;
    if request.image_index != 0 {
        return Err(E::Source(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }));
    }
    let mut bounded_request = request.clone();
    bounded_request.memory_budget = Some(budget.clone());
    let source = crate::bounded_io::read_bounded(&bounded_request).map_err(E::Source)?;
    let cancelled = || {
        request
            .cancellation
            .as_ref()
            .is_some_and(crate::GenerationToken::is_cancelled)
    };
    let file = inspect_rpf_file(
        &source,
        limits.max_node_names,
        limits.max_node_name_bytes,
        cancelled,
    )
    .map_err(E::File)?;
    file.decode_with_budget(
        budget,
        limits.max_row_layers,
        limits.max_total_layers,
        limits.max_output_bytes,
        cancelled,
    )
    .map_err(E::Image)
}

#[cfg(test)]
mod file_input_tests {
    use super::*;
    #[test]
    fn file_and_decoded_buffers_share_root_cap_and_read_source_is_released() {
        let bytes = super::allocation_plan_tests::fixture();
        let path = std::env::temp_dir().join(format!(
            "rrrah-rpf-input-{}-{}.rpf",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        std::fs::write(&path, &bytes).unwrap();
        let mut request = crate::DecodeRequest::new(path);
        let limits = RpfDecodeLimits {
            max_node_names: 0,
            max_node_name_bytes: 0,
            max_row_layers: 1,
            max_total_layers: 2,
            max_output_bytes: 816,
        };
        let required = bytes.len() as u64 + 816;
        let root = rrrah_core::MemoryBudget::new(required);
        let image = decode_rpf_file_with_budget(&request, &root, limits).unwrap();
        assert_eq!(root.peak(), required);
        assert_eq!(root.used(), 816);
        assert_eq!(image.main_sample(0, 1), Some(64.0));
        drop(image);
        assert_eq!(root.used(), 0);
        let tiny = rrrah_core::MemoryBudget::new(required - 1);
        assert!(matches!(
            decode_rpf_file_with_budget(&request, &tiny, limits),
            Err(RpfReadError::Image(RpfImageDecodeError::Memory(_)))
        ));
        assert_eq!(tiny.peak(), bytes.len() as u64);
        assert_eq!(tiny.used(), 0);
        request.image_index = 1;
        let fresh = rrrah_core::MemoryBudget::new(required);
        assert!(matches!(
            decode_rpf_file_with_budget(&request, &fresh, limits),
            Err(RpfReadError::Source(crate::DecodeError::UnsupportedImageIndex {
                index: 1
            }))
        ));
        assert_eq!(fresh.peak(), 0);
        request.image_index = 0;
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
            0,
        ));
        assert!(matches!(
            decode_rpf_file_with_budget(&request, &fresh, limits),
            Err(RpfReadError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(fresh.peak(), 0);
    }
}

/// Explicit display interpretation; RPF has no universally inferred RGB profile
/// or alpha association. Channel indices address color and matte banks separately.
#[derive(Debug, Clone)]
pub struct RpfRasterInterpretation {
    pub rgb: [u16; 3],
    pub matte: Option<u16>,
    pub alpha_mode: crate::RlaAlphaMode,
    pub color_space: rrrah_core::RasterColorSpace,
}

#[derive(Debug)]
pub enum RpfRasterReadError {
    Read(RpfReadError),
    Aspect(RpfAspectError),
    Display(RpfDisplayError),
}

/// Imports one explicitly interpreted raster and its optional producer pixel
/// aspect. Source, packed channels and output share one root; only output pixels
/// remain retained on success. Use `decode_rpf_file_with_budget` to retain GB
/// channels, layers and producer metadata instead. Does not apply display color
/// conversion or infer a missing aspect ratio.
pub fn decode_rpf_file_raster_with_budget(
    request: &crate::DecodeRequest,
    budget: &rrrah_core::MemoryBudget,
    limits: RpfDecodeLimits,
    interpretation: &RpfRasterInterpretation,
) -> Result<(rrrah_core::DecodedRaster, Option<f64>), RpfRasterReadError> {
    let image = decode_rpf_file_with_budget(request, budget, limits).map_err(RpfRasterReadError::Read)?;
    let aspect = image
        .producer_pixel_aspect()
        .map_err(RpfRasterReadError::Aspect)?;
    let raster = image
        .to_raster_with_interpretation(
            interpretation.rgb,
            interpretation.matte,
            interpretation.alpha_mode,
            interpretation.color_space.clone(),
            budget,
            || {
                request
                    .cancellation
                    .as_ref()
                    .is_some_and(crate::GenerationToken::is_cancelled)
            },
        )
        .map_err(RpfRasterReadError::Display)?;
    Ok((raster, aspect))
}

#[cfg(test)]
mod file_raster_tests {
    use super::*;
    #[test]
    fn explicit_file_raster_preserves_hdr_alpha_aspect_and_releases_intermediates() {
        let mut bytes = vec![0; 744];
        for (at, value) in [
            (2, 1u16),
            (10, 1),
            (20, 3),
            (22, 1),
            (26, 0xfffd),
            (658, 32),
            (662, 32),
        ] {
            bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
        }
        bytes[400..413].copy_from_slice(b"3ds max : ( )");
        bytes[572..575].copy_from_slice(b"3.0");
        bytes[740..744].copy_from_slice(&744u32.to_be_bytes());
        for channel in [[0.125f32, 1.0], [0.25, 2.0], [0.5, 4.0], [0.25, 0.5]] {
            bytes.extend(8u16.to_be_bytes());
            for sample in channel {
                bytes.extend(sample.to_bits().to_be_bytes());
            }
        }
        let path = std::env::temp_dir().join(format!(
            "rrrah-rpf-raster-{}-{}.rpf",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        std::fs::write(&path, &bytes).unwrap();
        let request = crate::DecodeRequest::new(&path);
        let packed = inspect_rpf_file(&bytes, 0, 0, || false)
            .unwrap()
            .allocation_plan(0, 0, 4096, || false)
            .unwrap()
            .total_bytes;
        let root_limit = bytes.len() as u64 + packed;
        let limits = RpfDecodeLimits {
            max_node_names: 0,
            max_node_name_bytes: 0,
            max_row_layers: 0,
            max_total_layers: 0,
            max_output_bytes: packed,
        };
        for (alpha_mode, expected) in [
            (
                crate::RlaAlphaMode::Straight,
                [0.125f32, 0.25, 0.5, 0.25, 1., 2., 4., 0.5],
            ),
            (
                crate::RlaAlphaMode::Premultiplied,
                [0.5, 1., 2., 0.25, 2., 4., 8., 0.5],
            ),
        ] {
            let root = rrrah_core::MemoryBudget::new(root_limit);
            let interpretation = RpfRasterInterpretation {
                rgb: [0, 1, 2],
                matte: Some(0),
                alpha_mode,
                color_space: rrrah_core::RasterColorSpace::LinearSrgb,
            };
            let (raster, aspect) =
                decode_rpf_file_raster_with_budget(&request, &root, limits, &interpretation).unwrap();
            assert_eq!(aspect, Some(1.5));
            let rrrah_core::RasterPixels::Rgba32Float(samples) = raster.pixels() else {
                panic!("float raster required")
            };
            assert!(
                samples
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
            assert_eq!(root.used(), 32);
            assert_eq!(root.peak(), root_limit);
            let clone = raster.clone();
            drop(raster);
            assert_eq!(root.used(), 32);
            drop(clone);
            assert_eq!(root.used(), 0);
            let tiny = rrrah_core::MemoryBudget::new(root_limit - 1);
            assert!(decode_rpf_file_raster_with_budget(&request, &tiny, limits, &interpretation).is_err());
            assert_eq!(tiny.used(), 0);
            let invalid = RpfRasterInterpretation {
                rgb: [0, 1, 3],
                ..interpretation
            };
            assert!(matches!(
                decode_rpf_file_raster_with_budget(&request, &root, limits, &invalid),
                Err(RpfRasterReadError::Display(RpfDisplayError::ChannelSelection))
            ));
            assert_eq!(root.used(), 0);
        }
    }
}
