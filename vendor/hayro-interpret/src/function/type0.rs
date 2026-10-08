use crate::function::{Clamper, TupleVec, Values, interpolate};
use hayro_syntax::bit_reader::BitReader;
use hayro_syntax::object::Array;
use hayro_syntax::object::Stream;
use hayro_syntax::object::dict::keys::{BITS_PER_SAMPLE, DECODE, ENCODE, SIZE};
use smallvec::{SmallVec, ToSmallVec, smallvec};
use std::collections::HashMap;

/// A type 0 function (sampled function).
#[derive(Debug)]
pub(crate) struct Type0 {
    sizes: IntVec,
    table: HashMap<Key, IntVec>,
    clamper: Clamper,
    range: TupleVec,
    bits_per_sample: u8,
    encode: TupleVec,
    decode: TupleVec,
    order: u8,
}

impl Type0 {
    pub(crate) fn eval_scalar(&self, input: f32, cancelled: &dyn Fn() -> bool,
        remaining: &mut usize) -> Option<f32> {
        if !input.is_finite() || self.order != 1 || self.sizes.len() != 1
            || self.sizes[0] == 0 || self.clamper.domain.len() != 1
            || self.range.len() != 1 || self.encode.len() != 1 || self.decode.len() != 1
            || !matches!(self.bits_per_sample, 1 | 2 | 4 | 8 | 16 | 24 | 32)
        { return None; }
        let (low, high) = self.clamper.domain[0];
        let (e0, e1) = self.encode[0];
        let (d0, d1) = self.decode[0];
        let (r0, r1) = self.range[0];
        if [low,high,e0,e1,d0,d1,r0,r1].iter().any(|v| !v.is_finite())
            || low >= high || r0 > r1
        { return None; }
        let input = f64::from(input.clamp(low, high));
        let position = (f64::from(e0) + (input-f64::from(low)) / (f64::from(high)-f64::from(low))
            * (f64::from(e1)-f64::from(e0))).clamp(0.0, f64::from(self.sizes[0]-1));
        let left = position.floor() as u32;
        let right = position.ceil() as u32;
        let sample = |index, remaining: &mut usize| {
            if cancelled() { return None; }
            *remaining = remaining.checked_sub(1)?;
            let value = self.table.get(&Key::from_raw(&self.sizes, &[index]))?;
            if value.len() != 1 { return None; }
            Some(f64::from(value[0]))
        };
        let a = sample(left, remaining)?;
        let b = if left == right { a } else { sample(right, remaining)? };
        let raw = a + (position-f64::from(left)) * (b-a);
        let maximum = ((1_u64 << self.bits_per_sample) - 1) as f64;
        let decoded = f64::from(d0) + raw/maximum * (f64::from(d1)-f64::from(d0));
        if !decoded.is_finite() { return None; }
        Some(decoded.clamp(f64::from(r0),f64::from(r1)) as f32)
    }
    /// Create a new type 0 function.
    pub(crate) fn new(stream: &Stream<'_>) -> Option<Self> {
        let dict = stream.dict();
        let bits_per_sample = dict.get::<u8>(BITS_PER_SAMPLE)?;

        if !matches!(bits_per_sample, 1 | 2 | 4 | 8 | 16 | 24 | 32) {
            error!("invalid bits per sample: {bits_per_sample}");

            return None;
        }

        let clamper = Clamper::new(dict)?;
        let range = clamper.range.clone()?;

        if range.is_empty() {
            warn!("encountered Type0 function with invalid range length 0.");

            return None;
        }

        let sizes = dict
            .get::<Array<'_>>(SIZE)?
            .iter::<u32>()
            .collect::<IntVec>();

        if sizes.is_empty() || sizes.iter().any(|size| *size == 0) { return None; }

        let encode = dict
            .get::<TupleVec>(ENCODE)
            .unwrap_or(sizes.iter().map(|s| (0.0, (*s - 1) as f32)).collect());

        let decode = dict.get::<TupleVec>(DECODE).unwrap_or(range.clone());

        let mut data = {
            let decoded = stream.decoded().ok()?;
            let mut buf = vec![];
            let mut reader = BitReader::new(&decoded);

            while let Some(data) = reader.read(bits_per_sample) {
                buf.push(data);
            }

            buf
        };

        let num_expected_entries = sizes.iter().try_fold(1_usize, |product, size| product.checked_mul(*size as usize))?
            .checked_mul(range.len())?;

        if data.len() != num_expected_entries {
            warn!("Type0 function didn't have the expected number of sample entries.");
            data.truncate(num_expected_entries);
        }

        let table = build_table(&data, &sizes, range.len())?;

        Some(Self {
            sizes,
            clamper,
            range,
            bits_per_sample,
            table,
            encode,
            decode,
            order: dict.get::<u8>(b"Order".as_slice()).unwrap_or(1),
        })
    }

