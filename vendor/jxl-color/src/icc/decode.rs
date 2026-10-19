use std::io::Cursor;
use std::io::prelude::*;

use jxl_bitstream::Bitstream;

use crate::{Error, Result};

/// Reads the encoded ICC profile stream from the given bitstream.
pub fn read_icc(bitstream: &mut Bitstream) -> Result<Vec<u8>> {
    read_icc_with_admission(bitstream, |_| Ok(()))
}

/// Read ICC after admitting its encoded/reconstructed/shuffle buffer high water.
///
/// The callback receives an absolute byte bound (not an incremental charge). It
/// must retain admitted credit while returned ICC data and reconstruction live.
/// The same bound is polled every 4096 encoded symbols for cancellation.
/// Entropy decoder state and profile parsing are outside this bound.
pub fn read_icc_with_admission(
    bitstream: &mut Bitstream,
    mut admit: impl FnMut(u64) -> Result<()>,
) -> Result<Vec<u8>> {
    let enc_size = bitstream.read_u64()?;
    tracing::trace!(enc_size);

    if enc_size > (1 << 28) {
        // Avoids allocating too much memory (>256MiB)
        // Maximum ICC output_size for Level 10
        return Err(jxl_bitstream::Error::ProfileConformance("Too large encoded ICC profile").into());
    }

    // Include up to 18 bytes of old header storage during exact reallocation.
    admit(enc_size + 18)?;
    let mut decoder = jxl_coding::Decoder::parse(bitstream, 41)?;

    let max_size_header_len = enc_size.min(18) as usize;
    let mut encoded_icc = vec![0u8; max_size_header_len];
    let mut b1 = 0u8;
    let mut b2 = 0u8;
    decoder.begin(bitstream)?;
    for (idx, b) in encoded_icc.iter_mut().enumerate() {
        let sym = decoder.read_varint(bitstream, get_icc_ctx(idx, b1, b2))?;
        if sym >= 256 {
            return Err(Error::InvalidIccStream("decoded value out of range"));
        }
        *b = sym as u8;

        b2 = b1;
        b1 = *b;
    }

    let mut tmp_cursor = Cursor::new(&*encoded_icc);
    let output_size = varint(&mut tmp_cursor)?;
    let commands_size = varint(&mut tmp_cursor)?;
    let stream_offset = tmp_cursor.position();
    tracing::trace!(
        enc_size,
        output_size,
        commands_size,
        stream_offset,
        "Quick ICC validity check",
    );

    if stream_offset + commands_size > enc_size {
        return Err(Error::InvalidIccStream("invalid commands_size"));
    }

    if output_size > (1 << 28) {
        return Err(jxl_bitstream::Error::ProfileConformance("ICC output_size too large").into());
    }

    if output_size + 65536 < enc_size {
        return Err(Error::InvalidIccStream(
            "reported output_size is far smaller than enc_size",
        ));
    }

    // Output plus at most one output-sized shuffle buffer.
    admit(enc_size + 2 * output_size + 18)?;
    // Avoid geometric Vec growth beyond the admitted encoded-buffer size.
    encoded_icc
        .try_reserve_exact(enc_size as usize - encoded_icc.len())
        .map_err(|e| Error::AllocationDenied(Box::new(e)))?;
    // Read remaining data
    encoded_icc.resize(enc_size as usize, 0);
    for (idx, b) in encoded_icc.iter_mut().enumerate().skip(max_size_header_len) {
        // Recheck the same high-water credit periodically so callers can
        // cancel stale work without waiting for the full ICC entropy stream.
        if idx % 4096 == 0 {
            admit(enc_size + 2 * output_size + 18)?;
        }
        let sym = decoder.read_varint(bitstream, get_icc_ctx(idx, b1, b2))?;
        if sym >= 256 {
            return Err(Error::InvalidIccStream("decoded value out of range"));
        }
        *b = sym as u8;

        b2 = b1;
        b1 = *b;
    }

    decoder.finalize()?;
    Ok(encoded_icc)
}

fn get_icc_ctx(idx: usize, b1: u8, b2: u8) -> u32 {
    if idx <= 128 {
        return 0;
    }

    let p1 = match b1 {
        b'a'..=b'z' | b'A'..=b'Z' => 0,
        b'0'..=b'9' | b'.' | b',' => 1,
        0..=1 => 2 + b1 as u32,
        2..=15 => 4,
        241..=254 => 5,
        255 => 6,
        _ => 7,
    };
    let p2 = match b2 {
        b'a'..=b'z' | b'A'..=b'Z' => 0,
        b'0'..=b'9' | b'.' | b',' => 1,
        0..=15 => 2,
        241..=255 => 3,
        _ => 4,
    };

    1 + p1 + 8 * p2
}

