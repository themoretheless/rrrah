//! Reproduce aggregate restore-cap bypass while a previous budget owner lives.
use rrrah_cache::{CacheLimits, ImageSwapCache, ImageSwapConfig, SwapPayload, SwapPayloadError};
use rrrah_memory::{MemoryBudget, SharedBuffer};
use std::io::{Read, Write};
#[derive(Debug)]
struct Payload(SharedBuffer<u8>);
impl SwapPayload for Payload {
    fn capacity_bytes(&self) -> u64 {
        self.0.capacity_bytes()
    }
    fn payload_len(&self) -> std::io::Result<u64> {
        Ok(self.0.len() as u64)
    }
    fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()> {
        writer.write_all(&self.0)
    }
    fn read_payload(
        reader: &mut impl Read,
        bytes: u64,
        budget: &MemoryBudget,
    ) -> Result<Self, SwapPayloadError> {
        let mut buffer = budget
            .try_buffer(bytes as usize, 0u8)
            .map_err(SwapPayloadError::Memory)?;
        reader
            .read_exact(&mut buffer)
            .map_err(SwapPayloadError::Invalid)?;
        Ok(Self(buffer.freeze()))
    }
}
fn main() {
    let directory = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(1024);
    let mut swap = ImageSwapCache::<u8, Payload>::new_with_budgets(
        directory.path(),
        ImageSwapConfig {
            limits: CacheLimits::bytes(1024),
            queue_bytes: 4,
            queue_count: 1,
            restore_bytes: 4,
        },
        MemoryBudget::new(4),
        root.clone(),
    )
    .unwrap();
    swap.enqueue(1, Payload(root.try_buffer(4, 7u8).unwrap().freeze()));
    swap.wait_idle().unwrap();
    let first = swap.try_get(&1, || false).unwrap().unwrap();
    swap.set_restore_budget(root.clone());
    let second = swap.try_get(&1, || false);
    let bypass = matches!(&second, Ok(Some(_)));
    println!(
        "restore_limit=4 live_restored_bytes={} rebind_bypassed_limit={bypass}",
        root.used()
    );
    drop(second);
    drop(first);
    assert_eq!(root.used(), 0);
    if bypass {
        std::process::exit(1);
    }
}