    /// Evaluate a type 0 function with the given input.
    pub(crate) fn eval(&self, mut input: Values) -> Option<Values> {
        if input.len() != self.sizes.len() {
            warn!("wrong number of arguments for sampled function");

            return None;
        }

        self.clamper.clamp_input(&mut input);

        let mut key = input;

        for (((x, domain), encode), size) in key
            .iter_mut()
            .zip(self.clamper.domain.iter())
            .zip(self.encode.iter())
            .zip(self.sizes.iter())
        {
            *x = interpolate(*x, domain.0, domain.1, encode.0, encode.1);
            *x = x.max(0.0).min(*size as f32 - 1.0);
        }

        let in_prev = key.iter().map(|v| v.floor() as u32).collect::<IntVec>();
        let in_next = key.iter().map(|v| v.ceil() as u32).collect::<IntVec>();

        let interpolator = Interpolator::new(
            key.clone().to_smallvec(),
            in_prev,
            in_next,
            self.sizes.clone(),
            self.range.len(),
        );

        let interpolated = interpolator.interpolate(&self.table)?;

        let mut out = interpolated
            .iter()
            .zip(self.decode.iter())
            .map(|(x, decode)| {
                interpolate(
                    *x,
                    0.0,
                    ((1_u64 << self.bits_per_sample) - 1) as f32,
                    decode.0,
                    decode.1,
                )
            })
            .collect::<SmallVec<_>>();

        self.clamper.clamp_output(&mut out);

        Some(out)
    }
}

#[cfg(test)]
mod scalar_tests {
    use super::*;
    use crate::function::{Function, FunctionType};
    use std::sync::Arc;

    fn sampled(bits: u8, encode: (f32,f32), decode: (f32,f32)) -> Function {
        let maximum = ((1_u64 << bits)-1) as u32;
        Function(Arc::new(FunctionType::Type0(Type0 {
            sizes: smallvec![2], table: build_table(&[0,maximum], &[2], 1).unwrap(),
            clamper: Clamper { domain: smallvec![(0.,1.)], range: Some(smallvec![(0.,1.)]) },
            range: smallvec![(0.,1.)], bits_per_sample: bits,
            encode: smallvec![encode], decode: smallvec![decode], order: 1,
        })))
    }

    #[test]
    fn sampled_scalar_linear_references_cover_all_bit_depths() {
        for bits in [1,2,4,8,16,24,32] {
            let function = sampled(bits, (0.,1.), (0.,1.));
            for input in [0.,0.125,0.5,0.875,1.] {
                assert_eq!(function.eval_scalar_bounded(input, &|| false), Some(input));
            }
            assert_eq!(function.eval(smallvec![0.5]).unwrap()[0], 0.5);
        }
        assert_eq!(sampled(8,(1.,0.),(0.,1.)).eval_scalar_bounded(0.25,&|| false),Some(0.75));
        let decoded = sampled(8,(0.,1.),(-1.,2.));
        for (input,expected) in [(0.25,0.),(0.5,0.5),(0.75,1.)] {
            assert_eq!(decoded.eval_scalar_bounded(input,&|| false),Some(expected));
        }
    }

    #[test]
    fn sampled_scalar_refuses_cancellation_missing_samples_and_work_exhaustion() {
        let function = sampled(8,(0.,1.),(0.,1.));
        for stop in 1..=4 {
            let polls = std::cell::Cell::new(0);
            assert!(function.eval_scalar_bounded(0.5,&|| {
                polls.set(polls.get()+1); polls.get()==stop
            }).is_none());
        }
        assert!(function.eval_scalar_with_budget(0.5,&|| false,&mut 2,0).is_none());
        assert!(function.eval_scalar_bounded(f32::NAN,&|| false).is_none());
        let FunctionType::Type0(table) = function.0.as_ref() else { unreachable!() };
        let mut remaining = 100;
        assert!(table.eval_scalar(f32::INFINITY,&|| false,&mut remaining).is_none());
        let mut invalid = Type0 {
            sizes: smallvec![2], table: HashMap::new(), clamper: table.clamper.clone(),
            range: table.range.clone(), bits_per_sample: 8, encode: table.encode.clone(),
            decode: table.decode.clone(), order: 1,
        };
        assert!(invalid.eval_scalar(0.5,&|| false,&mut remaining).is_none());
        invalid.table = table.table.clone();
        invalid.order = 3;
        assert!(invalid.eval_scalar(0.5,&|| false,&mut remaining).is_none());
    }
}

