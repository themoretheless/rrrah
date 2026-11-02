# TIFF CCITT RLE/Group3/Group4 — 2026-10-24

Native selected TIFF/BigTIFF pages admit Compression3, Group3 1D and mixed1D/2D, unsigned single-channel1-bit strips. The application owns both row state machines, using existing Rust fax0.2.7 public Huffman tables. No production LibTIFF or LibRaw path was added. LibTIFF is used only by the independent test generator. The direct dependency was already present transitively; the existing lock-derived RAW recipe identity was synchronized with Cargo.lock.

## Independent pixel evidence

Original Go x/image bw-gopher_ccittGroup3.tiff:153×55, all Pillow RGBA8 bytes exact. Source/oracle hashes are in tests/fixtures/raster/fax3-go-reference.json; the BSD license is retained in tiff-palette-go-LICENSE.txt.

scripts/generate-tiff-fax3-controls.py uses system LibTIFF4.7.2 to encode and independently read back256 short controls: classic/BigTIFF, LE/BE, FillOrder1/2, T4Options0/1/4/5, orientations1–8,13×7, strips3+3+1.1281D and128 mixed1D/2D sources decode exactly. Pillow12.3.0 additionally verifies192 full images. The64 BigTIFF BE controls use independent LibTIFF scanlines and authored orientation mapping because this Pillow version misdetects the BE BigTIFF marker. Source/oracle hashes are in fax3-libtiff-reference.json.

The same generator with --long-runs encodes32 further controls:2049×11, strips5+5+1, all-white/all-black rows, a half-width region, isolated marks513 pixels apart and a three-pixel black tail. It covers makeup/terminating run codes and reference-line changes across the same container/endian/fill/options matrix. All source bits roundtrip LibTIFF;24 full images additionally match Pillow. The8 BigTIFF BE files use the declared scanline/mathematical oracle. fax3-long-libtiff-reference.json pins all32 sources and one shared RGBA8 oracle, avoiding repeated identical oracle files.

## Fixed failures

valid-source-red.log retains the initial valid-source failure around TIFF strips without trailing EOL/RTC. mixed-valid-source-red.log retains the subsequent mixed-row EOF lookahead failure.

Long controls exposed a further genuine1D failure: a valid aligned strip ended in a short code for three black pixels; the old Group3 decoder returned transitions[2046] instead of the expected width2049 (long-run-valid-source-red.log). The1D route now uses the application's row decoder too. Non-consuming Huffman lookup may zero-extend its window, but consume requires real source bits. The valid short terminal code decodes without adding input, padding pixels or accepting unfinished runs. Next rows still require a complete EOL; final strips need no trailing EOL.

## Memory, cancellation and color

Directory arrays, strip extents and initial EOL are borrowed and preflighted before output allocation. Output and conservative scratch quotas are admitted before allocation. Per-row input, operations and transition counts are bounded; mixed rows reserve additional space for two transition vectors. Reference lookup uses binary search. Cancellation is checked during bit access, output writing and the error boundary, preserving typed cancellation.

The original Go borrowed-source test succeeds at59260 quota bytes (33660 output +25600 scratch), refuses one byte less and releases all quota on drop. The2D exact-peak/one-byte-short checks also release all quota. These logical reservations exclude caller-owned input and are not measured RSS or allocator usage. Truncated strip, bad offset, zero-length-run work limit, inverted photometric interpretation, short EOL, first-row2D, incomplete/over-wide runs, physical-EOF consumption, cancellation and unsupported uncompressed-option controls pass.

ICC is retained. Strict preparation refuses untagged input; explicit sRGB is required for viewing. Integer TIFF does not implicitly become linear.

## Current checks

Decoder249 passed/8ignored/0failed (decode-current.log), covering433 encoded controls plus two original Go sources. App329 passed/84ignored/0failed (shared ../prefetch-count-swap-hit-2026-10-16/app-current.log).

Required offscreen Apple M4 Max Metal TIFF suite6 passed/0ignored/0failed (metal-current.log).69 CCITT RLE/Fax3/Fax4 sources cover all eight orientations,1D/2D, classic/BigTIFF, LE/BE, FillOrder1/2, long runs and both original Go sources. Native bytes and independently constructed linear endpoint frames match exact SDR/float16 HDR attachments. Pin/lease eviction, disk swap, full-root refusal, cancellation after restore admission and retry preserve prepared float bits and exact before/after attachments; final managed root0, swap errors0.

