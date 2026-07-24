//! Micro-benchmarks for the DNG decoder hot paths:
//! lossless Huffman JPEG (Compression = 7) and MSB-first packed uncompressed
//! rows. Streams are synthetic but structurally identical to the unit-test
//! fixtures in `src/dng/lossless_jpeg.rs`.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

const PRECISION: u8 = 12;

type DecodeFn = fn(&[u8]) -> Result<(u16, u16, usize, u64), String>;
type UnpackFn = fn(&[u8], &mut [u16], u8) -> Result<(), String>;

/// Simple xorshift64* generator for deterministic synthetic content.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

struct BitWriter {
    bytes: Vec<u8>,
    current: u8,
    used: u8,
}

impl BitWriter {
    const fn new() -> Self {
        Self {
            bytes: Vec::new(),
            current: 0,
            used: 0,
        }
    }

    fn write(&mut self, value: u32, bits: u8) {
        for shift in (0..bits).rev() {
            self.current = (self.current << 1) | u8::try_from((value >> shift) & 1).unwrap();
            self.used += 1;
            if self.used == 8 {
                self.flush_byte();
            }
        }
    }

    fn pad_ones(&mut self) {
        while self.used != 0 {
            self.current = (self.current << 1) | 1;
            self.used += 1;
            if self.used == 8 {
                self.flush_byte();
            }
        }
    }

    fn flush_byte(&mut self) {
        self.bytes.push(self.current);
        if self.current == 0xff {
            self.bytes.push(0);
        }
        self.current = 0;
        self.used = 0;
    }
}

fn marker(output: &mut Vec<u8>, code: u8) {
    output.extend_from_slice(&[0xff, code]);
}

fn segment(output: &mut Vec<u8>, code: u8, payload: &[u8]) {
    marker(output, code);
    let length = u16::try_from(payload.len() + 2).unwrap();
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(payload);
}

fn category_and_bits(difference: i32) -> (u8, u32) {
    if difference == 0 {
        return (0, 0);
    }
    if difference == -32_768 || difference == 32_768 {
        return (16, 0);
    }
    let magnitude = difference.unsigned_abs();
    let category = u8::try_from(32 - magnitude.leading_zeros()).unwrap();
    if difference > 0 {
        (category, magnitude)
    } else {
        let mask = (1_u32 << category) - 1;
        (
            category,
            u32::try_from(difference + i32::try_from(mask).unwrap()).unwrap(),
        )
    }
}

fn signed_modulo_difference(sample: i32, predictor: i32) -> i32 {
    let modulo = (sample - predictor) & 0xffff;
    if modulo > 32_767 { modulo - 65_536 } else { modulo }
}

fn select_predictor(selection: u8, left: i32, above: i32, upper_left: i32) -> i32 {
    match selection {
        1 => left,
        2 => above,
        3 => upper_left,
        4 => left + above - upper_left,
        5 => left + ((above - upper_left) >> 1),
        6 => above + ((left - upper_left) >> 1),
        7 => (left + above) >> 1,
        _ => unreachable!(),
    }
}

