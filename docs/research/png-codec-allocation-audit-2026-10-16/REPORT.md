# PNG private allocations bypass best-effort Limits

A standalone test-only global allocator probe links the current built vendored image library and its locked png0.18.1 dependency. It measures successful Rust allocation requests during one single-threaded decoder operation, including realloc growth, and checks that all measured live allocations return to0 before observation ends. Caller source and preallocated1024-byte output are outside the measurement. Full decoded pixels exactly match the pinned16x16 RGBA fixture oracle.

Results (requested Rust heap bytes, not RSS/allocator overhead/thread stacks):

|Operation|Configured max_alloc|Result|Peak private bytes|Allocation calls|
|---|---:|---|---:|---:|
|Constructor|0|error|20856|5|
|Constructor|64|success|20856|5|
|Constructor|1048576|success|20856|5|
|Full decode excluding caller output|1048576|success|27768|6|

The source documents png Limits as best effort. StreamingDecoder constructs a boxed fdeflate decompressor/Huffman tables and initial chunk storage before its byte-limit checks. The unfiltering buffer also has independently allocated initial capacity. Therefore passing available global bytes as max_alloc cannot be treated as shared-root admission, even for a tiny PNG. This is a verified remaining gap, not a completed memory-budget fix.

The probe uses unsafe allocator delegation only in this research executable, never production. Its System calls preserve allocation/deallocation layouts and count only successfully returned allocations. Build command, linked rlib digest, source hashes and JSONL measurements are retained. Initial direct-link attempt needed the native dav1d library search path; the successful build obtains it from pkg-config.

Next work must admit decoder-private storage before construction and track growth/lifetimes or otherwise establish a validated bounded workspace. Skipping unused compressed textual metadata can reduce work but cannot replace such accounting. No arbitrary fixed reserve or best-effort limit is presented as full PNG heap qualification. TIFF and AVIF private allocations still need their own contracts.
