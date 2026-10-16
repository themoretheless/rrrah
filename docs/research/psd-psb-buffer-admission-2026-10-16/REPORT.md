# PSD/PSB composition buffer admission

Reserve channel-plane storage before allocation and RGBA interleave output before construction when DecodeRequest supplies a memory budget. Transfer output credit into managed pixel ownership; scratch credit remains until channel storage is destroyed.32-bit ZIP prediction reuses a single reserved row buffer instead of allocating a row clone repeatedly. RLE row lengths are read as borrowed big-endian2/4-byte table entries, removing the heap length vector. Decoding/color/merged-alpha behavior remains unchanged.

Eleven PSD/PSB fixtures exercise raw/RLE/ZIP/prediction,8/16/32-bit samples and merged alpha. Managed output matches baseline sample values (float bit patterns), exact measured peak admission passes,33 zero/output-short/peak-short refusals release all credit, and clones retain output reservation until last owner. Layered59 passed/2ignored; full viewer276 passed/74ignored, including common legacy raster cache/swap qualification. Existing independent fixtures remain in the full suites.

ICC profile copies, parser metadata and zlib internals are not fully budgeted by this change. No standalone end-to-end speed gain or new physical GPU/HDR/NVIDIA qualification is claimed. Source bytes supplied directly to decode are external to the borrowed-buffer test budget.

## Bounded ZIP input and deterministic cancellation

Replaced read::ZlibDecoder.read with a Decompress loop limiting input and output slices to65536 bytes per step. The old output-read boundary could consume a large low-output stream before returning to cancellation checks. Channel storage admission and exact output/trailing-byte checks remain enforced. A no-op progress callback in production allows deterministic checkpoint observation in tests.

An independently authored500017-byte zlib stream contains100000 empty stored deflate blocks followed by six planar samples. It passes the actual PSD decode path with exact expected RGBA. Eight decompressor steps respect input/output bounds. Cancellation after the first consumed step (before any output) returns Cancelled and releases six-byte admitted scratch credit; six prefixes, a trailing byte and output overflow refuse; retry succeeds. Layered60 passed/2ignored; full viewer276 passed/74ignored, including expanded2009-case cache/swap corpus.

This is controlled checkpoint cancellation, not measured native UI scheduling latency. The prior actual Metal2009-case result predates this ZIP loop change; CPU integration is current. Internal zlib allocation remains outside complete memory accounting.

## Borrowed uncompressed channel planes

For compression0, read the original planar samples directly from the borrowed source instead of reserving/copying the whole channel array. Compressed paths retain managed plane scratch. Output interleaving, alpha handling and source immutability are preserved. For a standard RGB input this removes3*width*height*sample_bytes of intermediate allocation; externally held source memory still exists and must be included by file callers.

Six independently authored PSD/PSB version×precision cases verify8/16/32-bit exact channel interleave, u16 adjacent precision, HDR negative/above-one floats, signedzero and NaN payload bits. Exactly output-sized budget succeeds, one-byte-short fails before allocation (peak0), output is managed and final used0. Existing eleven fixture memory cases pass; raw fixture tracked peak is exactly output bytes rather than planes+output. Layered61 passed/2ignored, full viewer276 passed/74ignored including2009-case CPU cache/swap. No wall-clock speedup claim or new actual Metal result for this revision.

## ICC resource ownership admission

Reserve ICC resource bytes before copying (existing4MiB profile cap remains), then transfer credit into DecodedRaster shared color metadata via with_color_profile_reservation. Resource iteration now also checks cancellation. Two authored PSD/PSB wrappers carry the pinned588-byte RGB profile: exact596-byte borrowed decode budget passes, profile-short and total-short fail with final0. Profile bytes/capacity match; clones share metadata and output reservations, retain596bytes until final owner, and try_manage_pixels does not charge already admitted ICC again. Layered62 passed/2ignored; full viewer276 passed/74ignored.

This closes the native PSD/PSB ICC copy/retention credit gap for the supplied budget. It does not claim all parser metadata/zlib allocator accounting or producer/color fidelity from a wrapper fixture. Input source storage is external in these borrowed tests.

## 36MP public file-path raw plane qualification

Added explicitly enabled synthetic6000x6000RGB PSD test through public decode_raster. File generation streams rows to a temporary file; channel values derive independently from x,y modulo251. Every output pixel is checked. Managed source108000040bytes and output144000000bytes produce exact tracked peak252000040bytes, with only output retained on return. Clones retain output credit until last owner. Limit252000039bytes refuses output admission after source reading and returns final0; peak on refusal is source size. The old full plane copy would add108000000bytes. Temporary synthetic file removed after success.

This proves tracked allocation savings for public native decoding, not RSS including allocator/zlib metadata, display preparation, GPU upload or end-to-end viewing speed. The36MP test is ignored by default and was explicitly enabled; default decoder library regression also passed. No production change in this follow-up.

## PackBits input-based cancellation checkpoints

PSD/PSB PackBits previously checked cancellation between rows only. A PSB32-bit row length can describe a large no-output command stream, so a single row could monopolize decoding until its end. Added checkpoints after at most65536input bytes plus one indivisible PackBits command (maximum129bytes consumed per command). A final checkpoint also covers row completion. Production checks the request cancellation token; existing row grammar and expected output checks remain unchanged.

Independently authored PSB contains1000000no-op commands before its first two planar samples. Controlled cancellation on the second checkpoint observes offset65536 and no output, then returns all admitted scratch credit. Full row retry succeeds and actual PSB decoder produces exact authored RGB/alpha at14byte tracked peak (6channel+8RGBA), final0. Layered64 passed/2ignored; full viewer276 passed/75ignored, including current2012-case CPU cache/swap qualification. This is deterministic checkpoint control, not native UI or OS scheduling latency measurement. Source file bytes are external in the borrowed test.

## Refuse unsupported merged layouts before allocating

Moved the existing high-precision/grayscale merged-transparency refusal before channel/RGBA allocation. ICC resource parsing now retains a borrowed slice until layout and compression validation succeed, then reserves/copies exactly as before. Unsupported compression codes also refuse before profile copy. Twenty PSD/PSB×layout×ICC controls return the explicit InvalidPsd diagnostic with budget0/peak0; four ICC-bearing unsupported-compression cases do likewise. Existing supported profile admission/clone tests retain exact behavior. Initial regression fixture accidentally exceeded the separate total-byte cap; changed only its canvas and preserved the failed log.

Layered65 passed/2ignored, focused ICC/compression regression passed, viewer276 passed/76ignored. No16bitmerged-alpha support is claimed: no local ImageMagick producer was available, and no independent16bitproducer pixel fixture was established in this turn. Adobe's primary specification documents merged transparency signaling; it alone does not qualify the reader's white-unmatting behavior across higher precision. Primary references: https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/ and https://github.com/ImageMagick/ImageMagick/blob/main/coders/psd.c .