fn varint(stream: &mut Cursor<&[u8]>) -> Result<u64> {
    let mut value = 0u64;
    let mut shift = 0;
    let mut b = 0;
    while shift < 63 {
        stream
            .read_exact(std::slice::from_mut(&mut b))
            .map_err(|_| Error::InvalidIccStream("stream is too short"))?;
        value |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    Ok(value)
}

fn predict_header(idx: usize, output_size: u32, header: &[u8]) -> u8 {
    match idx {
        0..=3 => output_size.to_be_bytes()[idx],
        8 => 4,
        12..=23 => b"mntrRGB XYZ "[idx - 12],
        36..=39 => b"acsp"[idx - 36],
        // APPL
        41 | 42 if header[40] == b'A' => b'P',
        43 if header[40] == b'A' => b'L',
        // MSFT
        41 if header[40] == b'M' => b'S',
        42 if header[40] == b'M' => b'F',
        43 if header[40] == b'M' => b'T',
        // SGI_
        42 if header[40] == b'S' && header[41] == b'G' => b'I',
        43 if header[40] == b'S' && header[41] == b'G' => b' ',
        // SUNW
        42 if header[40] == b'S' && header[41] == b'U' => b'N',
        43 if header[40] == b'S' && header[41] == b'U' => b'W',
        70 => 246,
        71 => 214,
        73 => 1,
        78 => 211,
        79 => 45,
        80..=83 => header[4 + idx - 80],
        _ => 0,
    }
}

fn shuffle2(bytes: &[u8], check: &mut impl FnMut() -> Result<()>) -> Result<Vec<u8>> {
    check()?;
    let len = bytes.len();
    let mut out = Vec::with_capacity(bytes.len());
    let height = len / 2;
    let odd = len % 2;
    for idx in 0..height {
        if idx != 0 && idx % 4096 == 0 {
            check()?;
        }
        out.push(bytes[idx]);
        out.push(bytes[idx + height + odd]);
    }
    if odd != 0 {
        out.push(bytes[height]);
    }
    Ok(out)
}

fn shuffle4(bytes: &[u8], check: &mut impl FnMut() -> Result<()>) -> Result<Vec<u8>> {
    check()?;
    let len = bytes.len();
    let mut out = Vec::with_capacity(bytes.len());
    let step = len / 4;
    let wide_count = len % 4;
    for idx in 0..step {
        if idx != 0 && idx % 4096 == 0 {
            check()?;
        }
        let mut base = idx;
        for _ in 0..wide_count {
            out.push(bytes[base]);
            base += step + 1;
        }
        for _ in wide_count..4 {
            out.push(bytes[base]);
            base += step;
        }
    }
    for idx in 1..=wide_count {
        out.push(bytes[(step + 1) * idx - 1]);
    }
    Ok(out)
}

fn check_output_size(current: usize, additional: usize, limit: usize) -> Result<()> {
    if current.checked_add(additional).is_none_or(|size| size > limit) {
        return Err(Error::InvalidIccStream(
            "decoded ICC profile exceeds declared size",
        ));
    }
    Ok(())
}

fn extend_output(out: &mut Vec<u8>, limit: usize, bytes: &[u8]) -> Result<()> {
    check_output_size(out.len(), bytes.len(), limit)?;
    out.extend_from_slice(bytes);
    Ok(())
}

fn finish_output(out: Vec<u8>, expected: usize) -> Result<Vec<u8>> {
    if out.len() != expected {
        return Err(Error::InvalidIccStream("decoded ICC profile size mismatch"));
    }
    Ok(out)
}

/// Decodes the given ICC profile stream.
pub fn decode_icc(stream: &[u8]) -> Result<Vec<u8>> {
    decode_icc_with_check(stream, || Ok(()))
}

/// Reconstruct ICC with cancellation checks before output allocation, between
/// commands, every 4096 shuffle/predictor iterations, and every 64 KiB copy.
pub fn decode_icc_with_check(stream: &[u8], mut check: impl FnMut() -> Result<()>) -> Result<Vec<u8>> {
    use std::num::Wrapping;

    const COMMON_TAGS: [&[u8]; 19] = [
        b"rTRC", b"rXYZ", b"cprt", b"wtpt", b"bkpt", b"rXYZ", b"gXYZ", b"bXYZ", b"kXYZ", b"rTRC", b"gTRC",
        b"bTRC", b"kTRC", b"chad", b"desc", b"chrm", b"dmnd", b"dmdd", b"lumi",
    ];

    const COMMON_DATA: [&[u8]; 8] = [
        b"XYZ ", b"desc", b"text", b"mluc", b"para", b"curv", b"sf32", b"gbd ",
    ];

    let mut tmp_cursor = Cursor::new(stream);
    let output_size = varint(&mut tmp_cursor)?;
    let commands_size = varint(&mut tmp_cursor)?;
    let stream_offset = tmp_cursor.position();
    if stream_offset + commands_size > stream.len() as u64 {
        return Err(Error::InvalidIccStream("invalid commands_size"));
    }

    if output_size > (1 << 28) {
        return Err(jxl_bitstream::Error::ProfileConformance("ICC output_size too large").into());
    }

    let (commands, data) = stream[stream_offset as usize..].split_at(commands_size as usize);
    let header_size = output_size.min(128) as usize;
    if data.len() < header_size {
        return Err(Error::InvalidIccStream("invalid output_size"));
    }
    let (header_data, mut data) = data.split_at(header_size);
    let mut commands_stream = Cursor::new(commands);
    check()?;
    let mut out = Vec::with_capacity(output_size as usize);

    // Header
    for (idx, &e) in header_data.iter().enumerate() {
        let p = predict_header(idx, output_size as u32, header_data);
        extend_output(&mut out, output_size as usize, &[p.wrapping_add(e)])?;
    }
    if output_size <= 128 {
        return Ok(out);
    }

    // Tag
    let v = varint(&mut commands_stream)?;
    if let Some(num_tags) = v.checked_sub(1) {
        if (output_size - 128) / 12 < num_tags {
            return Err(Error::InvalidIccStream("num_tags too large"));
        }
        let num_tags = num_tags as u32;
        extend_output(&mut out, output_size as usize, &num_tags.to_be_bytes())?;

        let mut prev_tagstart = num_tags * 12 + 128;
        let mut prev_tagsize = 0u32;

        loop {
            check()?;
            let mut command = 0u8;
            if commands_stream
                .read_exact(std::slice::from_mut(&mut command))
                .is_err()
            {
                return finish_output(out, output_size as usize);
            }
            let tagcode = command & 63;
            let tag = match tagcode {
                0 => break,
                1 => {
                    if data.len() < 4 {
                        return Err(Error::InvalidIccStream("unexpected end of data stream"));
                    }
                    let (tag, next_data) = data.split_at(4);
                    data = next_data;
                    tag
                }
                2..=20 => COMMON_TAGS[(tagcode - 2) as usize],
                _ => return Err(Error::InvalidIccStream("invalid tagcode")),
            };

            let tagstart = if command & 64 == 0 {
                prev_tagstart + prev_tagsize
            } else {
                varint(&mut commands_stream)? as u32
            };
            let tagsize = match tag {
                _ if command & 128 != 0 => varint(&mut commands_stream)? as u32,
                b"rXYZ" | b"gXYZ" | b"bXYZ" | b"kXYZ" | b"wtpt" | b"bkpt" | b"lumi" => 20,
                _ => prev_tagsize,
            };
            if (tagstart as u64 + tagsize as u64) > output_size {
                return Err(Error::InvalidIccStream("ICC profile size mismatch"));
            }

            prev_tagstart = tagstart;
            prev_tagsize = tagsize;

            extend_output(&mut out, output_size as usize, tag)?;
            extend_output(&mut out, output_size as usize, &tagstart.to_be_bytes())?;
            extend_output(&mut out, output_size as usize, &tagsize.to_be_bytes())?;
            if tagcode == 2 {
                extend_output(&mut out, output_size as usize, b"gTRC")?;
                extend_output(&mut out, output_size as usize, &tagstart.to_be_bytes())?;
                extend_output(&mut out, output_size as usize, &tagsize.to_be_bytes())?;
                extend_output(&mut out, output_size as usize, b"bTRC")?;
                extend_output(&mut out, output_size as usize, &tagstart.to_be_bytes())?;
                extend_output(&mut out, output_size as usize, &tagsize.to_be_bytes())?;
            } else if tagcode == 3 {
                extend_output(&mut out, output_size as usize, b"gXYZ")?;
                extend_output(
                    &mut out,
                    output_size as usize,
                    &(tagstart + tagsize).to_be_bytes(),
                )?;
                extend_output(&mut out, output_size as usize, &tagsize.to_be_bytes())?;
                extend_output(&mut out, output_size as usize, b"bXYZ")?;
                extend_output(
                    &mut out,
                    output_size as usize,
                    &(tagstart + tagsize * 2).to_be_bytes(),
                )?;
                extend_output(&mut out, output_size as usize, &tagsize.to_be_bytes())?;
            }
        }
    }

    // Main
    let mut command = 0u8;
    while commands_stream
        .read_exact(std::slice::from_mut(&mut command))
        .is_ok()
    {
        check()?;
        match command {
            1 => {
                let num = varint(&mut commands_stream)? as usize;
                if num > data.len() {
                    return Err(Error::InvalidIccStream("stream is too short"));
                }
                check_output_size(out.len(), num, output_size as usize)?;
                let (bytes, next_data) = data.split_at(num);
                data = next_data;
                for chunk in bytes.chunks(65536) {
                    check()?;
                    extend_output(&mut out, output_size as usize, chunk)?;
                }
            }
            2 | 3 => {
                let num = varint(&mut commands_stream)? as usize;
                if num > data.len() {
                    return Err(Error::InvalidIccStream("stream is too short"));
                }
                check_output_size(out.len(), num, output_size as usize)?;
                let (bytes, next_data) = data.split_at(num);
                data = next_data;
                let bytes = if command == 2 {
                    shuffle2(bytes, &mut check)?
                } else {
                    shuffle4(bytes, &mut check)?
                };
                for chunk in bytes.chunks(65536) {
                    check()?;
                    extend_output(&mut out, output_size as usize, chunk)?;
                }
            }
            4 => {
                let mut flags = 0u8;
                commands_stream
                    .read_exact(std::slice::from_mut(&mut flags))
                    .map_err(|_| Error::InvalidIccStream("stream is too short"))?;
                let width = ((flags & 3) + 1) as usize;
                let order = (flags >> 2) & 3;
                if width == 3 || order == 3 {
                    return Err(Error::InvalidIccStream("width == 3 || order == 3"));
                }

                let stride = if (flags & 16) == 0 {
                    width
                } else {
                    let stride = varint(&mut commands_stream)? as usize;
                    if stride < width {
                        return Err(Error::InvalidIccStream("stride < width"));
                    }
                    stride
                };
                if stride.saturating_mul(4) >= out.len() {
                    return Err(Error::InvalidIccStream("stride * 4 >= out.len()"));
                }

                let num = varint(&mut commands_stream)? as usize;
                if data.len() < num {
                    return Err(Error::InvalidIccStream("stream is too short"));
                }
                check_output_size(out.len(), num, output_size as usize)?;
                let (bytes, next_data) = data.split_at(num);
                data = next_data;
                let shuffled;
                let bytes = match width {
                    1 => bytes,
                    2 => {
                        shuffled = shuffle2(bytes, &mut check)?;
                        &shuffled
                    }
                    4 => {
                        shuffled = shuffle4(bytes, &mut check)?;
                        &shuffled
                    }
                    _ => unreachable!(),
                };

                for i in (0..num).step_by(width) {
                    if i % 4096 == 0 {
                        check()?;
                    }
                    let mut prev = [Wrapping(0u32); 3];
                    for (j, p) in prev[..=order as usize].iter_mut().enumerate() {
                        let offset = out.len() - stride * (j + 1);
                        let mut bytes = [0u8; 4];
                        bytes[(4 - width)..].copy_from_slice(&out[offset..][..width]);
                        p.0 = u32::from_be_bytes(bytes);
                    }
                    let p = match order {
                        0 => prev[0],
                        1 => Wrapping(2) * prev[0] - prev[1],
                        2 => Wrapping(3) * (prev[0] - prev[1]) + prev[2],
                        _ => unreachable!(),
                    };

                    for j in 0..width.min(num - i) {
                        let val = Wrapping(bytes[i + j] as u32) + (p >> (8 * (width - 1 - j)));
                        extend_output(&mut out, output_size as usize, &[val.0 as u8])?;
                    }
                }
            }
            10 => {
                if data.len() < 12 {
                    return Err(Error::InvalidIccStream("stream is too short"));
                }
                extend_output(
                    &mut out,
                    output_size as usize,
                    &[b'X', b'Y', b'Z', b' ', 0, 0, 0, 0],
                )?;
                let (bytes, next_data) = data.split_at(12);
                data = next_data;
                extend_output(&mut out, output_size as usize, bytes)?;
            }
            16..=23 => {
                extend_output(&mut out, output_size as usize, COMMON_DATA[command as usize - 16])?;
                extend_output(&mut out, output_size as usize, &[0, 0, 0, 0])?;
            }
            _ => {
                return Err(Error::InvalidIccStream("invalid command"));
            }
        }
    }
    finish_output(out, output_size as usize)
}

#[cfg(test)]
mod bounded_profile_tests {
    use super::*;
    fn stream(size: u64, commands: &[u8], payload: &[u8]) -> Vec<u8> {
        fn write(mut n: u64, out: &mut Vec<u8>) {
            while n >= 128 {
                out.push((n as u8 & 127) | 128);
                n >>= 7;
            }
            out.push(n as u8);
        }
        let mut out = Vec::new();
        write(size, &mut out);
        write(commands.len() as u64, &mut out);
        out.extend_from_slice(commands);
        out.extend_from_slice(payload);
        out
    }
    #[test]
    fn truncated_tag_table_cannot_succeed_with_wrong_output_length() {
        // Header plus an advertised zero-entry tag table cannot fit 129 bytes.
        assert!(decode_icc(&stream(129, &[1], &[0; 128])).is_err());
    }
    #[test]
    fn truncated_nonempty_tag_table_cannot_succeed() {
        assert!(decode_icc(&stream(144, &[2], &[0; 128])).is_err());
    }
    #[test]
    fn every_expanding_command_refuses_before_growing_output() {
        let cases: &[&[u8]] = &[
            &[0, 1, 2],
            &[0, 2, 2],
            &[0, 3, 2],
            &[0, 4, 0, 2],
            &[0, 4, 1, 2],
            &[0, 4, 3, 2],
            &[0, 10],
            &[0, 16],
            &[0, 17],
            &[0, 18],
            &[0, 19],
            &[0, 20],
            &[0, 21],
            &[0, 22],
            &[0, 23],
            // Zero-entry table plus a tag descriptor cannot fit 132 bytes.
            &[1, 4],
            // The first descriptor fits, but the implicit G/B descriptors do not.
            &[2, 2],
            &[2, 3],
        ];
        for commands in cases {
            let size = if *commands == [2, 3] {
                164
            } else if commands[0] == 2 {
                144
            } else if *commands == [1, 4] {
                132
            } else {
                129
            };
            let error = decode_icc(&stream(size, commands, &[0; 140])).unwrap_err();
            assert!(
                matches!(
                    error,
                    Error::InvalidIccStream("decoded ICC profile exceeds declared size")
                ),
                "{commands:?}: {error}"
            );
        }
    }
    #[test]
    fn exact_header_table_and_main_command_sizes_remain_valid() {
        for (size, commands) in [(128, &[][..]), (132, &[1, 0][..]), (136, &[0, 16][..])] {
            assert_eq!(
                decode_icc(&stream(size, commands, &[0; 128])).unwrap().len(),
                size as usize
            );
        }
    }
    #[test]
    fn long_copy_shuffle_and_predictor_commands_can_cancel_inside_work() {
        let payload = vec![0; 128 + 131072];
        for (command, flags) in [
            (1, None),
            (2, None),
            (3, None),
            (4, Some(0)),
            (4, Some(1)),
            (4, Some(3)),
        ] {
            let mut commands = vec![0, command];
            if let Some(flags) = flags {
                commands.push(flags);
            }
            commands.extend_from_slice(&[128, 128, 8]); // 131072 payload bytes.
            let source = stream(payload.len() as u64, &commands, &payload);
            let expected = decode_icc(&source).unwrap();
            assert_eq!(expected.len(), payload.len());
            let mut calls = 0;
            let result = decode_icc_with_check(&source, || {
                calls += 1;
                // Initial output and command checks have passed; this reaches
                // the second copy chunk or an internal shuffle/predictor loop.
                if calls == 4 {
                    return Err(Error::AllocationDenied(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "cancel test",
                    ))));
                }
                Ok(())
            });
            assert!(
                matches!(result, Err(Error::AllocationDenied(_))),
                "command {command} flags {flags:?}"
            );
            assert_eq!(calls, 4);
            let mut retry_calls = 0;
            assert_eq!(
                decode_icc_with_check(&source, || {
                    retry_calls += 1;
                    Ok(())
                })
                .unwrap(),
                expected
            );
            assert!(retry_calls >= 4);
        }
    }
}
