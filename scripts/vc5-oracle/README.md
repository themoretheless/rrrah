# VC-5 coefficient oracle

Test-only host adapter for unmodified GoPro source at commit
446c736a38fb14f51343605c0780d347dc602f89. Download `vlc.c` and `vlc.h`
from source/lib/vc5_decoder and `table17.inc` from source/lib/vc5_common.
Keep those three files together in REFERENCE. They are MIT/Apache-2.0 licensed;
the retained MIT notice is crates/rrrah-decode/data/vc5-LICENSE-MIT.

Build: `cc -O2 -I scripts/vc5-oracle -I REFERENCE scripts/vc5-oracle/main.c REFERENCE/vlc.c -o ORACLE`.
Run: `ORACLE BLOCK COEFFICIENT_COUNT OUTPUT.i32le`.
Compare with the public native `vc5_entropy_dump` example using the same arguments.
Both write all signed quantized coefficients in little-endian i32 form and check
the end marker. These values precede inverse companding and quantization.
The C source is used only to produce independent test/reference output; production
VC-5 decoding uses the Rust implementation and attributed numerical codebook.

For dequantization also download unmodified `companding.c` and `pixel.h` from
source/lib/vc5_common, and `dequantize.c` from source/lib/vc5_decoder at the same
commit. `pixel.h` is also required by the shared host header. Build:
`cc -O2 -DNDEBUG -I scripts/vc5-oracle -I REFERENCE scripts/vc5-oracle/dequantize-main.c REFERENCE/companding.c REFERENCE/dequantize.c -o DEQUANT_ORACLE`.
Run `DEQUANT_ORACLE OUTPUT.i16le` to reproduce
`tests/fixtures/vc5/dequantize-gopro.i16le`. NDEBUG selects the reference release
saturation behavior; debug assertions reject intentionally overflowing cases.
The golden covers every signed codebook magnitude for eight quantization factors.

Horizontal inverse-filter oracle: download unmodified `inverse.c` from
source/lib/vc5_decoder and `macros.h` from source/lib/common/private. Extract the
complete `InvertHorizontal16s` function without edits into
`REFERENCE/horizontal-original.inc` (from its declaration through its closing
brace, before the next documentation block). Build:
`cc -O2 -DNDEBUG -I scripts/vc5-oracle -I REFERENCE scripts/vc5-oracle/horizontal-main.c -o HORIZONTAL_ORACLE`.
Run `HORIZONTAL_ORACLE OUTPUT.i16le` to reproduce the horizontal golden. Records
contain source/output widths, lowpass/highpass input rows and reference output.
The 256 rows cover widths 3–10, odd/even output and both saturation boundaries.

For the prescale=2 oracle also extract unchanged `InvertHorizontalDescale16s`
from the same `inverse.c` into `REFERENCE/horizontal-descale-original.inc`.
The horizontal host now includes both functions. Run
`HORIZONTAL_ORACLE OUTPUT.i16le --descale` to reproduce
`horizontal-descaled-gopro.i16le`, including signed i16 saturation boundaries.

Spatial mode-0 oracle: extract the complete unchanged `InvertSpatialQuant16s`
function into `REFERENCE/spatial-original.inc`. Build:
`cc -O2 -DNDEBUG -I scripts/vc5-oracle -I REFERENCE scripts/vc5-oracle/spatial-main.c REFERENCE/dequantize.c REFERENCE/companding.c -o SPATIAL_ORACLE`.
Run `SPATIAL_ORACLE OUTPUT.i16le` to reproduce `spatial-gopro.i16le`. Each record
contains width/height/output-width/output-height, four input bands, and expected
output, all little-endian i16. Band indices LL=0,LH=1,HL=2,HH=3 match the pinned
GoPro vc5_common/wavelet.h enumeration. The host uses independent malloc/free
scratch allocation; Rust uses caller-provided scratch/output buffers.

Spatial prescale=2: additionally extract unchanged `InvertSpatialQuantDescale16s`
into `REFERENCE/spatial-descale-original.inc`. The spatial host includes both
spatial and horizontal variants. Run `SPATIAL_ORACLE OUTPUT.i16le --descale`
to reproduce `spatial-descaled-gopro.i16le` with the same record layout.

Real-channel spatial host: build `spatial-file-main.c` with the same include paths
and dequantization/companding sources. It accepts INPUT OUTPUT paths. Input is an
8-word little-endian u16 header (w,h,ow,oh,prescale,qLH,qHL,qHH), then LL/LH/HL/HH
little-endian i16 bands. `scripts/qualify-gpr-channel-oracle.py` admits the pinned
HERO9 essence SHA256, uses the independently decoded entropy outputs under
WORK/all-bands, and reconstructs channel 0 through three reference spatial calls.
This host assembles bands explicitly; it is not a full GoPro GPR file decoder.

Shared host headers now also require unmodified `macros.h` from
source/lib/common/private and `logcurve.h` from source/lib/vc5_common.
Inverse-log oracle: compile `logcurve-main.c` with unmodified `logcurve.c` using
the same include paths, then run it with the output path to reproduce all 4096
reference lookup values. Component-cell oracle: extract the complete unchanged
PackComponentsToRAW from vc5_decoder/raw.c into `REFERENCE/pack-original.inc`,
compile `bayer-cell-main.c` with `logcurve.c`, and run it with the output path.
That golden has 768 records: output precision, four GS/RG/BG/GD signed components
and four unsigned RGGB outputs, each little-endian 16-bit. The host selects RGGB
placement with explicit output-bit-depth argument; physical file-format admission
is separate from this component-value qualification.

Full RGGB packing host: compile `bayer-file-main.c` with the unchanged logcurve.c
and the same reference include files. Run
`BAYER_ORACLE COMPONENT_WIDTH COMPONENT_HEIGHT CH0 CH1 CH2 CH3 OUTPUT.u16le`.
It reads the independently reconstructed little-endian i16 component files and
executes unchanged PackComponentsToRAW for 14-bit RGGB output. For pinned HERO9,
component extent is 2784 x 2088 and resulting sensor extent is 5568 x 4176.
This validates all sensor values with original GoPro stages under explicit test
assembly; complete GPR/DNG file-parser/color interpretation is a separate gate.
