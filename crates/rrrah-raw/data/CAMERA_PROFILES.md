# Camera calibration data

`camera-matrices.tsv` is derived from the make/model, aliases and three-plane
ColorMatrix entries in rawspeed `data/cameras.xml` at commit `c835b05aecfacb7343f7c424abd620aa12116c3f`.
Source: https://github.com/darktable-org/rawspeed/blob/c835b05aecfacb7343f7c424abd620aa12116c3f/data/cameras.xml
Upstream attribution: rawspeed contributors (see upstream Git history).
Data license: Creative Commons Attribution-ShareAlike 3.0 Unported:
https://creativecommons.org/licenses/by-sa/3.0/
This derived TSV remains under CC-BY-SA 3.0; Rust code retains the workspace license.
Changes: XML converted to sorted TSV, aliases expanded, coefficients preserved.
Source XML SHA-256: `d67d32beb3acf073a4ecc521daae29545f90bf79270749e9041031dedb89d166`.

Coefficients are XYZ-to-camera, scaled by 10000, following upstream's
D65/dcraw calibration convention. Matching uses exact make and model strings.
No approximate model matching or identity fallback is allowed. These matrices
are baseline calibrations, not camera JPEG looks or per-unit characterizations.
Updating this table requires a camera semantic recipe revision bump.

Sony metadata tag structure and XOR recurrence were checked against rawspeed
ArwDecoder.cpp and TiffTag.h; the local implementation uses bounded TIFF parsing.

WB layout references:
- https://github.com/darktable-org/rawspeed/blob/develop/src/librawspeed/decoders/Cr2Decoder.cpp
- https://github.com/darktable-org/rawspeed/blob/develop/src/librawspeed/decoders/ArwDecoder.cpp
- https://github.com/darktable-org/rawspeed/blob/develop/src/librawspeed/decoders/NefDecoder.cpp
- https://github.com/darktable-org/rawspeed/blob/develop/src/librawspeed/decoders/OrfDecoder.cpp
- https://github.com/darktable-org/rawspeed/blob/develop/src/librawspeed/decoders/PefDecoder.cpp

The native WB readers are covered by explicit-value TIFF fixtures and real-file
qualification. Nine public RAW cases and two local EOS R8 CR3 captures match
independent LibRaw sensor, WB and final matrix references; see
[`RAW_QUALIFICATION.md`](../../../docs/RAW_QUALIFICATION.md) for commands and limits.
Nikon encrypted ColorBalance, Sony DSLR-A100, other Pentax signatures and unknown
Canon ColorData versions remain unsupported or unqualified. Missing WB rejects
display instead of substituting neutral gains. Exact file-level contracts do not
certify every camera or remove the limits of bilinear demosaicing.
