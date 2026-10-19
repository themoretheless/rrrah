# JPEG XL retained ICC admission — 2026-10-19

## Actual gap and change

The general raster wrapper already admitted retained ICC capacity, and core
`RasterColorMetadata` already shared the profile and reservation through an Arc.
Swap also already restored ICC with a budget reservation. The remaining JXL gap
was **allocation before admission**: `image.rendered_icc()` constructed the final
profile while codec working storage was live, then the general wrapper charged
it after the decoder returned and released that working storage.

New optional APIs admit final output before allocating it:

- `colour_encoding_to_icc_with_admission` builds synthesis metadata, reports the
  exact final length, then allocates the final Vec with `try_reserve_exact`.
- `JxlImage::rendered_icc_with_admission` uses that API for synthesized profiles;
  for a raw/unrecognized profile it admits its length before copying it.
- The JXL decoder reserves final-profile bytes from the request budget, preserves
  typed `DecodeError::Memory` failures, and attaches the reservation using the
  existing `DecodedRaster::with_color_profile_reservation` API. The outer wrapper
  therefore does not charge the profile twice. Native frame clones share the ICC
  and credit; the final owner releases both.

The existing APIs remain available with a no-op admission callback. Profile
bytes and color interpretation are unchanged. Temporary synthesis buffers are
not covered by this final-output reservation.

## Verification

`rendered_profile_admission_covers_synthesized_and_unrecognized_icc_paths`
exercises both branches. Its raw-profile branch uses the existing linear RGB ICC
fixture with an authored three-point TRC (not reducible to the codec's simple
transfer encoding), and verifies the returned raw profile byte-for-byte.
For both branches, one byte short refuses with a typed memory error before final
allocation; the exact size succeeds, reports exact Vec capacity, and releases its
credit after output teardown. This is an allocation/transport test, not a color
qualification of the authored curve.

The native output ownership regression now includes the ICC capacity alongside
pixels. It verifies refusal with only codec/pixel credit, one-byte-short final
ICC credit, exact success, shared clone ownership, and final root zero.

The independently encoded embedded-ICC animation still preserves native samples,
original profile, timing and selection. Its complete public managed peak is now
70355 bytes (previous working-admission snapshot: 69539), because the 816-byte
rendered profile is now charged while codec work is still live. This is corrected
accounting, not a performance gain. Exact-peak retry and one-byte-short refusal
continue to pass. All 678 incomplete prefixes release their reservations,
including 189 failures before frame admission.

`suites.log`, `app.log`, `metal.log` and `format.log` contain terminal results.
The explicitly selected Metal test covers five JXL fixtures /15 frames, pinned
RAM, full root pressure, native/prepared-float swap, refusal/retry, exact timing
and native/SDR/half transport. It does not independently certify GPU color
accuracy or physical HDR calibration.

## Open scope

ICC entropy state, header/container buffers, TOC/frame metadata, color-profile
parsing/copies and temporary synthesis descriptor/tag/transfer-table allocations
remain outside these targeted reservations. Allocator bookkeeping and RSS are
not claimed bounded. Large-profile/producer coverage, full-history animation
cost, six pending catalog formats, physical disk/live GUI endurance, physical
HDR and deferred NVIDIA/CUDA qualification remain open. The overall goal is
incomplete.


Validation totals: jxl-color 18, animation 35 /1 ignored, decode 227 /5 ignored,
app 288 /81 ignored: 568 ordinary tests passed /87 ignored. The explicitly
selected actual Metal test also passed (15 frames, final root zero). Workspace
formatting and source whitespace checks passed. The existing upstream lifetime
syntax warning remains; no strict Clippy qualification is claimed.
