//! Interleaved A/B timing harness: alternates the word-wise refill path with
//! the byte-at-a-time reference path inside one process so external machine
//! load affects both variants equally. Reports min / p50 / p95 per variant
//! and verifies bit-identical output between them.

use std::hint::black_box;
use std::time::{Duration, Instant};

use rrrah_decode::dng::bench_support;

const PRECISION: u8 = 12;

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

fn build_lossless_jpeg(width: u16, height: u16, samples: &[u16]) -> Vec<u8> {
    let mut output = Vec::new();
    marker(&mut output, 0xd8);
    let mut dht = vec![0_u8];
    dht.extend_from_slice(&[0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    dht.extend(0_u8..=16);
    segment(&mut output, 0xc4, &dht);
    let mut frame = vec![PRECISION];
    frame.extend_from_slice(&height.to_be_bytes());
    frame.extend_from_slice(&width.to_be_bytes());
    frame.push(1);
    frame.extend_from_slice(&[1, 0x11, 0]);
    segment(&mut output, 0xc3, &frame);
    segment(&mut output, 0xda, &[1, 1, 0, 1, 0, 0]);

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
        } else if y == 0 || x != 0 {
            i32::from(samples[mcu - 1])
        } else {
            i32::from(samples[mcu - width_usize])
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
    marker(&mut output, 0xd9);
    output
}

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

fn random_samples(width: usize, height: usize) -> Vec<u16> {
    let mut rng = Rng(0xdead_beef_cafe_f00d);
    (0..width * height)
        .map(|_| u16::try_from(rng.next() & 0xfff).unwrap())
        .collect()
}

fn pack_msb_rows(samples: &[u16], width: usize, bits_per_sample: u8) -> Vec<u8> {
    let row_bits = width * usize::from(bits_per_sample);
    let row_bytes = row_bits.div_ceil(8);
    let mut encoded = vec![0_u8; row_bytes * (samples.len() / width)];
    for (row, row_samples) in samples.chunks_exact(width).enumerate() {
        let mut bit_position = row * row_bytes * 8;
        for &sample in row_samples {
            for shift in (0..bits_per_sample).rev() {
                if (sample >> shift) & 1 == 1 {
                    encoded[bit_position / 8] |= 1 << (7 - (bit_position % 8));
                }
                bit_position += 1;
            }
        }
    }
    encoded
}

fn percentile(sorted: &[Duration], percentile_rank: usize) -> Duration {
    let index = (sorted.len() - 1) * percentile_rank / 100;
    sorted[index]
}

trait TapSort {
    fn tap_sort(&mut self) -> Self;
}

impl TapSort for Vec<Duration> {
    fn tap_sort(&mut self) -> Self {
        self.sort_unstable();
        self.clone()
    }
}

fn report(label: &str, mut samples: Vec<Duration>) {
    samples.sort_unstable();
    let ms = |duration: Duration| duration.as_secs_f64() * 1e3;
    println!(
        "  {label}: n={} min={:.3} ms  p50={:.3} ms  p95={:.3} ms",
        samples.len(),
        ms(samples[0]),
        ms(percentile(&samples, 50)),
        ms(percentile(&samples, 95)),
    );
}

fn ab_lossless_jpeg(label: &str, stream: &[u8], rounds: usize) {
    let reference = bench_support::decode_lossless_jpeg_bytewise(stream).unwrap();
    let word = bench_support::decode_lossless_jpeg(stream).unwrap();
    assert_eq!(reference, word, "{label}: word-wise refill changed the output");
    println!("{label} ({} stream bytes, bit-identical: yes)", stream.len());

    let mut reference_times = Vec::with_capacity(rounds);
    let mut word_times = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let started = Instant::now();
        black_box(bench_support::decode_lossless_jpeg_bytewise(black_box(stream)).unwrap());
        reference_times.push(started.elapsed());

        let started = Instant::now();
        black_box(bench_support::decode_lossless_jpeg(black_box(stream)).unwrap());
        word_times.push(started.elapsed());
    }
    let min = |times: &[Duration]| times.iter().min().copied().unwrap();
    let speedup_min = min(&reference_times).as_secs_f64() / min(&word_times).as_secs_f64();
    let speedup_p50 = percentile(&reference_times.clone().tap_sort(), 50).as_secs_f64()
        / percentile(&word_times.clone().tap_sort(), 50).as_secs_f64();
    report("byte-wise reference", reference_times);
    report("word-wise refill   ", word_times);
    println!("  speedup: min-based {speedup_min:.3}x, p50-based {speedup_p50:.3}x");
}

fn ab_unpack(label: &str, encoded: &[u8], width: usize, height: usize, bits: u8, rounds: usize) {
    let row_bytes = (width * usize::from(bits)).div_ceil(8);
    let mut reference_out = vec![0_u16; width * height];
    let mut word_out = vec![0_u16; width * height];
    for row in 0..height {
        let start = row * row_bytes;
        bench_support::unpack_msb_row_bytewise(
            &encoded[start..start + row_bytes],
            &mut reference_out[row * width..(row + 1) * width],
            bits,
        )
        .unwrap();
        bench_support::unpack_msb_row(
            &encoded[start..start + row_bytes],
            &mut word_out[row * width..(row + 1) * width],
            bits,
        )
        .unwrap();
    }
    assert_eq!(reference_out, word_out, "{label}: word refill changed output");
    println!("{label} ({row_bytes} bytes/row, bit-identical: yes)");

    let mut reference_times = Vec::with_capacity(rounds);
    let mut word_times = Vec::with_capacity(rounds);
    let mut output = vec![0_u16; width];
    for _ in 0..rounds {
        let started = Instant::now();
        for row in 0..height {
            let start = row * row_bytes;
            bench_support::unpack_msb_row_bytewise(
                black_box(&encoded[start..start + row_bytes]),
                &mut output,
                bits,
            )
            .unwrap();
        }
        black_box(&output);
        reference_times.push(started.elapsed());

        let started = Instant::now();
        for row in 0..height {
            let start = row * row_bytes;
            bench_support::unpack_msb_row(black_box(&encoded[start..start + row_bytes]), &mut output, bits)
                .unwrap();
        }
        black_box(&output);
        word_times.push(started.elapsed());
    }
    let min = |times: &[Duration]| times.iter().min().copied().unwrap();
    let speedup_min = min(&reference_times).as_secs_f64() / min(&word_times).as_secs_f64();
    let speedup_p50 = percentile(&reference_times.clone().tap_sort(), 50).as_secs_f64()
        / percentile(&word_times.clone().tap_sort(), 50).as_secs_f64();
    report("byte-wise reference", reference_times);
    report("word-wise refill   ", word_times);
    println!("  speedup: min-based {speedup_min:.3}x, p50-based {speedup_p50:.3}x");
}

fn main() {
    let rounds = 30;

    let samples = gradient_samples(4000, 3000);
    let stream = build_lossless_jpeg(4000, 3000, &samples);
    ab_lossless_jpeg("lossless_jpeg 12bit gradient 4000x3000", &stream, rounds);

    let samples = random_samples(1024, 1024);
    let stream = build_lossless_jpeg(1024, 1024, &samples);
    let stuffed = stream.windows(2).filter(|pair| *pair == [0xff, 0x00]).count();
    println!("high-entropy stream: {stuffed} stuffed 0xFF00 pairs");
    ab_lossless_jpeg("lossless_jpeg 12bit random 1024x1024", &stream, rounds);

    let samples = gradient_samples(6000, 4000);
    for bits in [10_u8, 12, 14] {
        let mask = (1_u16 << bits) - 1;
        let masked: Vec<u16> = samples.iter().map(|sample| sample & mask).collect();
        let encoded = pack_msb_rows(&masked, 6000, bits);
        ab_unpack(
            &format!("unpack_msb {bits}bit 6000x4000"),
            &encoded,
            6000,
            4000,
            bits,
            rounds,
        );
    }
}