/// Builds a single-component lossless-JPEG stream identical in structure to
/// the `build_fixture` helper in the decoder's unit tests.
fn build_lossless_jpeg(width: u16, height: u16, predictor: u8, samples: &[u16]) -> Vec<u8> {
    let mut output = Vec::new();
    marker(&mut output, 0xd8); // SOI

    let mut dht = vec![0_u8];
    dht.extend_from_slice(&[0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    dht.extend(0_u8..=16);
    segment(&mut output, 0xc4, &dht); // DHT

    let mut frame = vec![PRECISION];
    frame.extend_from_slice(&height.to_be_bytes());
    frame.extend_from_slice(&width.to_be_bytes());
    frame.push(1);
    frame.extend_from_slice(&[1, 0x11, 0]);
    segment(&mut output, 0xc3, &frame); // SOF3

    segment(&mut output, 0xda, &[1, 1, 0, predictor, 0, 0]); // SOS

    let width_usize = usize::from(width);
    let mcu_count = width_usize * usize::from(height);
    let initial = 1_i32 << (PRECISION - 1);
    let mut bits = BitWriter::new();
    for mcu in 0..mcu_count {
        let x = mcu % width_usize;
        let y = mcu / width_usize;
        let sample = i32::from(samples[mcu]);
        let predicted = if mcu == 0 {
            initial
        } else if y == 0 {
            i32::from(samples[mcu - 1])
        } else if x == 0 {
            i32::from(samples[mcu - width_usize])
        } else {
            let left = i32::from(samples[mcu - 1]);
            let above = i32::from(samples[mcu - width_usize]);
            let upper_left = i32::from(samples[mcu - width_usize - 1]);
            select_predictor(predictor, left, above, upper_left)
        };
        let difference = signed_modulo_difference(sample, predicted);
        let (category, encoded) = category_and_bits(difference);
        bits.write(u32::from(category), 5);
        if category < 16 {
            bits.write(encoded, category);
        }
    }
    bits.pad_ones();
    output.extend_from_slice(&bits.bytes);
    marker(&mut output, 0xd9); // EOI
    output
}

/// Photo-like content: smooth gradient plus small noise, so Huffman
/// differences are short and the entropy stream has few `0xFF` bytes.
fn gradient_samples(width: usize, height: usize) -> Vec<u16> {
    let mut rng = Rng(0x1234_5678_9abc_def0);
    let mut samples = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let base = 2048 + i32::try_from((x * 3 + y * 5) % 1024).unwrap();
            let noise = (rng.next() & 0x3f) as i32 - 32;
            samples.push(u16::try_from((base + noise).clamp(0, 4095)).unwrap());
        }
    }
    samples
}

/// High-entropy content: full-range pseudo-random samples, forcing long
/// Huffman differences and frequent `0xFF` byte stuffing.
fn random_samples(width: usize, height: usize) -> Vec<u16> {
    let mut rng = Rng(0xdead_beef_cafe_f00d);
    (0..width * height)
        .map(|_| u16::try_from(rng.next() & 0xfff).unwrap())
        .collect()
}

/// MSB-first packed rows (DNG `Compression = 1`, 9..=15 bits per sample).
fn pack_msb_rows(samples: &[u16], width: usize, bits_per_sample: u8) -> Vec<u8> {
    let row_bits = width * usize::from(bits_per_sample);
    let row_bytes = row_bits.div_ceil(8);
    let mut encoded = vec![0_u8; row_bytes * (samples.len() / width)];
    for (row, row_samples) in samples.chunks_exact(width).enumerate() {
        let mut bit_position = row * row_bytes * 8;
        for &sample in row_samples {
            for shift in (0..bits_per_sample).rev() {
                let bit = (sample >> shift) & 1;
                if bit == 1 {
                    encoded[bit_position / 8] |= 1 << (7 - (bit_position % 8));
                }
                bit_position += 1;
            }
        }
    }
    encoded
}

fn bench_lossless_jpeg(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("lossless_jpeg");

    let (width, height) = (4000_u16, 3000_u16);
    let samples = gradient_samples(usize::from(width), usize::from(height));
    let stream = build_lossless_jpeg(width, height, 1, &samples);
    let (_, _, _, expected_checksum) =
        rrrah_decode::dng::bench_support::decode_lossless_jpeg(&stream).unwrap();
    let variants: [(&str, DecodeFn); 2] = [
        ("word", rrrah_decode::dng::bench_support::decode_lossless_jpeg),
        (
            "bytewise",
            rrrah_decode::dng::bench_support::decode_lossless_jpeg_bytewise,
        ),
    ];
    for (variant, decode) in variants {
        group.bench_function(
            BenchmarkId::new(format!("12bit_gradient/{variant}"), "4000x3000"),
            |bencher| {
                bencher.iter(|| {
                    let decoded = decode(criterion::black_box(&stream)).unwrap();
                    assert_eq!(decoded.3, expected_checksum);
                    criterion::black_box(decoded);
                });
            },
        );
    }

    let (width, height) = (1024_u16, 1024_u16);
    let samples = random_samples(usize::from(width), usize::from(height));
    let stream = build_lossless_jpeg(width, height, 1, &samples);
    let stuffed = stream.windows(2).filter(|pair| *pair == [0xff, 0x00]).count();
    assert!(stuffed > 100, "high-entropy stream should exercise stuffing");
    let (_, _, _, expected_checksum) =
        rrrah_decode::dng::bench_support::decode_lossless_jpeg(&stream).unwrap();
    let variants: [(&str, DecodeFn); 2] = [
        ("word", rrrah_decode::dng::bench_support::decode_lossless_jpeg),
        (
            "bytewise",
            rrrah_decode::dng::bench_support::decode_lossless_jpeg_bytewise,
        ),
    ];
    for (variant, decode) in variants {
        group.bench_function(
            BenchmarkId::new(format!("12bit_random_stuffed/{variant}"), "1024x1024"),
            |bencher| {
                bencher.iter(|| {
                    let decoded = decode(criterion::black_box(&stream)).unwrap();
                    assert_eq!(decoded.3, expected_checksum);
                    criterion::black_box(decoded);
                });
            },
        );
    }

    group.finish();
}

