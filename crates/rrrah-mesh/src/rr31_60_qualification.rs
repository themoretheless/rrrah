//! Negative admission control for adding a second buffered layer to OBJ/PLY swap.
use super::*;
use std::io::{self, Read};
fn restore(
    kind: &str,
    input: &mut impl Read,
    bytes: u64,
    budget: &rrrah_core::MemoryBudget,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut encoded = Vec::new();
    if kind == "obj" {
        let model = read_obj_swap_payload(input, bytes, budget)?;
        write_obj_swap_payload(&model, &mut encoded)?;
    } else {
        let model = read_ply_swap_payload(input, bytes, budget)?;
        write_ply_swap_payload(&model, &mut encoded)?;
    }
    Ok(encoded)
}
#[test]
fn second_stream_buffer_changes_tight_admission_and_read_errors_stay_bounded() {
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
    for (kind, name) in [("obj", "obj-shared-metadata.obj"), ("ply", "ply-64-le.ply")] {
        let request = DecodeRequest::new(fixtures.join(name));
        let mut wire = Vec::new();
        if kind == "obj" {
            write_obj_swap_payload(&decode_obj(&request).unwrap(), &mut wire).unwrap();
        } else {
            write_ply_swap_payload(&decode_ply(&request).unwrap(), &mut wire).unwrap();
        }
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        assert_eq!(
            restore(kind, &mut wire.as_slice(), wire.len() as u64, &budget).unwrap(),
            wire
        );
        assert_eq!(budget.used(), 0);
        let peak = budget.peak();
        let tight = rrrah_core::MemoryBudget::new(peak);
        assert_eq!(
            restore(kind, &mut wire.as_slice(), wire.len() as u64, &tight).unwrap(),
            wire
        );
        assert_eq!(tight.used(), 0);
        // A properly charged additional 16KiB buffer cannot retain old admission.
        assert!(
            tight.try_reserve(16384).is_err(),
            "fixture must exercise a genuinely tight budget"
        );
        let extra = rrrah_core::MemoryBudget::new(peak + 16384);
        let scratch = extra.try_reserve(16384).unwrap();
        let mut bounded = wire.as_slice().take(wire.len() as u64);
        let mut buffered = std::io::BufReader::with_capacity(16384, &mut bounded);
        assert_eq!(
            restore(kind, &mut buffered, wire.len() as u64, &extra).unwrap(),
            wire
        );
        drop(buffered);
        drop(scratch);
        assert_eq!(extra.used(), 0);
        struct Chunks<'a> {
            data: &'a [u8],
            calls: usize,
            bytes: usize,
            cancel_at: usize,
            interrupt: bool,
        }
        impl Read for Chunks<'_> {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.interrupt {
                    self.interrupt = false;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                if self.bytes >= self.cancel_at {
                    return Err(io::Error::other("cancelled"));
                }
                let count = out.len().min(3).min(self.data.len());
                out[..count].copy_from_slice(&self.data[..count]);
                self.data = &self.data[count..];
                self.bytes += count;
                Ok(count)
            }
        }
        let mut joined = wire.clone();
        joined.extend_from_slice(b"NEXT");
        let mut reader = Chunks {
            data: &joined,
            calls: 0,
            bytes: 0,
            cancel_at: usize::MAX,
            interrupt: true,
        };
        assert_eq!(
            restore(kind, &mut reader, wire.len() as u64, &budget).unwrap(),
            wire
        );
        assert_eq!(reader.bytes, wire.len());
        assert_eq!(reader.data, b"NEXT");
        for cut in [0, wire.len() / 2, wire.len() - 1] {
            let mut input = &wire[..cut];
            assert!(restore(kind, &mut input, wire.len() as u64, &budget).is_err());
            assert_eq!(budget.used(), 0);
        }
        let mut reader = Chunks {
            data: &wire,
            calls: 0,
            bytes: 0,
            cancel_at: 24,
            interrupt: false,
        };
        assert!(restore(kind, &mut reader, wire.len() as u64, &budget).is_err());
        assert!(reader.bytes <= 27);
        assert_eq!(budget.used(), 0);
        let mut corrupted = wire.clone();
        corrupted[0] ^= 0xff;
        assert!(restore(kind, &mut corrupted.as_slice(), wire.len() as u64, &budget).is_err());
        assert_eq!(budget.used(), 0);
        eprintln!(
            "STREAM_BUFFER_NEGATIVE kind={kind} wire_bytes={} baseline_peak={peak} buffered_peak={} tight_baseline_pass=true scratch_refused=true short_interrupted_cancel_corrupt_framing=true final=0",
            wire.len(),
            extra.peak()
        );
    }
}
