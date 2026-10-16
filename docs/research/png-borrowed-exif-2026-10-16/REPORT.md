# Read PNG orientation without copying the full EXIF block

PngDecoder now overrides ImageDecoder::orientation to call the same Orientation::from_exif_chunk parser on borrowed stored EXIF bytes. Missing/invalid orientation keeps NoTransforms behavior. Public exif_metadata() still returns the original complete owned metadata and can be called repeatedly; no destructive extraction is introduced.

The old trait default first called exif_metadata(), cloning the complete block. A test-only allocator probe compares that exact getter+parser path with the new orientation method on the same constructed current decoder. Synthetic eXIf contains a valid little-endian TIFF Orientation6 entry and4MiB padding (4194330bytes total). Getter/default path: one4194330-byte allocation; borrowed method: **zero allocations /zero bytes**. Both return Rotate90 and release all observed transient allocations. Repeated full EXIF getter calls remain byte-identical before/after the orientation query. Source generation, probe, linked library digest and raw results are retained.

Full public decode library: **201 passed /5 ignored /zero failures**, including all eight independent EXIF permutations at RGBA8/16 and existing ICC/metadata controls. Animation library: **34 passed /1 ignored /zero failures**, including APNG frame composition, precision, CRC, cancellation and color controls. This safe production change also avoids the full EXIF clone in APNG's orientation admission query.

This removes one proven redundant metadata copy. Decoder-owned EXIF storage, fixed PNG decoder state, row/unfiltering storage and ICC copies are still separate shared-root accounting work. No wall-clock speedup or complete codec-memory qualification claim. Unsafe allocator instrumentation remains only in this standalone research probe.

Current viewer gate: **285 passed /80 ignored /zero failures** (`viewer.log`).
