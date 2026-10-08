//! Original image samples after stream decompression, before color conversion.
//! Stream/filter allocation and image masks are separate from this admission.
type Credit = Box<dyn std::any::Any + Send + Sync>;
/// Admitted original component buffer; its credit outlives the sample storage.
pub struct NativeComponentSamples {
    /// Interleaved row-major samples in original color coordinates.
    values: Vec<f32>,
    _credit: Credit,
}
impl NativeComponentSamples {
    pub(crate) fn clamp_alpha(&mut self, cancelled: &dyn Fn() -> bool) -> Option<()> {
        for value in &mut self.values { if cancelled() { return None; } *value=value.clamp(0.,1.); }
        Some(())
    }
    /// Borrow the admitted interleaved row-major component samples.
    pub fn samples(&self) -> &[f32] {
        &self.values
    }
}
/// Decode MSB-first, byte-aligned rows into original component coordinates.
/// `/Decode` endpoints may be reversed or outside [0,1] (e.g. indexed/Lab).
pub fn unpack(
    data: &[u8],
    width: u32,
    height: u32,
    components: u8,
    bits: u8,
    decode: &[(f32, f32)],
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Credit>,
) -> Option<NativeComponentSamples> {
    if cancelled()
        || width == 0
        || height == 0
        || components == 0
        || !(1..=16).contains(&bits)
        || decode.len() != usize::from(components)
        || decode.iter().any(|(a, b)| !a.is_finite() || !b.is_finite())
    {
        return None;
    }
    let row_samples = usize::try_from(width)
        .ok()?
        .checked_mul(usize::from(components))?;
    let row_bytes = row_samples.checked_mul(usize::from(bits))?.checked_add(7)? / 8;
    let count = row_samples.checked_mul(usize::try_from(height).ok()?)?;
    if data.len() != row_bytes.checked_mul(usize::try_from(height).ok()?)? {
        return None;
    }
    let bytes = count
        .checked_mul(size_of::<f32>())?
        .checked_add(size_of::<NativeComponentSamples>())?;
    let credit = admit(bytes)?;
    if cancelled() {
        return None;
    }
    let mut values = Vec::new();
    values.try_reserve_exact(count).ok()?;
    let maximum = ((1u32 << bits) - 1) as f64;
    for row in data.chunks_exact(row_bytes) {
        let mut bit_offset = 0;
        for index in 0..row_samples {
            if cancelled() {
                return None;
            }
            let mut sample = 0u32;
            for _ in 0..bits {
                sample = (sample << 1) | u32::from((row[bit_offset / 8] >> (7 - bit_offset % 8)) & 1);
                bit_offset += 1;
            }
            let (a, b) = decode[index % usize::from(components)];
            let value = (f64::from(a) + f64::from(sample) / maximum * (f64::from(b) - f64::from(a))) as f32;
            if !value.is_finite() {
                return None;
            }
            values.push(value);
        }
    }
    if cancelled() {
        return None;
    }
    Some(NativeComponentSamples {
        values,
        _credit: credit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Guard(Arc<AtomicUsize>, usize);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    #[test]
    fn native_component_samples_precision_padding_and_decode_ranges() {
        let admit = |_| Some(Box::new(()) as Credit);
        let out = unpack(
            &[0x12, 0x34, 0xAB, 0xCD],
            1,
            1,
            2,
            16,
            &[(0., 1.), (1., 0.)],
            &|| false,
            &admit,
        )
        .unwrap();
        assert_eq!(out.values, vec![4660. / 65535., 21554. / 65535.]);
        let cmyk = unpack(&[17, 95, 203, 41], 1, 1, 4, 8, &[(0., 1.); 4], &|| false, &admit).unwrap();
        assert_eq!(cmyk.values, vec![17. / 255., 95. / 255., 203. / 255., 41. / 255.]);
        // Low row-padding bits must not become the next row's first sample.
        let out = unpack(
            &[0b10111111, 0b01011111],
            3,
            2,
            1,
            1,
            &[(0., 1.)],
            &|| false,
            &admit,
        )
        .unwrap();
        assert_eq!(out.values, vec![1., 0., 1., 0., 1., 0.]);
        let out = unpack(&[0xF0], 1, 1, 1, 4, &[(-128., 127.)], &|| false, &admit).unwrap();
        assert_eq!(out.values, vec![127.]);
    }
    #[test]
    fn native_component_samples_refusals_and_every_cancellation_release_credit() {
        let used = Arc::new(AtomicUsize::new(0));
        let admit = |bytes| {
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Guard(used.clone(), bytes)) as Credit)
        };
        let polls = Cell::new(0);
        let source = [0x12, 0x34, 0xAB, 0xCD];
        let decode = [(0., 1.), (1., 0.)];
        let out = unpack(
            &source,
            1,
            1,
            2,
            16,
            &decode,
            &|| {
                polls.set(polls.get() + 1);
                false
            },
            &admit,
        )
        .unwrap();
        assert!(used.load(Ordering::Relaxed) > 0);
        drop(out);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        for stop in 1..=polls.get() {
            let now = Cell::new(0);
            assert!(
                unpack(
                    &source,
                    1,
                    1,
                    2,
                    16,
                    &decode,
                    &|| {
                        now.set(now.get() + 1);
                        now.get() == stop
                    },
                    &admit
                )
                .is_none()
            );
            assert_eq!(used.load(Ordering::Relaxed), 0);
        }
        assert!(unpack(&source, 1, 1, 2, 16, &decode, &|| false, &|_| None).is_none());
        assert!(unpack(&source[..3], 1, 1, 2, 16, &decode, &|| false, &admit).is_none());
        assert!(unpack(&source, 1, 1, 2, 16, &[(0., f32::NAN); 2], &|| false, &admit).is_none());
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
}