fn bench_unpack_msb(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("unpack_msb");
    let (width, height) = (6000_usize, 4000_usize);

    for bits_per_sample in [10_u8, 12, 14] {
        let mask = (1_u16 << bits_per_sample) - 1;
        let samples: Vec<u16> = {
            let mut rng = Rng(0x0bad_f00d_1357_9bdf);
            gradient_samples(width, height)
                .into_iter()
                .map(|sample| sample.wrapping_add(u16::try_from(rng.next() & 0xff).unwrap()) & mask)
                .collect()
        };
        let encoded = pack_msb_rows(&samples, width, bits_per_sample);
        let row_bits = width * usize::from(bits_per_sample);
        let row_bytes = row_bits.div_ceil(8);
        let mut output = vec![0_u16; width];
        let variants: [(&str, UnpackFn); 2] = [
            ("word", rrrah_decode::dng::bench_support::unpack_msb_row),
            (
                "bytewise",
                rrrah_decode::dng::bench_support::unpack_msb_row_bytewise,
            ),
        ];
        for (variant, unpack) in variants {
            group.bench_function(
                BenchmarkId::new(format!("{bits_per_sample}bit_rows/{variant}"), "6000x4000"),
                |bencher| {
                    bencher.iter(|| {
                        for row in 0..height {
                            let start = row * row_bytes;
                            unpack(
                                criterion::black_box(&encoded[start..start + row_bytes]),
                                &mut output,
                                bits_per_sample,
                            )
                            .unwrap();
                        }
                        criterion::black_box(&output);
                    });
                },
            );
        }
    }

    group.finish();
}

/// End-to-end tiled-DNG decode (parse + segment decode) across segment
/// worker counts. The 6000x4000 12-bit frame is stored as 24x16 = 384
/// independent 256x256 lossless-JPEG tiles; output must be bit-identical
/// for every worker count.
fn bench_dng_tiled_parallel(criterion: &mut Criterion) {
    use rrrah_decode::dng::bench_support::{
        SegmentLayout, SyntheticCompression, build_segmented_dng, decode_dng_pixels_with_workers,
    };

    let (width, height) = (6000_u32, 4000_u32);
    let samples = gradient_samples(width as usize, height as usize);
    let dng = build_segmented_dng(
        width,
        height,
        SegmentLayout::Tiles {
            tile_width: 256,
            tile_height: 256,
        },
        SyntheticCompression::LosslessJpeg12,
        &samples,
    );
    let reference = decode_dng_pixels_with_workers(&dng, 1).unwrap();

    let mut group = criterion.benchmark_group("dng_tiled_parallel");
    group.sample_size(10);
    for workers in [1_usize, 2, 4, 8] {
        group.bench_function(
            BenchmarkId::new("12bit_gradient_256px_tiles", format!("{workers}workers")),
            |bencher| {
                bencher.iter(|| {
                    let checksum =
                        decode_dng_pixels_with_workers(criterion::black_box(&dng), workers).unwrap();
                    assert_eq!(checksum, reference);
                    criterion::black_box(checksum);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_lossless_jpeg, bench_unpack_msb, bench_dng_tiled_parallel);
criterion_main!(benches);