RAW library after the earlier lock identity update:440 passed/59ignored/0failed (raw-current.log). Latest post-fix external1024-source collection passed: all929 previously accepted source/pixel digest pairs unchanged,696 nearest-navigation hits,1390 far byte refusals,59 TTL restores, final managed root0. External collection transport evidence is pinned in collection-current.json; repeated per-source/navigation log markers are omitted from collection-current.log. It is same-decoder regression evidence, not an independent appearance oracle.

## Remaining scope

Uncompressed extension mode, widths above65535, EOL fill longer than18 leading zero bits, broader original producer/compression/color combinations and full TIFF/100-format qualification remain open. The fixed matrices do not establish universal malformed-input coverage. Offscreen float16 attachments do not prove physical HDR display. NVIDIA/CUDA remains deferred by the user. The correctness suites do not measure opening speed; the isolated bit-window microbenchmark below has a narrower scope. Coding semantics follow [ITU-T T.4,4.2](https://www.itu.int/rec/dologin_pub.asp?id=T-REC-T.4-199303-S%21%21PDF-E&lang=e&type=items).

## Bit-window optimization

The bit reader now extracts up to16 bits from a three-byte window and masks all bits beyond the admitted row/physical end. FillOrder2 reverses source bytes before extraction. consume checks real extent and cancellation without performing a second lookup. An independent bit-by-bit reference compares all byte alignments, lengths0–16, five deterministic byte patterns, both bit orders and0–17 remaining bits, including physical EOF. A cancellation control invalidates the generation between successful peek and consume, requires refusal without moving the bit position, and retains the typed cancellation result. Existing codeword/EOF, budget and source/oracle controls pass unchanged.

On Apple M4 Max with rustc stable1.98.1, the ignored bit_window_alternating_microbenchmark runs eight alternating-order pairs,1048576 queries per variant per pair, identical checksum4294377472. Median candidate/reference elapsed ratio:0.387437 (range0.381495–0.388814); raw pairs are retained in bit-window-bench.log. This isolates peek, using the same null cancellation policy and query distribution. It does not measure complete decoding, allocation, disk I/O, viewer latency or FPS, and gives no whole-application speed claim. Reproduce with cargo +stable test -p rrrah-decode --lib bit_window_alternating_microbenchmark --locked --offline -- --ignored --nocapture --test-threads=1.

## Application-owned Group4 route — 2026-10-24

Compression4 now uses the same bounded application-owned transition engine in T.6 mode: no Group3 EOL/tag bits, initial all-white reference per strip, strict pass/horizontal/vertical bounds and an additional two-row scratch quota. The common TIFF directory/strip preflight, page/orientation handling, photometric mapping, ICC/explicit-sRGB policy, row/input work limits and cancellation apply. This replaces the generic TIFF Group4 fallback for admitted single-channel1-bit sources; it adds no C codec dependency. T6Options293 defaults to0; uncompressed/unknown options are refused. Flag definitions are independently visible in [LibTIFF tiff.h](https://github.com/vadz/libtiff/blob/master/libtiff/tiff.h).

The unmodified original Go bw-gopher_ccittGroup4.tiff matches Pillow at every153×55 RGBA8 pixel. Its manifest fax4-go-reference.json reuses the independently verified identical Group3 image oracle; the existing Go BSD license covers the source. Generator flags --group4 and --group4 --long-runs add64 short13×7 sources and8 long2049×11 sources. They cover classic/BigTIFF, LE/BE, FillOrder1/2, all eight short-source orientations and multi-strip resets. All72 roundtrip LibTIFF scanlines;54 also match full Pillow images. Eighteen BigTIFF BE cases remain explicit scanline/mathematical orientation controls. Manifests fax4-libtiff-reference.json and fax4-long-libtiff-reference.json reuse existing identical RGBA8 oracle files, with no duplicate image blobs. Generate the Group3 controls first to provide these shared oracles.

Typed unsupported-option refusal occurs before allocation, exact-peak admission succeeds, one byte less refuses, premature T.6 EOFB before the first row cannot publish partial pixels, and cancellation releases all managed credit. The actual M4 Max suite adds17 Group4 source/oracle/strict-color/RAM/swap/pressure/cancel/retry lifecycles; all native float bits and SDR/HDR attachments remain exact, final managed root0. This is a finite matrix, not universal Group4/original-producer qualification; uncompressed extensions and widths above65535 remain outside this row engine.

First full app run had328 pass/1fail: the empty-folder animation test exceeded its scanner deadline (group4-app-scan-timeout-red.log). Its fixture lived directly under the shared system temporary directory, so sibling-cover discovery examined unrelated temporary directories. A standalone repeat passed. The fixture now creates an empty child inside an isolated temporary parent, preserving all timer/cancellation/empty-gallery assertions and the original5-second deadline. Full app then329 passed/84ignored/0failed. This fixture correction changes no production folder-scanning behavior and does not establish the sole cause of the earlier timeout.

Latest post-RLE external1024-source navigation qualification passed. All929 previously accepted source/pixel digest pairs are unchanged,696 nearest hits,1390 far-byte refusals,59 TTL restores, final managed root0. collection-current.json pins the full report retained outside the repository. Source/pixel digests prove transport regression consistency, not independent appearance.

## Monotonic reference cursor — 2026-10-24

The2D row engine now maintains the lower-bound reference cursor as a0 advances monotonically; it adjusts edge parity separately for the requested color. This replaces a fresh binary search for every mode word. Strict pass/horizontal/vertical bounds already ensure a0 cannot decrease. Each reference edge is skipped at most once per row, and long scans poll cancellation every4096 skipped edges. Group3 mixed and Group4 share this path. The old binary-search selection remains a test-only constant-generic control; normal decoding instantiates the cursor path.

A new LibTIFF/Pillow-verified1025×129 alternating-pixel source stresses dense reference transitions and sign changes in vertical offsets. The --group4 --dense generator emits one encoded source and one shared RGBA8 oracle; fax4-dense-libtiff-reference.json pins them. Candidate/control transitions and consumed bit position match on every row; full public native pixels additionally match the independent oracle. Its actual M4 Max RAM/swap/pressure/cancel/retry lifecycle adds the53rd Fax3/Fax4 GPU source, with exact SDR/HDR and final0. Full decoder249 passed/8ignored/0failed; required Metal TIFF suite6 passed/0ignored/0failed.

The same frozen input/binary contains both constant-generic strategies. Eight alternating-order pairs of16 repeats run separately for the row engine and complete borrowed-source native TIFF decoding. The latter includes directory/strip checks, pixel allocation, quota and RGBA materialization; it excludes source-file I/O, display color preparation, GPU and viewer presentation. Each variant has identical checked checksum; every complete decode releases managed credit. The source/oracle test verifies full pixels independently rather than treating the timing checksum as a pixel oracle.

Apple M4 Max, rustc stable1.98.1, optimized test profile (workspace dev opt-level1, dependency override3), raw pairs in reference-cursor-bench.log. Row-engine candidate/control median ratio0.280182 (range0.275023–0.283026). Complete native TIFF median paired ratio0.399058 (range0.383988–0.403905); median per-image elapsed2.035012ms candidate versus5.124863ms control. These numbers describe this dense source and profile, not universal format performance, release build timing, disk opening or FPS. Reproduce both measurements with cargo +stable test -p rrrah-decode --lib group4 --locked --offline -- --ignored --nocapture --test-threads=1.

## Baseline CCITT Modified Huffman RLE (Compression2) — 2026-10-24

The first valid LibTIFF RLE control failed through the public API with unsupported Huffman compression (rle-valid-source-red.log). Compression2 now uses the application-owned bounded1D run engine, with byte-aligned row starts, no EOL/RTC, exact declared row width and ignored final-byte padding. An explicit Encoding enum separates RLE, Group3 one-dimensional/mixed and Group4 states. Existing strip bounds/quota/cancellation, orientation1–8, ICC/strict untagged policy and explicit sRGB preparation are shared. These rules follow [TIFF6, Section10](https://www.itu.int/itudoc/itu-t/com16/tiff-fx/docs/tiff6.pdf).

Generator flags --group2 and --group2 --long-runs produce64 short and8 long independent LibTIFF encodings: classic/BigTIFF, LE/BE, FillOrder1/2, all short-source orientations and three-strip layouts. All encoded bits roundtrip LibTIFF;54 full images additionally agree with Pillow. Eighteen BigTIFF BE cases use declared scanline/mathematical orientation controls. fax2-libtiff-reference.json and fax2-long-libtiff-reference.json reuse existing identical RGBA8 oracles rather than copying them. These are independently encoded authored controls, not original camera/scanner-producer files.

A separate authored bitstream proves two differently colored rows with nonzero ignored padding, exact byte positions and no synthetic source consumption. Incomplete and over-wide runs fail. A derived Photometric1 source reverses every RGB byte while retaining alpha; exact quota succeeds, one-byte-short refuses, truncated/empty strip guards and already-cancelled request release all credit. Public decoding of all72 RLE sources matches full independent pixels.

Required actual M4 Max TIFF suite6/6 now includes16 RLE sources through strict untagged refusal/explicit sRGB, pin/lease RAM eviction, bounded swap, full-root refusal, cancel-after-admission and retry. Native/prepared bits and before/after SDR/HDR attachments are exact, final0. Full decoder249 passed/8ignored/0failed; app329 passed/84ignored/0failed. The prior2D cursor measurements were repeated after introducing the encoding enum: both row-engine and full-native controls retain the measured improvement, with current raw pairs in reference-cursor-bench.log.

At the Compression2 milestone, word-aligned RLEW (32771), tiled CCITT, row widths above65535, broader original-producer/color combinations and complete TIFF/100-format qualification remain open. This change claims Compression2 support for the admitted strip subset, not full TIFF conformance, physical HDR or NVIDIA qualification.

Post-RLE collection recheck: all929 prior accepted source/pixel digests remain identical, current accepted929, new accepted0, managed root final0. Current compact summary replaces the previous equivalent summary; full rows remain pinned outside Git.


## Word-aligned CCITT RLEW strips (32771) — 2026-10-24

The public TIFF path reproduced unsupported compression32771 before this change (rlew-valid-source-red.log). The owned Rust row engine now aligns RLEW rows to16 bits relative to each strip, consumes only physical padding bits, accepts arbitrary padding values and resets at every strip. Byte-aligned Compression2 behavior remains separate. The boundary rules follow [LibTIFF codec documentation](https://libtiff.gitlab.io/libtiff/functions/libtiff.html) and the [RLEW implementation](https://gitlab.com/libtiff/libtiff/-/blob/master/libtiff/tif_fax3.c). No production C codec was added.

The generator's --group2w flag produces64 short and8 long sources across classic/BigTIFF, LE/BE, FillOrder1/2, all short orientations, and multi-strip layouts. Each row is independently encoded and decoded through LibTIFF Compression2, then padded to a16-bit boundary and assembled through the raw-strip API. Authored geometry supplies the full-image RGBA oracle, shared with existing controls. All72 public Rust outputs match every expected pixel. The independent one-row check does not qualify full-strip RLEW decoding.

Important negative evidence: system LibTIFF4.7.2 RLEW encoding/readback initially disagreed with authored source pixels. Even explicit padded raw-strip assembly disagrees with its RLEW full-strip decoder on all72 files. The generator records mismatched pixel counts in each manifest and retains diagnostic output in rlew-reference-disagreement.log. Its RLEW/Pillow path is not treated as a successful independent full-image oracle. These remain independently encoded rows plus authored container/alignment controls, not external scanner originals or universal RLEW qualification.

A separate authored white8/black8 stream exercises nonzero ignored padding, exact16/32-bit cursor positions, both FillOrders, missing physical padding refusal and strip restart. Exact-quota/one-byte-short, inverse Photometric1, truncated/empty strips and pre-cancellation now exercise both byte- and word-aligned sources with final managed0. Full decoder250 passed/8ignored/0failed (decode-current.log).

Actual Metal Apple M4 Max TIFF suite6 passed/0ignored/0failed (metal-current.log), including16 new RLEW endpoints. The shared lifecycle checks strict untagged refusal/explicit sRGB, RAM pin/lease and eviction, bounded swap, root-pressure refusal, cancellation and retry, exact payload/SDR/HDR and final0. This is offscreen transport; physical HDR, NVIDIA, external RLEW originals, tiled CCITT and width above65535 remain open. No performance gain is claimed.


## Full-image RLEW oracle boundary — 2026-10-24

Added eight one-row-per-strip controls with --group2w --single-row-strips, covering classic/BigTIFF, LE/BE and FillOrder1/2. Every source pixel now roundtrips the full LibTIFF RLEW file; six classic/BigTIFF LE cases additionally match Pillow, with the established BigTIFF BE Pillow marker exception kept explicit. Existing multi-row controls and their disagreement are retained. The new sources reuse the same authored appearance oracle, not another rrrah decoder's output.

The separate verify-tiff-rlew-imageio.py invokes system ImageIO through /usr/bin/sips, checks pinned source/oracle hashes, converts full images and compares every RGBA byte and dimensions. On macOS26.7.2/sips316, all8 one-row-strip files are exact. All72 prior multi-row files disagree (none refused), matching the unresolved scope boundary; representative differing byte counts and manifest hashes are recorded in rlew-imageio-current.json. ImageIO is a separate system invocation, but its underlying TIFF codec ancestry is unproven; do not count it as a proven third independent engine. FFmpeg also refused32771 and no Java runtime was available; neither supplies positive evidence.

All80 RLEW fixtures match native Rust output; the full decoder suite remains250 passed/8ignored. Required Apple M4 Max TIFF suite6 passed/0ignored now includes all8 positive full-image-reference sources in addition to the16 multi-row endpoints: exact native/prepared pixels, RAM pin/lease, pressure refusal, swap, cancellation/retry, SDR/HDR and final0. Current successful logs replace their previous equivalent versions. No production path changed in this follow-up.

This strengthens the verified one-row-strip subset. Multi-row independence, external producer originals, full TIFF/100-format qualification and physical HDR/NVIDIA remain open; the full RLEW row is not qualified.


## Native encoder equivalence for every RLEW strip — 2026-10-24

The new reproducible scripts/audit-tiff-rlew-native-encoder.py writes each authored image through the unmodified system LibTIFF4.7.2 native Compression32771 encoder, with matching TIFF/BigTIFF byte order, FillOrder, orientation and rows-per-strip. It then reads all raw compressed strips from both files through TIFFReadRawStrip and compares every byte. Expected image patterns are authored independently; no rrrah decode output is used. Temporary native encodings are removed instead of duplicating fixtures.

All80 source files are exact:64 short multi-row sources/192 strips,8 long multi-row sources/24 strips,8 one-row-strip sources/56 strips. All272 strip lengths and payloads match without trimming, padding edits or exceptions. Compact evidence is rlew-native-encoder-current.json, pinned to all three fixture manifests and the external library version. Reproduce with python3 scripts/audit-tiff-rlew-native-encoder.py.

This rules out our manual row/strip assembly as the source of the previous discrepancy for these controls: the independent library's own full-image encoder emits identical compressed bytes. Combined with independently roundtripped individual RLE rows, authored all-pixel references and current native Rust/Metal results, it strengthens multi-row strip evidence. The system full-image RLEW decoder still disagrees on72 cases. The word-padding/read-ahead branch in upstream Fax3DecodeRLE is a candidate cause, not a proven root-cause attribution from this byte comparison. No decoder was patched to create a positive reference, no production dependency was introduced, and no performance result is claimed. External scanner originals and broader full-format qualification remain open.


## RLEW decoder word-boundary root cause — 2026-10-24

The system decoder audit is reproduced by scripts/audit-tiff-rlew-reference-decoder.py:0/64 short multi-row,0/8 long multi-row,8/8 one-row-strip images match, retained in rlew-reference-system-current.json. This script only reads retained source bytes and authored pixel oracles; it never calls rrrah.

Built isolated LibTIFF4.7.2 from the official source archive outside the repository. rlew-reference-source-pin.json records URL and original archive/source hashes. rlew-reference-trace.patch adds only before/after logging inside Fax3DecodeRLE. The first source's second row ends with input pointer15 bytes and9 bits unread: logical consumed position111bits. Correct16-bit alignment needs one padding bit, ending at112. The original branch clears9bits and advances the odd pointer to16, ending at128: it skips16 genuine bits of the next row. Both minimal traces are retained in rlew-word-boundary-trace.log.

The isolated rlew-reference-word-alignment.patch computes padding using both unread-bit count and pointer byte parity. The same row then retains8 read-ahead bits at pointer15, giving the correct logical boundary112. All80 full images become exact in rlew-reference-patched-current.json (64 short,8 long,8 single-row). This controlled change isolates the observed decoder defect for this matrix. The modified reference is diagnostic evidence, not an unmodified independent full-image oracle, and not a proposed production dependency. Existing exact native encoder/all272-strip equality and authored full-image/native Rust/Metal results remain separate evidence.

Reproduce externally: extract the pinned archive; optionally apply the trace patch with patch -p1; configure CMake with -Dtiff-tools=OFF -Dtiff-tests=OFF -Dtiff-contrib=OFF -Dtiff-docs=OFF -DBUILD_SHARED_LIBS=ON; build target tiff. Invoke the read-only audit with --library pointing to build/libtiff/libtiff.dylib and `--first 1`. Apply the word-alignment patch and rebuild for the diagnostic corrected comparison. Source patches affect only external LibTIFF test code; rrrah source/dependencies are unchanged in this step.

This resolves the previously unproven cause of the72-source discrepancy. External scanner-produced originals and broader TIFF variants are still required; no full100-format, physical-display or NVIDIA qualification follows from this controlled matrix.
