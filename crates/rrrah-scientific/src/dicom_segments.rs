//! Segmented16 LUT expansion: bounded, no recursive indirect references.
use super::{DecodeRequest, RasterDecodeError, bad, rgb, word};
use rrrah_core::PixelBuffer;
#[derive(Clone, Copy)]
struct Segment {
    op: u16,
    n: usize,
    value: usize,
    end: usize,
}
struct Stream<'a> {
    bytes: &'a [u8],
    little: bool,
    visits: usize,
    request: &'a DecodeRequest,
}
impl Stream<'_> {
    fn value(&self, at: usize) -> Result<u16, RasterDecodeError> {
        Ok(word(
            self.bytes
                .get(at..at + 2)
                .ok_or_else(|| bad("truncated palette segment"))?
                .try_into()
                .unwrap(),
            self.little,
        ))
    }
    fn segment(&mut self, at: usize) -> Result<Segment, RasterDecodeError> {
        self.visits += 1;
        if self.visits > 1_000_000 {
            return Err(bad("palette segment work limit"));
        }
        if self.visits % 256 == 0 {
            self.request.check_cancelled()?;
        }
        if at % 2 != 0 {
            return Err(bad("unaligned palette segment"));
        }
        let op = self.value(at)?;
        let n = usize::from(self.value(at + 2)?);
        let value = at + 4;
        let size = match op {
            0 => n * 2,
            1 if n > 0 => 2,
            2 if n > 0 => 4,
            _ => return Err(bad("invalid palette segment opcode/length")),
        };
        let end = value
            .checked_add(size)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| bad("palette segment value boundary"))?;
        Ok(Segment { op, n, value, end })
    }
    fn boundary(&self, target: usize, starts: &[u8]) -> Result<(), RasterDecodeError> {
        if target % 2 == 0
            && target < self.bytes.len()
            && starts
                .get(target / 16)
                .is_some_and(|v| v & (1 << (target % 16 / 2)) != 0)
        {
            Ok(())
        } else {
            Err(bad("indirect palette offset is not a segment boundary"))
        }
    }
    fn walk(
        &mut self,
        starts: &[u8],
        entries: usize,
        mut emit: impl FnMut(&Self, Segment) -> Result<(), RasterDecodeError>,
    ) -> Result<usize, RasterDecodeError> {
        let mut at = 0;
        let mut produced = 0usize;
        while at < self.bytes.len() {
            let outer = self.segment(at)?;
            let (mut target, copies) = if outer.op == 2 {
                if produced == 0 {
                    return Err(bad("first palette segment cannot be indirect"));
                }
                let target = usize::try_from(
                    u32::from(self.value(outer.value)?) | (u32::from(self.value(outer.value + 2)?) << 16),
                )
                .map_err(|_| bad("palette offset overflow"))?;
                self.boundary(target, starts)?;
                (target, outer.n)
            } else {
                (at, 1)
            };
            for _ in 0..copies {
                let simple = if outer.op == 2 {
                    self.segment(target)?
                } else {
                    outer
                };
                if simple.op == 2 {
                    return Err(bad("indirect palette cannot copy indirect segment"));
                }
                if simple.op == 1 && produced == 0 {
                    return Err(bad("first palette segment cannot be linear"));
                }
                let next = produced
                    .checked_add(simple.n)
                    .filter(|n| *n <= entries)
                    .ok_or_else(|| bad("palette segment exceeds descriptor count"))?;
                emit(self, simple)?;
                produced = next;
                target = simple.end;
            }
            at = outer.end;
        }
        Ok(produced)
    }
    fn emit(&self, s: Segment, out: &mut [u8], length: &mut usize) -> Result<(), RasterDecodeError> {
        if s.op == 2 {
            return Err(bad("indirect palette cannot copy indirect segment"));
        }
        if length.checked_add(s.n).is_none_or(|n| n > out.len() / 2) {
            return Err(bad("palette segment exceeds descriptor count"));
        }
        let start = if s.op == 1 {
            if *length == 0 {
                return Err(bad("first palette segment cannot be linear"));
            }
            u64::from(word(
                out[(*length - 1) * 2..*length * 2].try_into().unwrap(),
                self.little,
            ))
        } else {
            0
        };
        let last = if s.op == 1 {
            u64::from(self.value(s.value)?)
        } else {
            0
        };
        for i in 0..s.n {
            if i % 4096 == 0 {
                self.request.check_cancelled()?;
            }
            let v = if s.op == 0 {
                self.value(s.value + i * 2)?
            } else {
                let n = s.n as u64;
                let step = i as u64 + 1;
                let numerator = (n - step) * start + step * last;
                let q = numerator / n;
                let rem = numerator % n;
                (q + u64::from(rem * 2 > n || (rem * 2 == n && q % 2 != 0))) as u16
            };
            out[*length * 2..*length * 2 + 2].copy_from_slice(&if self.little {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            });
            *length += 1;
        }
        Ok(())
    }
}
pub(super) fn expand(
    bytes: &[u8],
    entries: usize,
    little: bool,
    request: &DecodeRequest,
) -> Result<PixelBuffer<u8>, RasterDecodeError> {
    request.check_cancelled()?;
    if bytes.is_empty() {
        return Err(bad("empty segmented palette"));
    }
    if entries == 0 || entries > 65536 {
        return Err(bad("segmented palette entry limit"));
    }
    let mut stream = Stream {
        bytes,
        little,
        visits: 0,
        request,
    };
    let mut at = 0;
    let mut starts = rgb::buffer(bytes.len().div_ceil(16), 0u8, request)?;
    while at < bytes.len() {
        let segment = stream.segment(at)?;
        starts.get_mut().unwrap()[at / 16] |= 1 << (at % 16 / 2);
        at = segment.end;
    }
    let planned = stream.walk(starts.as_slice(), entries, |_, _| Ok(()))?;
    if planned != entries {
        return Err(bad("segmented palette descriptor count mismatch"));
    }
    let mut output = rgb::buffer(entries * 2, 0u8, request)?;
    let mut written = 0;
    let actual = stream.walk(starts.as_slice(), entries, |stream, segment| {
        stream.emit(segment, output.get_mut().unwrap(), &mut written)
    })?;
    if actual != entries || written != entries {
        return Err(bad("segmented palette descriptor count mismatch"));
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(words: &[u16], little: bool) -> Vec<u8> {
        words
            .iter()
            .flat_map(|w| if little { w.to_le_bytes() } else { w.to_be_bytes() })
            .collect()
    }
    #[test]
    fn exact_linear_zero_endpoint_byte_offsets_and_indirect_forbidden() {
        for little in [false, true] {
            let request = DecodeRequest::new("seg.dcm");
            // Start0, interpolate0->5 in2steps: nearest-even2,5. Copy that
            // linear segment at byteoffset6, this time from5 ->5.
            let data = bytes(&[0, 1, 0, 1, 2, 5, 2, 1, 6, 0], little);
            let out = expand(&data, 5, little, &request).unwrap();
            let actual: Vec<_> = out
                .chunks_exact(2)
                .map(|v| word(v.try_into().unwrap(), little))
                .collect();
            assert_eq!(actual, [0, 2, 5, 5, 5]);
            for words in [
                &[1, 1, 0][..],
                &[2, 1, 0, 0],
                &[0, 1, 0, 2, 1, 8, 0],
                &[0, 1, 0, 2, 1, 2, 0],
                &[0, 1, 0, 2, 1, 6, 0],
                &[3, 1, 0],
                &[0, 1, 0, 1, 0, 3],
            ] {
                let budget = rrrah_core::MemoryBudget::new(100);
                let mut request = request.clone();
                request.memory_budget = Some(budget.clone());
                assert!(expand(&bytes(words, little), 5, little, &request).is_err());
                assert_eq!(budget.used(), 0);
            }
            for end in 0..data.len() {
                assert!(expand(&data[..end], 5, little, &request).is_err());
            }
            let budget = rrrah_core::MemoryBudget::new(9);
            let mut request = request.clone();
            request.memory_budget = Some(budget.clone());
            assert!(expand(&data, 5, little, &request).is_err());
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn work_limit_and_interior_cancellation_release_admitted_scratch() {
        let data = vec![0u8; 4_000_004];
        let budget = rrrah_core::MemoryBudget::new(300_000);
        let mut request = DecodeRequest::new("overwork.dcm");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            expand(&data, 1, true, &request),
            Err(RasterDecodeError::InvalidDicom("palette segment work limit"))
        ));
        assert_eq!(budget.used(), 0);
        assert!(budget.peak() > 0);
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        let mut stream = Stream {
            bytes: &[0, 0, 0, 0],
            little: true,
            visits: 255,
            request: &request,
        };
        assert!(matches!(
            stream.segment(0),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn malformed_expansion_is_rejected_before_large_output_admission() {
        for little in [false, true] {
            for words in [
                &[][..],
                &[0, 1, 9],
                &[1, 65535, 0],
                &[0, 1, 0, 2, 1, 2, 0],
                &[0, 1, 0, 2, 1, 6, 0],
            ] {
                let budget = rrrah_core::MemoryBudget::new(1);
                let mut request = DecodeRequest::new("malformed.dcm");
                request.memory_budget = Some(budget.clone());
                assert!(
                    matches!(
                        expand(&bytes(words, little), 65536, little, &request),
                        Err(RasterDecodeError::InvalidDicom(_))
                    ),
                    "{words:?}"
                );
                assert!(budget.peak() <= 1);
                assert_eq!(budget.used(), 0);
            }
        }
    }
}
