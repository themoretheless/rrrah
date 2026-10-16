//! Heap admission for the pinned flate2/zlib-rs decompression backend.
// Conservative state+history capacity for zlib-rs 0.6.8. The allocator
// probe in ktx2 tests guards this bound on each target.
pub(crate) const INFLATE_BYTES: u64 = rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES;
