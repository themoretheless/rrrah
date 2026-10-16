# WAL external-palette cache identity

Native WAL pixels depend on `pics/colormap.pcx`. The previous raster display key fingerprinted only the WAL source, so an unchanged WAL could return old colors from RAM after its palette changed. `repro.log` records the failing regression (zero decoder calls instead of one).

The display key now includes a domain-separated fingerprint of native external dependencies. The tuple layout and ordinary source fingerprints remain unchanged. Missing/unreadable palettes disable cache lookup so decoding returns its actual error. Magic-recognized rasters with misleading `.wal` suffixes do not acquire palette dependencies.

The regression changes palette bytes while preserving size and mtime, verifies the WAL fingerprint is unchanged, checks exact black-to-red pixels in RAM and spilled-swap cases, refuses old swap restoration, checks missing-palette failure preserves valid cache membership, and verifies held old pixels stay immutable and all memory credits return to zero. A separate PNG-with-WAL-suffix control checks unchanged keys and RAM hits without a palette.

Focused WAL regression: passed. Full viewer verification: 284 passed, 79 ignored, zero failures (`viewer.log`), including both new controls.

This is cache coherence evidence for external WAL palettes, not full WAL format, GPU, HDR, or concurrent file-write qualification. Existing sampled source identity semantics remain in use.

## Prefetch and thumbnail follow-up

A native WAL prefetch regression holds the original black-palette neighbour lease, changes the palette to green while restoring its mtime, and verifies a distinct retained green frame. Repeated prefetch hits the current green entry with foreground DecodeGate admission blocked; a generation watchdog bounds accidental cache misses. Both old and new leases retain memory after cache destruction, and the final lease release returns root use to zero. The thumbnail regression also now restores palette mtime and asserts source-stamp changes alongside exact output colors. No disk thumbnail cache exists in this loader: thumbnails decode directly and compare source stamps before publication.

Follow-up full viewer gate: **285 passed, 79 ignored, zero failures** (`prefetch-viewer.log`), including the bounded prefetch and strengthened thumbnail regressions.
