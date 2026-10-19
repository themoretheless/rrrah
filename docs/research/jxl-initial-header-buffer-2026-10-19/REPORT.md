# Initial JPEG XL codestream buffer admission — 2026-10-19

## Gap and implementation

The caller-owned source was already budgeted, as were selected codec groups and
ICC encoded/reconstructed/final-output buffers. `UninitializedJxlImage::buffer`
was another allocation: it accumulated decontainerized codestream bytes until
metadata initialization, then moved into deferred initial frame loading. Its
capacity and possible old/new overlap during growth were previously uncharged.

The optional `feed_bytes_with_header_admission` API now plans geometric capacity
growth and requests `new_capacity + old_capacity` before allocation. It then uses
`try_reserve_exact` for that planned capacity. Each codestream part emitted by
the container parser is handled separately; a single input feed can contain
multiple parts. Default `feed_bytes` retains the previous growth path.

The rrrah decoder holds a single high-water Reservation, growing it via
`ensure_bytes` across incomplete initialization attempts. It checks generation
cancellation before admission, preserves typed memory refusals, and keeps credit
until `resume_frame_loading` consumes/drops the pending initial Vec. That credit
is then released while codec working/output ownership remains. Error paths drop
codec state before this reservation. Reallocation overlap is conservative:
credit is not reduced after each growth, and no RSS/allocator-overhead bound is
claimed.

## Verification

The independent libjxl large-ICC control produces three header growth requests
when fed in 4096-byte chunks: 4043, 12182, 24417 bytes. The values differ from
input-chunk lengths because the input is a container with codestream parts and
framing, not a raw byte-identical codestream.

A header-only budget test verifies the exact requests, zero loaded frame groups
(grid tracker zero), retained 8712-byte original ICC, and 24417-byte high water.
One byte short of the first allocation refuses with peak zero; one byte short
of the last refuses with only 12182 bytes admitted. Teardown releases all header
credit. This isolated budget covers only the initial header Vec; ICC storage is
covered by separate tests and the production callback.

The public large-ICC decode peak is 135023 bytes, with one-byte-short typed
refusal and exact-peak retry. After decode only the frame's pixel/profile capacity
remains charged; after frame teardown root usage is zero. The small ICC control's
public peak is now 70550 bytes (prior 70355), reflecting the different live overlap
with the initial header reservation. This is corrected conservative accounting,
not a speedup claim.

Existing native u8/u16/f32 controls, rational timing, signed-linear color proofs,
ICC and cancellation paths, 678 incomplete prefixes, and the 600-frame bounded
animation remain passing. The 600-frame selected-frame root peak is 251528 bytes
under its existing 262144-byte limit. `targeted.log` records these measurements.

## Remaining scope

This API covers the initial accumulated codestream Vec, not all data named
"header": container-parser/auxiliary boxes, inner frame-parser buffering,
header/TOC descriptors, entropy state, profile parsing/copies and temporary
synthesis metadata/tables still need separate accounting. Exact-allocation
capacity checks do not establish a total heap/RSS guarantee. Maximum-sized
producer inputs, full-history JXL animation cost, physical slow disks/live GUI,
physical HDR, six pending catalog formats and deferred NVIDIA/CUDA remain open.
The overall goal is incomplete.


Terminal validation: `suites.log` has jxl-color 19, animation 35 /1 ignored,
decode 230 /5 ignored; `app.log` has 288 /81 ignored: 572 ordinary tests passed
/87 ignored. `metal.log` records the additional actual Apple M4 Max test passing
18 frames through RAM/native-and-float swap, full pressure refusal/retry and
SDR/half readback, with final root zero. `format.log` and source whitespace checks
passed. The existing upstream lifetime-syntax warning remains; no strict Clippy
qualification or physical HDR certification is claimed.
