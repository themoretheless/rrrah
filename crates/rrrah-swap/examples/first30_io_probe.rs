//! Current bounded warm-filesystem baseline for RR-0020..RR-0022.
use rrrah_memory::MemoryBudget;
use rrrah_swap::{SwapLimits, SwapStore};
use std::{
    io::{Cursor, Read},
    time::Instant,
};

struct Counted<R> {
    inner: R,
    calls: usize,
}
impl<R: Read> Read for Counted<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        self.calls += 1;
        self.inner.read(output)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = SwapStore::new(
        directory.path(),
        SwapLimits {
            max_bytes: 64 * 1024 * 1024,
            max_objects: Some(4),
        },
    )?;
    let ram = MemoryBudget::new(32 * 1024 * 1024);
    let index = MemoryBudget::new(8192);
    let scratch = MemoryBudget::new(65536);
    println!(
        "{{\"scope\":\"actual rrrah-swap APIs, warm temporary filesystem, synthetic bytes; cold disk and alternative algorithms unmeasured\",\"samples\":["
    );
    let mut comma = "";
    for bytes in [0usize, 65539, 16 * 1024 * 1024] {
        let mut source = ram.try_buffer(bytes, 0u8)?;
        for (i, value) in source.iter_mut().enumerate() {
            *value = i.to_le_bytes()[0];
        }
        let mut destination = ram.try_buffer(bytes, 0u8)?;
        for iteration in 0..13 {
            let start = Instant::now();
            let handle = store.write_chunks(bytes as u64, [&source[..]], || false)?;
            let write_ns = start.elapsed().as_nanos();
            let start = Instant::now();
            store.read_into(&handle, &mut destination, || false)?;
            let read_ns = start.elapsed().as_nanos();
            assert_eq!(&*source, &*destination);
            drop(handle);
            let mut reader = Counted {
                inner: Cursor::new(&source[..]),
                calls: 0,
            };
            let start = Instant::now();
            let segmented = store.write_segmented(&mut reader, bytes as u64, &index, &scratch, || false)?;
            let segmented_ns = start.elapsed().as_nanos();
            drop(segmented);
            assert_eq!(store.usage()?.bytes, 0);
            assert_eq!(store.usage()?.objects, 0);
            assert_eq!(index.used(), 0);
            assert_eq!(scratch.used(), 0);
            if iteration >= 3 {
                print!(
                    "{comma}{{\"bytes\":{bytes},\"sample\":{},\"write_chunks_ns\":{write_ns},\"read_into_ns\":{read_ns},\"write_segmented_ns\":{segmented_ns},\"reader_calls_including_eof\":{}}}",
                    iteration - 3,
                    reader.calls
                );
                comma = ",\n";
            }
        }
    }
    assert_eq!(ram.used(), 0);
    println!(
        "\n],\"final_ram\":0,\"final_disk_bytes\":0,\"index_peak\":{},\"scratch_peak\":{},\"ram_peak\":{}}}",
        index.peak(),
        scratch.peak(),
        ram.peak()
    );
    Ok(())
}