type FloatVec = SmallVec<[f32; 4]>;
type IntVec = SmallVec<[u32; 4]>;

// See <https://github.com/apache/pdfbox/blob/bb778d4784f354c36ce032e91a0cee2169a4c598/pdfbox/src/main/java/org/apache/pdfbox/pdmodel/common/function/PDFunctionType0.java#L252>
struct Interpolator {
    input: FloatVec,
    sizes: IntVec,
    in_prev: IntVec,
    in_next: IntVec,
    out_len: usize,
}

impl Interpolator {
    fn new(
        input: FloatVec,
        in_prev: IntVec,
        in_next: IntVec,
        sizes: IntVec,
        out_len: usize,
    ) -> Self {
        Self {
            input,
            in_prev,
            in_next,
            sizes,
            out_len,
        }
    }

    fn interpolate(&self, table: &HashMap<Key, IntVec>) -> Option<FloatVec> {
        self.interpolate_inner(smallvec![0; self.input.len()], 0, table)
    }

    fn interpolate_inner(
        &self,
        mut coord: IntVec,
        step: usize,
        table: &HashMap<Key, IntVec>,
    ) -> Option<FloatVec> {
        if step == self.input.len() - 1 {
            if self.in_prev[step] == self.in_next[step] {
                coord[step] = self.in_prev[step];

                Some(
                    table
                        .get(&Key::from_raw(&self.sizes, &coord))?
                        .clone()
                        .iter()
                        .map(|n| *n as f32)
                        .collect(),
                )
            } else {
                coord[step] = self.in_prev[step];
                let val1 = table.get(&Key::from_raw(&self.sizes, &coord))?;
                coord[step] = self.in_next[step];
                let val2 = table.get(&Key::from_raw(&self.sizes, &coord))?;
                let mut out = smallvec![0.0; self.out_len];

                for i in 0..self.out_len {
                    out[i] = interpolate(
                        self.input[step],
                        self.in_prev[step] as f32,
                        self.in_next[step] as f32,
                        val1[i] as f32,
                        val2[i] as f32,
                    );
                }

                Some(out)
            }
        } else if self.in_prev[step] == self.in_next[step] {
            coord[step] = self.in_prev[step];
            self.interpolate_inner(coord, step + 1, table)
        } else {
            coord[step] = self.in_prev[step];
            let val1 = self.interpolate_inner(coord.clone(), step + 1, table)?;
            coord[step] = self.in_next[step];
            let val2 = self.interpolate_inner(coord, step + 1, table)?;

            let mut out = smallvec![0.0; self.out_len];

            for i in 0..self.out_len {
                out[i] = interpolate(
                    self.input[step],
                    self.in_prev[step] as f32,
                    self.in_next[step] as f32,
                    val1[i],
                    val2[i],
                );
            }

            Some(out)
        }
    }
}

fn build_table(data: &[u32], sizes: &[u32], n: usize) -> Option<HashMap<Key, IntVec>> {
    let mut key = Key::new(sizes);
    let mut table = HashMap::new();

    let mut first = true;

    for b in data.chunks_exact(n) {
        if !first {
            key.increment();
        }

        table.insert(key.clone(), b.to_smallvec());

        first = false;
    }

    Some(table)
}

/// A sampled function consists of a (possibly) multi-dimensional table that we can index
/// into. We do this by representing the entries as a flat list of vectors, where each
/// element in the vector represents the value of the key in that specific dimension.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    sizes: SmallVec<[u32; 4]>,
    parts: SmallVec<[u32; 4]>,
}

impl Key {
    fn new(sizes: &[u32]) -> Self {
        let parts = smallvec![0; sizes.len()];

        Self {
            sizes: sizes.to_smallvec(),
            parts,
        }
    }

    fn from_raw(sizes: &[u32], parts: &[u32]) -> Self {
        Self {
            sizes: sizes.to_smallvec(),
            parts: parts.to_smallvec(),
        }
    }

    fn increment(&mut self) -> Option<()> {
        self.increment_index(0)
    }

    fn increment_index(&mut self, index: usize) -> Option<()> {
        let size = *self.sizes.get(index).or_else(|| {
            error!("overflowed key in sampled function");

            None
        })?;
        let val = self.parts.get_mut(index)?;

        if *val >= (size - 1) {
            *val = 0;
            self.increment_index(index + 1)?;
        } else {
            *val += 1;
        }

        Some(())
    }
}
