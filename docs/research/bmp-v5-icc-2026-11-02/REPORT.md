# Native BMP V4/V5 color metadata and physical pixels — 2026-11-02

The ordinary BMP decoder previously classified every BMP as sRGB, ignoring
V4/V5 color declarations. Native metadata handling now borrows the V5 ICC slice,
validates its extent/header, admits one owned copy before allocating it, and
passes it through the existing shared ICC/color-management path. Pixel decoding
cannot consume profile bytes to finish a truncated image. Offsets are relative
to the DIB header, as specified by [Microsoft BITMAPV5HEADER](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapv5header).

Explicit sRGB and Windows-default sRGB retain their interpretation; unused
profile fields are ignored for these declarations. Linked profiles fail
explicitly without opening external paths. Calibrated/unknown declarations
remain unqualified rather than being silently labeled sRGB. Native calibrated
endpoint/gamma conversion remains implementation work; full BMP qualification
is not claimed.

Eight metadata tests pass: native samples/ICC and shared ownership; exact native
profile+output+overlap quota and one-byte-short refusal; malformed offsets,
lengths, signature and linked-profile refusal; explicit color precedence; and
pixel truncation refusing to borrow ICC data; physical resolution interpretation
and negative-resolution refusal. The complete decoder library passes276 tests,
with9 external/manual cases ignored: [decoder log](decode-regression.log).

Two CC0 authored32-bit BITFIELDS sources cover top-down and bottom-up rows,
non-neutral RGB and four alpha values including0 and1/255. The test-only
generator uses existing ICC bytes and independent LittleCMS double conversion
to linear-sRGB D65 (relative colorimetric, no optimization/cache). Every source
byte matches the authored native samples; prepared RGB matches the independent
reference within2e-4, with alpha excluded from the color transform.

Required actual Apple M4 Max Metal passes twelve native/prepared storage cases.
Every serialized restored payload is exact, including native ICC and samples;
exhausted-root refusal, cancellation and retry preserve the object. Both31×29
SDR and half-float HDR frames agree with the independent linear reference within
one output code/half-float ULP; final managed root/queue usage is zero. One test
passed, zero failed, zero ignored: [Metal log](metal.log).

## Pixels-per-meter geometry

Windows 40/52/56/108/124-byte headers now preserve explicit pixel width/height
as `Y pixels/m / X pixels/m`. Both positive equal densities explicitly declare
square pixels; either zero axis leaves the ratio undefined. Negative densities
fail before profile/pixel allocation. Other header layouts remain the pixel
decoder's responsibility and are not interpreted using these field offsets.
Forty authored combinations exercise all five headers, ratios2,1/2,1, zero axes
and extreme positive signed 32-bit values through native decoding and display
preparation; three negative combinations release all credit with peak0.

The twelve Metal cases combine both row origins, both storage modes and
undefined, 2 and 0.5 physical ratios. Their reference starts with the independent
LittleCMS samples and an explicitly assigned aspect; non-square controls must
produce a different full SDR/HDR frame from the square control. This checks
container geometry transport against the common renderer, not an independent
qualification of that renderer's geometry implementation. Exact swap payloads
include the aspect metadata. The current ordinary viewer suite passes354 tests (105 manual
cases ignored): [latest viewer regression](viewer-regression.log).

Fixtures/oracle: `tests/fixtures/raster/bmp-v5-icc-reference.json` and its two BMPs.
Reproduce them with `python3 scripts/generate-bmp-v5-icc-fixtures.py`; LittleCMS
is test/reference-only. No production native dependency was added. [Source pins](sources.json).

```sh
cargo test --locked -p rrrah-decode --lib bmp_color
RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 RRRAH_GPU_VENDOR=any cargo test --locked -p rrrah --test bmp_profile_metal -- --ignored --nocapture --test-threads=1
```

Broader producers, calibrated V4/V5 color, linked-resource policy, other BMP
storage variants, physical display accuracy and full100-format coverage remain
open. The independently observed sustained-worker prefetch refusal remains a
separate unresolved failure; these finite BMP tests do not supersede it.

## Exact calibration metadata retention

Native V4/V5 import now retains an explicit `BmpCalibratedRgb` declaration rather than discarding endpoints/gammas into `Unspecified`. `BmpRgbCalibration` keeps all nine signed endpoint words, three unsigned gamma words, the DIB header layout and V5 intent in fixed inline56-byte storage. No floating conversion or independent white-point/gamma interpretation is asserted. Unknown intent and endpoint/gamma bit patterns remain raw metadata; retaining them is not numerical validation of a color space. V4 has no intent field; bytes outside that header are ignored.

The [Microsoft V5 API reference](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapv5header) describes gamma as unsigned16.16; the [V4 protocol specification](https://learn.microsoft.com/openspecs/windows_protocols/ms-wmf/071b0c0d-c2df-4f1c-9828-d03c26002c61) describes shifted8.8. [CIEXYZ](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-ciexyz) specifies2.30 coordinates. This checkpoint keeps original bits and layout so later independently qualified conversion can resolve these semantics without reparsing source files. Neither source inspection nor authored metadata proves Windows color-conversion interoperability.

The raster payload adds color tag9 with exactly56 bytes of metadata. Existing tags and payload layouts remain unchanged; malformed lengths, truncated metadata/pixels and invalid DIB layouts refuse. The metadata is read on the stack, without an ICC-sized allocation. The live shared color object still has ordinary fixed object bookkeeping; the managed pixel/variable-profile budget does not claim to account for every allocator/struct byte.

Evidence:

- Public native decoding preserves asymmetric endpoint/gamma bits and V5 intent for both header layouts, even with the untagged-sRGB override enabled. Pixels remain unchanged; capacity4, native peak7, final0. Explicit unqualified calibration requires a color transform before display; it is never labeled sRGB.
- [Cache suite](calibration-cache.log):162 passed/5 ignored. Exact payload round-trip, pixel aspect and malformed/truncated records pass at a4-byte pixel budget. Actual disk-swap publication/restore passes both layouts; full-root refusal and cancellation leave the object available for exact retry, with one write/read, no cache errors and managed final0.
- [Identity test](calibration-identity.log):1 passed/zero ignored. Caller-supplied decode interpretation distinguishes V4/V5, every calibration-field byte and every intent bit, as well as unspecified color.
- [Decoder suite](decode-regression.log):276 passed/9 ignored. [Viewer suite](viewer-regression.log):354 passed/105 ignored. [Production check](calibration-production-check.log), formatting and whitespace checks pass. Formatting after the production check did not change production behavior.

[Calibration source/binary pins](calibration-sources.json) are separate from the earlier ICC/physical-aspect Metal checkpoint's source pins. No calibrated-color transform, calibrated BMP Metal output, new fully qualified format, or physical HDR qualification is claimed. The catalogue remains94 implemented subsets,6 pending,0 fully qualified.
