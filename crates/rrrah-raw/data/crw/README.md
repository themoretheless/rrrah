# Canon CRW table-zero data

The 209 numerical bytes are the first/remaining-symbol tables for decoder index
zero from LibRaw 0.22.2 `src/decoders/decoders_dcraw.cpp`:
https://github.com/LibRaw/LibRaw/blob/0.22.2/src/decoders/decoders_dcraw.cpp

Copyright 2019-2025 LibRaw LLC (info@libraw.org). LibRaw contains dcraw data,
copyright 1997-2018 Dave Coffin (dcoffin a cybercom o net).
These data files use the upstream CDDL-1.0 license option; see LICENSE.CDDL.
The representation is modified: arrays are concatenated into a binary and a
human-readable hexadecimal source. No upstream decoder implementation is
included here. All source bytes, including padding, are retained in table-zero.hex.
Include this directory and its license when redistributing the data.
