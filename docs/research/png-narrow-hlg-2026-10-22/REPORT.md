# PNG narrow HLG: signed native RGB display conversion

Added an explicit integer narrow-HLG RGB method and PNG policy dispatch for cICP 9/18/0/0. RGB 16..235 at 8 bits (scaled by 256 for 16-bit storage) is expanded before inverse HLG OETF. Alpha stays full-range. Below-black scene light retains its sign; the luminance-dependent OOTF gain uses absolute scene luminance. White, display peak and system gamma remain explicit and the display black is zero. No tone mapping, clipping, inferred calibration, intermediate image buffer or production Colour dependency was added. Normalized RGB from video codecs keeps its prior route.

Primary references: ITU-R BT.2100-2 Table5/Note5h discusses extended signal values; PNG3 cICP recommends signed transfer extensions. Pinned upstream Colour inverse ARIB STD-B67 uses mirror handling of the negative quadratic branch; its BT.2100 HLG OOTF uses absolute scene luminance. URLs, commit and source hashes are in reference-sources.json. The native implementation also handles zero-luminance gamma=1 without losing chromatic components; undefined nonzero chroma at zero luminance with gamma<1 is explicitly refused rather than emitting infinity.

Independent authored checks: neutral-reference.py uses 70-digit Decimal arithmetic to regenerate 12 anchors across 8/16-bit storage. Complete integer ramps check finite monotone output, exact negative/positive reflection around black, retained excursions above nominal peak and unchanged alpha. One-byte-short output admission and cancellation after allocation return zero credit; exact output-only ownership and teardown are checked. Develop library: 101 passed / 2 ignored / 0 failed; public PNG color-policy focused test passed.

Independent external reference: the existing generator now additionally covers the retained W3C 1920x1080 RGB16 narrow-HLG attachment (source SHA-256 480bf843742c2d966bbab039fac7a6d4104c874ed1ae73613aca1b81fe2a4ded). It starts from hash-verified FFmpeg native RGB codes and derives the gamut matrix from chromaticities with Decimal precision70. Existing full-range HLG/PQ oracle hashes remain unchanged. The opt-in app conformance tests now require narrow-HLG policy acceptance and include all three external sources in linear-reference/RAM/swap/Metal checks.

Status at preparation: independent oracle and core/public gates passed; full decoder and Metal gates were still running. Terminal results below supersede this preparation status. No speed or physical display claim is implied.

## Terminal validation

- Full develop library: 101 passed / 2 ignored / 0 failed.
- Full decode library: 232 passed / 5 ignored / 0 failed.
- Required Apple M4 Max Metal external conformance gate: 2 passed / 0 ignored / 0 failed (metal.log). It now includes all three retained W3C sources, both native RGB16 checks and independent linear-reference transport. Across the three images all 6,220,800 native and linear pixels are checked. For the newly admitted narrow-HLG image, maximum absolute float32 error against Decimal is 0.0000019073486328125; full HLG remains 0.00000095367431640625 and PQ remains zero.

The narrow-HLG source passes managed RAM pin/lease protection, eviction to byte/count-bounded swap, complete shared-root refusal, cancellation after restore admission and retry. All native float bits and full before/after-swap SDR/HDR attachments are exact. Independent reference raster readbacks agree within one SDR code / 0.2% binary16 HDR tolerance at 256x144 output. Final managed root is zero, one successful swap read/write and zero errors per source. Test reference vectors and readback buffers are outside the production managed root; no RSS claim.

This confirms one external 16-bit narrow-HLG source and authored 8/16-bit ramp/alpha/admission controls. Broader external producers, translucent HDR PNG presentations, animations/full PNG variants, physical HDR output, NVIDIA and the remaining format qualification work are not completed by this gate. All tests are functional; elapsed times under shared host load are not accepted performance measurements.
