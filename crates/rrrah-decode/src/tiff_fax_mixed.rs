//! Application-owned T.4/T.6 rows using fax Huffman tables.
use super::{DecodeRequest, RasterDecodeError, bad};
use fax::{
    BitReader,
    maps::{Mode, black, mode, white},
};
struct Bits<'a> {
    source: &'a [u8],
    at: usize,
    end: usize,
    reversed: bool,
    request: &'a DecodeRequest,
}
impl BitReader for Bits<'_> {
    type Error = ();
    fn peek(&self, n: u8) -> Option<u16> {
        if self.at >= self.end || self.request.check_cancelled().is_err() {
            return None;
        }
        // Huffman table lookup may inspect beyond physical EOF. Only peek is
        // padded; consume always requires actual bits, so no decoded codeword
        // can consume synthetic input.
        let byte = self.at / 8;
        let mut window = 0u32;
        for offset in 0..3 {
            let value = self.source.get(byte + offset).copied().unwrap_or(0);
            window = (window << 8)
                | u32::from(if self.reversed {
                    value.reverse_bits()
                } else {
                    value
                });
        }
        let count = usize::from(n);
        let value = (window >> (24 - self.at % 8 - count)) & ((1u32 << count) - 1);
        let actual = (self.end - self.at).min(count);
        let padding = count - actual;
        Some(((value >> padding) << padding) as u16)
    }
    fn consume(&mut self, n: u8) -> Result<(), ()> {
        if self.at + usize::from(n) > self.end {
            return Err(());
        }
        if self.at >= self.end || self.request.check_cancelled().is_err() {
            return Err(());
        }
        self.at += usize::from(n);
        Ok(())
    }
    fn bits_to_byte_boundary(&self) -> u8 {
        ((8 - self.at % 8) % 8) as u8
    }
}
impl Bits<'_> {
    fn bit(&mut self) -> Result<u16, RasterDecodeError> {
        let v = self.peek(1).ok_or_else(|| bad("truncated mixed fax bits"))?;
        self.consume(1).map_err(|_| bad("mixed fax bit read"))?;
        Ok(v)
    }
    fn eol(&mut self) -> Result<(), RasterDecodeError> {
        let mut zeros = 0;
        while self.bit()? == 0 {
            zeros += 1;
            if zeros > 18 {
                return Err(bad("excessive mixed fax fill"));
            }
        }
        if zeros < 11 {
            return Err(bad("short mixed fax EOL"));
        }
        Ok(())
    }
    fn run(&mut self, black_run: bool) -> Result<u16, RasterDecodeError> {
        let mut total = 0u16;
        loop {
            let n = if black_run {
                black::decode(self)
            } else {
                white::decode(self)
            }
            .ok_or_else(|| bad("invalid mixed fax run"))?;
            total = total
                .checked_add(n)
                .ok_or_else(|| bad("mixed fax run overflow"))?;
            if n < 64 {
                return Ok(total);
            }
        }
    }
}
#[derive(Clone, Copy)]
enum Encoding {
    Group3OneD,
    Group3Mixed,
    Group4,
    Rle { word_aligned: bool },
}
pub(super) struct Mixed<'a> {
    bits: Bits<'a>,
    width: u16,
    previous: Vec<u16>,
    current: Vec<u16>,
    first: bool,
    encoding: Encoding,
}
impl<'a> Mixed<'a> {
    pub(super) fn new(
        source: &'a [u8],
        fill: u64,
        width: u16,
        request: &'a DecodeRequest,
        mixed: bool,
    ) -> Self {
        Self {
            bits: Bits {
                source,
                at: 0,
                end: source.len() * 8,
                reversed: fill == 2,
                request,
            },
            width,
            previous: Vec::new(),
            current: Vec::new(),
            first: true,
            encoding: if mixed {
                Encoding::Group3Mixed
            } else {
                Encoding::Group3OneD
            },
        }
    }
    pub(super) fn new_group4(source: &'a [u8], fill: u64, width: u16, request: &'a DecodeRequest) -> Self {
        let mut decoder = Self::new(source, fill, width, request, true);
        decoder.encoding = Encoding::Group4;
        decoder
    }
    pub(super) fn new_rle(
        source: &'a [u8],
        fill: u64,
        width: u16,
        request: &'a DecodeRequest,
        word_aligned: bool,
    ) -> Self {
        let mut decoder = Self::new(source, fill, width, request, false);
        decoder.encoding = Encoding::Rle { word_aligned };
        decoder
    }
    pub(super) fn advance<const CURSOR: bool>(&mut self) -> Result<(), RasterDecodeError> {
        let result = self.decode_row::<CURSOR>();
        self.bits.request.check_cancelled()?;
        result
    }
    fn decode_row<const CURSOR: bool>(&mut self) -> Result<(), RasterDecodeError> {
        self.bits.request.check_cancelled()?;
        let full_end = self.bits.source.len() * 8;
        self.bits.end = full_end.min(
            self.bits
                .at
                .saturating_add((usize::from(self.width) * 4 + 512) * 8),
        );
        let one_d = match self.encoding {
            Encoding::Group4 => false,
            Encoding::Rle { .. } => true,
            Encoding::Group3OneD => {
                self.bits.eol()?;
                true
            }
            Encoding::Group3Mixed => {
                self.bits.eol()?;
                self.bits.bit()? == 1
            }
        };
        if self.first && !one_d && matches!(self.encoding, Encoding::Group3Mixed) {
            return Err(bad("mixed fax first row must be 1D"));
        }
        self.current.clear();
        let mut a = 0u16;
        let mut black_run = false;
        let mut start = true;
        let mut operations = 0usize;
        let mut reference_index = 0usize;
        while a < self.width {
            self.bits.request.check_cancelled()?;
            operations += 1;
            if operations > usize::from(self.width) * 4 + 32 {
                return Err(bad("mixed fax row work limit"));
            }
            if one_d {
                a = a
                    .checked_add(self.bits.run(black_run)?)
                    .ok_or_else(|| bad("mixed fax position overflow"))?;
                if a > self.width {
                    return Err(bad("mixed fax row exceeds width"));
                }
                if a < self.width {
                    self.current.push(a);
                }
                black_run = !black_run;
            } else {
                // Even edges enter black. b1 enters the opposite of current color,
                // strictly after a0 except for the imaginary white start position.
                let mut index = if CURSOR {
                    while self
                        .previous
                        .get(reference_index)
                        .is_some_and(|&edge| if start { edge < a } else { edge <= a })
                    {
                        if reference_index % 4096 == 0 {
                            self.bits.request.check_cancelled()?;
                        }
                        reference_index += 1;
                    }
                    reference_index
                } else {
                    self.previous
                        .partition_point(|&edge| if start { edge < a } else { edge <= a })
                };
                if (index % 2 == 0) != !black_run {
                    index += 1;
                }
                let found = self.previous.get(index).copied().map(|edge| (index, edge));
                let b1 = found.map_or(self.width, |(_, e)| e);
                match mode::decode(&mut self.bits).ok_or_else(|| bad("invalid mixed fax mode"))? {
                    Mode::Pass => {
                        let b2 = found
                            .and_then(|(i, _)| self.previous.get(i + 1))
                            .copied()
                            .unwrap_or(self.width);
                        if b2 <= a {
                            return Err(bad("nonprogressing mixed fax pass"));
                        }
                        a = b2;
                    }
                    Mode::Vertical(delta) => {
                        let next = i32::from(b1) + i32::from(delta);
                        if next < i32::from(a) || next > i32::from(self.width) {
                            return Err(bad("mixed fax vertical bounds"));
                        }
                        a = next as u16;
                        if a < self.width {
                            self.current.push(a);
                        }
                        black_run = !black_run;
                    }
                    Mode::Horizontal => {
                        let first = self.bits.run(black_run)?;
                        let second = self.bits.run(!black_run)?;
                        let middle = a
                            .checked_add(first)
                            .ok_or_else(|| bad("mixed fax horizontal overflow"))?;
                        let next = middle
                            .checked_add(second)
                            .ok_or_else(|| bad("mixed fax horizontal overflow"))?;
                        if next > self.width || next <= a {
                            return Err(bad("mixed fax horizontal bounds"));
                        }
                        if middle < self.width {
                            self.current.push(middle);
                        }
                        if next < self.width {
                            self.current.push(next);
                        }
                        a = next;
                    }
                    Mode::Extension | Mode::EOF => return Err(bad("mixed fax unexpected extension or end")),
                }
            }
            if self.current.len() > usize::from(self.width) + 1 {
                return Err(bad("excessive mixed fax transitions"));
            }
            start = false;
        }
        // Preserve a width sentinel for the shared renderer, but omit it in the
        // reference row where it is an imaginary boundary, not a color change.
        if matches!(self.encoding, Encoding::Group3Mixed | Encoding::Group4) {
            self.previous.clear();
            self.previous.extend_from_slice(&self.current);
        }
        if let Encoding::Rle { word_aligned } = self.encoding {
            let alignment = if word_aligned { 16 } else { 8 };
            let padding = ((alignment - self.bits.at % alignment) % alignment) as u8;
            if padding > 0 {
                self.bits
                    .consume(padding)
                    .map_err(|_| bad("truncated RLE row padding"))?;
            }
        }
        self.current.push(self.width);
        self.first = false;
        self.bits.end = full_end;
        Ok(())
    }
    pub(super) fn transitions(&self) -> &[u16] {
        &self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference(bits: &Bits<'_>, n: u8) -> Option<u16> {
        if bits.at >= bits.end || bits.request.check_cancelled().is_err() {
            return None;
        }
        let mut value = 0u16;
        for i in 0..usize::from(n) {
            let p = bits.at + i;
            let bit = if p >= bits.end {
                0
            } else {
                (bits.source[p / 8] >> if bits.reversed { p % 8 } else { 7 - p % 8 }) & 1
            };
            value = (value << 1) | u16::from(bit);
        }
        Some(value)
    }
    #[test]
    fn three_byte_windows_match_bitwise_reference_at_all_alignments_and_eof() {
        let request = DecodeRequest::new("window-control.tiff");
        for seed in [0u8, 1, 53, 128, 255] {
            let source = (0..33)
                .map(|i| seed.wrapping_add((i * 71) as u8))
                .collect::<Vec<_>>();
            for reversed in [false, true] {
                for at in 0..=source.len() * 8 {
                    for remaining in 0..=17 {
                        let end = (at + remaining).min(source.len() * 8);
                        let bits = Bits {
                            source: &source,
                            at,
                            end,
                            reversed,
                            request: &request,
                        };
                        for n in 0..=16 {
                            assert_eq!(
                                bits.peek(n),
                                reference(&bits, n),
                                "seed={seed} at={at} end={end} reversed={reversed} n={n}"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn cancellation_between_lookup_and_consumption_keeps_position() {
        use std::sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        };
        let generation = Arc::new(AtomicU64::new(0));
        let mut request = DecodeRequest::new("window-cancel.tiff");
        request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 0));
        let source = [255];
        let mut bits = Bits {
            source: &source,
            at: 0,
            end: 8,
            reversed: false,
            request: &request,
        };
        assert_eq!(bits.peek(1), Some(1));
        generation.store(1, Ordering::SeqCst);
        assert!(bits.consume(1).is_err());
        assert_eq!(bits.at, 0);
        assert!(matches!(
            request.check_cancelled(),
            Err(crate::DecodeError::Cancelled)
        ));
    }
    #[test]
    #[ignore = "isolated CPU bit-window microbenchmark; no viewer/FPS claim"]
    fn bit_window_alternating_microbenchmark() {
        use std::{hint::black_box, time::Instant};
        let request = DecodeRequest::new("window-bench.tiff");
        let source = (0..4096).map(|i| (i * 71 + 53) as u8).collect::<Vec<_>>();
        let mut expected = None;
        for pair in 0..8 {
            for fast in if pair % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let begin = Instant::now();
                let mut checksum = 0u64;
                for repeat in 0..256 {
                    for query in 0..4096 {
                        let at = (query * 7 + repeat) % ((source.len() - 3) * 8);
                        let n = (query % 16 + 1) as u8;
                        let bits = Bits {
                            source: black_box(&source),
                            at: black_box(at),
                            end: source.len() * 8,
                            reversed: query % 2 != 0,
                            request: &request,
                        };
                        checksum = checksum.wrapping_add(u64::from(
                            black_box(if fast { bits.peek(n) } else { reference(&bits, n) }).unwrap(),
                        ));
                    }
                }
                assert_eq!(*expected.get_or_insert(checksum), checksum);
                eprintln!(
                    "FAX_WINDOW pair={pair} fast={fast} ns={} queries=1048576 checksum={checksum}",
                    begin.elapsed().as_nanos()
                );
            }
        }
    }
    fn dense_strip(request: &DecodeRequest) -> &'static [u8] {
        let bytes = include_bytes!("../../../tests/fixtures/raster/fax4-dense-wl-fill1-opt0-ori1.tiff");
        let d = super::super::Directory::new(bytes).unwrap();
        let offsets = d.values(273, request).unwrap().unwrap();
        let lengths = d.values(279, request).unwrap().unwrap();
        assert_eq!(offsets.2, 1);
        assert_eq!(d.scalar(256, None, request).unwrap(), 1025);
        d.strip(offsets, lengths, 0).unwrap()
    }
    #[test]
    fn reference_cursor_matches_binary_search_for_every_dense_row_and_consumed_bit() {
        let request = DecodeRequest::new("dense-control.tiff");
        let source = dense_strip(&request);
        let mut candidate = Mixed::new_group4(source, 1, 1025, &request);
        let mut reference = Mixed::new_group4(source, 1, 1025, &request);
        for row in 0..129 {
            candidate.advance::<true>().unwrap();
            reference.advance::<false>().unwrap();
            assert_eq!(candidate.transitions(), reference.transitions(), "row={row}");
            assert_eq!(candidate.bits.at, reference.bits.at, "row={row}");
        }
    }
    #[test]
    #[ignore = "paired real compressed-strip row-decoder benchmark; no file-open/FPS claim"]
    fn dense_group4_reference_cursor_alternating_benchmark() {
        use std::{hint::black_box, time::Instant};
        let request = DecodeRequest::new("dense-bench.tiff");
        let source = dense_strip(&request);
        let mut expected = None;
        for pair in 0..8 {
            for cursor in if pair % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let start = Instant::now();
                let mut checksum = 0u64;
                for _ in 0..16 {
                    let mut decoder = Mixed::new_group4(black_box(source), 1, 1025, &request);
                    for _ in 0..129 {
                        if cursor {
                            decoder.advance::<true>().unwrap();
                        } else {
                            decoder.advance::<false>().unwrap();
                        }
                        checksum = checksum
                            .wrapping_add(decoder.transitions().iter().map(|&v| u64::from(v)).sum::<u64>());
                        black_box(decoder.transitions());
                    }
                }
                assert_eq!(*expected.get_or_insert(checksum), checksum);
                eprintln!(
                    "FAX_CURSOR pair={pair} cursor={cursor} ns={} rows=2064 checksum={checksum}",
                    start.elapsed().as_nanos()
                );
            }
        }
    }
    #[test]
    fn rle_rows_align_ignore_padding_and_refuse_incomplete_or_overwide_runs() {
        let request = DecodeRequest::new("rle-control.tiff");
        // white8 + ignored111; white0 + black8 + ignored11.
        let source = [0x9f, 0x35, 0x17];
        let mut decoder = Mixed::new_rle(&source, 1, 8, &request, false);
        decoder.advance::<true>().unwrap();
        assert_eq!(decoder.transitions(), [8]);
        assert_eq!(decoder.bits.at, 8);
        decoder.advance::<true>().unwrap();
        assert_eq!(decoder.transitions(), [0, 8]);
        assert_eq!(decoder.bits.at, 24);
        for source in [&[0x35][..], &[0xa0][..], &[0][..]] {
            assert!(
                Mixed::new_rle(source, 1, 8, &request, false)
                    .advance::<true>()
                    .is_err()
            );
        }
    }
    #[test]
    fn word_aligned_rle_rows_consume_real_padding_and_reset_per_strip() {
        let request = DecodeRequest::new("rlew-control.tiff");
        // white8, eleven ignored padding bits; white0/black8, two ignored bits.
        let source = [0x9f, 0xff, 0x35, 0x17];
        for fill in [1, 2] {
            let encoded: Vec<u8> = source
                .iter()
                .map(|&b: &u8| if fill == 2 { b.reverse_bits() } else { b })
                .collect();
            let mut decoder = Mixed::new_rle(&encoded, fill, 8, &request, true);
            decoder.advance::<true>().unwrap();
            assert_eq!(decoder.transitions(), [8]);
            assert_eq!(decoder.bits.at, 16);
            decoder.advance::<true>().unwrap();
            assert_eq!(decoder.transitions(), [0, 8]);
            assert_eq!(decoder.bits.at, 32);
        }
        assert!(
            Mixed::new_rle(&source[..1], 1, 8, &request, true)
                .advance::<true>()
                .is_err()
        );
        let mut decoder = Mixed::new_rle(&source[2..], 1, 8, &request, true);
        decoder.advance::<true>().unwrap();
        assert_eq!(decoder.transitions(), [0, 8]);
        assert_eq!(decoder.bits.at, 16);
    }
    fn packed(text: &str) -> Vec<u8> {
        let mut data = vec![0; text.len().div_ceil(8)];
        for (i, b) in text.bytes().enumerate() {
            if b == b'1' {
                data[i / 8] |= 1 << (7 - i % 8);
            }
        }
        data
    }
    #[test]
    fn mixed_rows_require_full_eol_first_1d_exact_runs_and_real_consumption() {
        let request = DecodeRequest::new("mixed-control.tiff");
        let source = packed("000000000001100001100000000000101"); // white13 then 2D vertical0
        let mut decoder = Mixed::new(&source, 1, 13, &request, true);
        decoder.advance::<true>().unwrap();
        assert_eq!(decoder.transitions(), [13]);
        decoder.advance::<true>().unwrap();
        assert_eq!(decoder.transitions(), [13]);
        for text in [
            "00000000000101",     // first row cannot be 2D
            "0000000011000011",   // short EOL
            "000000000001110011", // white8 cannot complete width13
            "0000000000011101011",
        ] {
            // white17 exceeds width13
            let source = packed(text);
            let mut decoder = Mixed::new(&source, 1, 13, &request, true);
            assert!(decoder.advance::<true>().is_err(), "{text}");
        }
        let source = [0b10000000];
        let mut bits = Bits {
            source: &source,
            at: 7,
            end: 8,
            reversed: false,
            request: &request,
        };
        assert_eq!(bits.peek(8), Some(0));
        assert!(bits.consume(8).is_err());
        assert_eq!(bits.at, 7);
        assert!(bits.consume(1).is_ok());
        assert!(bits.peek(1).is_none());
    }
}
