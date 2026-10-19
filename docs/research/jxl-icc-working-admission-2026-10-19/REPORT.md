# JPEG XL ICC working-buffer admission — 2026-10-19

## Implementation

The prior reconstructed-output bounds patch made the advertised ICC output size
trustworthy. This follow-up admits the principal ICC byte buffers before their
allocation, through the existing parent memory budget:

1. `jxl-color::icc::read_icc_with_admission` calls an optional admission callback
   with `encoded_size + 18` before entropy decoder construction and the initial
   decoded command header allocation.
2. After decoding the at-most-18-byte command header and validating sizes, it
   calls with `encoded_size + 2 * decoded_size + 18`, before enlarging encoded
   storage and before reconstructed ICC/shuffle storage can be allocated. One
   output-sized term covers reconstruction, one covers the largest single live
   shuffle. The 18-byte term covers old header storage during exact reallocation.
3. Encoded storage grows with `try_reserve_exact` instead of geometric Vec growth.
4. `jxl-oxide::try_init_header_only_with_icc_admission` exposes this callback and
   preserves its original error type. Legacy APIs keep a no-op callback.
5. The rrrah JXL decoder reserves only increases of the absolute high-water mark,
   holds the credit across incomplete initialization attempts and through codec
   teardown, and maps the original `DecodeError::Memory` without hiding it in an
   opaque image codec failure. The callback also checks request cancellation.

The declarations precede uninitialized/initialized codec state so Rust drop order
releases codec buffers before their working credit, including error exits.
Encoded source-file storage remains separately admitted by the existing public
reader; new ICC working charges are additional to that ownership.

## Evidence

The independent libjxl RGBA16 embedded-ICC fixture from the preceding milestone
has a 520-byte original profile and 679-byte encoded file. Its callbacks are
405 bytes, then 1445 bytes. Seven-byte feeds trigger 50 callbacks while holding
exactly one 1445-byte high-water reservation, rather than accumulating charges.
No codec frame groups are loaded at this point (grid tracker remains zero).
Original ICC bytes equal the independently decoded source profile.

The public decoder refuses with a typed parent-memory error at either callback
when one byte short, reporting the correct request and already-held source/ICC
credit. The exact measured complete public decode peak is 69539 bytes; one byte
short refuses, an exact-peak retry succeeds, and the final root usage is zero.
`targeted.log` preserves these measurements.

The prefix regression tries all 678 incomplete source prefixes, including 189
errors with admitted ICC working credit before frame admission, releasing all
credit on every path. The ordinary test suites, app tests and actual Metal test are logged
alongside this report. Metal qualification includes 15 frames across five JXL
controls, native and prepared-float swap, RAM pressure, timing/index/count and
SDR/half attachments; it is transport/readback evidence, not physical HDR proof.

## Scope still open

The bound covers primary encoded, reconstructed and shuffle payload storage,
with the stated small reallocation overlap. It is not a strict allocator/RSS
bound: entropy decoder tables, header buffering, TOC/frame metadata, color-profile
parsing and copies, rendered/retained ICC copy and allocator overhead still need
separate accounting. The small embedded profile does not qualify large ICC or
all producer/profile combinations. Cancellation is checked at admission stages;
it is not checked inside every entropy/profile reconstruction loop.

Full-history JXL animation loading, broader 100-format correctness, physical
slow-disk/live-GUI endurance, physical HDR and deferred NVIDIA/CUDA remain open.
This milestone does not complete the overall goal.


Ordinary suites passed: jxl-color 18, animation 35 /1 ignored, decode 226 /5
ignored, app 288 /81 ignored: 567 passed /87 ignored. The explicitly selected
Metal case is additional; see `metal.log` for its terminal result. Formatting
and source whitespace checks passed. One existing upstream lifetime-syntax
warning remains; this is not a strict Clippy qualification.


Subsequent retained-output work moves final rendered ICC allocation admission
into the codec and attaches that credit to the frame; see
[retained ICC report](../jxl-retained-icc-admission-2026-10-19/REPORT.md).
The 69539-byte peak and retained-copy gap above describe this earlier snapshot.
