# Do not retain unused PNG text in the raster route

Vendored image PngDecoder gains an explicit `with_limits_without_text` constructor. Existing constructors keep their original text behavior. The application still-raster PNG route opts into the new mode because DecodedRaster exposes no textual metadata. png's existing text-skip flag avoids collecting/copying tEXt/zTXt/iTXt; ICC, eXIf and color metadata are not disabled. CRC processing and existing ancillary-CRC policy are unchanged.

A test-only allocator probe compares both constructor policies in the same linked current library on one synthetic16x16 PNG containing4MiB ASCII tEXt. Caller input and1024-byte output are excluded. Old mode: **12610752 requested heap bytes peak /25 allocation calls**. Skip-text mode: **27768 bytes /6 calls**. Both decode every pixel exactly to the same pinned oracle and release all observed private allocations. These are heap allocation requests, not RSS, allocator overhead or latency results. The temporary large source is generated reproducibly rather than committed.

Public regression controls include1MiB tEXt, zTXt and compressed/uncompressed iTXt, matching all pixels and the retained ICC profile to a no-text source. Corrupted IDAT still fails and managed output credit rolls back. Existing EXIF, physical-aspect and color tests also pass. Full public library: **201 passed /5 ignored /zero failures**.

Compressed text is stored compressed by the original dependency until a getter requests decompression; this change must not be described as avoiding automatic huge text inflation. The demonstrated saving is collection/copy allocation for unused metadata, especially uncompressed text.

This reduces a verified source of private memory growth but does not establish complete shared-root accounting of PNG's fixed tables, row/unfiltering buffers, ICC or all codec metadata. No arbitrary reserve or best-effort limit is claimed as a complete memory fix. APNG and other PNG consumers are not switched by this still-raster-only routing change.

Current viewer gate: **285 passed /80 ignored /zero failures** (`viewer.log`).
