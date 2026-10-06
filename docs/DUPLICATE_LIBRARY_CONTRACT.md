# Duplicate search library acceptance contract

The requested library covers both byte-identical files and visual image matches.
Exact file identity, normalized-pixel equality, region containment and visual
similarity are separate evidence classes. A score never proves identity. The
library reports results and errors and never deletes files.

Completion requires every row below, including independent positive and negative
fixtures. Partial coverage is not silently promoted to general support.

| Requirement | Current evidence | Remaining qualification/work |
| --- | --- | --- |
| Renamed copies, arbitrary types, empty files | `exact_files.rs`: streaming BLAKE3 and full byte confirmation | Broader filesystem/platform corpus; injected sample/full collisions now pass byte-confirmed partitioning |
| Same size, different contents | Equal-size and equal-edge/different-middle negatives | Injected constant sample/full digest fixture passes; wider platform qualification remains |
| Repeated paths, symlinks, hardlinks | Unix overlap, aliases and followed-cycle fixtures | Windows volume/file-index implementation cross-checks; native Windows identity/hardlinks/replacement tests and other non-Unix targets remain pending |
| Missing, unreadable, changing files | Per-file errors, snapshot validation, equal-length mutation, decode-time mutation | Representative attribution/retry and FIFO replacement pass; non-UTF8 diagnostics pass on macOS; current Linux non-root standalone exact-source tests qualify non-UTF8 files and read denial; native decoder and non-Unix coverage remain |
| Cancellation and large files | Bounded exact reads, index-node/bucket cancellation, explicit resource errors | Core/ICC conversion cancellation checkpoints pass; indivisible ICC setup, underlying decoder latency and large-file measurements remain |
| Recursive directories and overlapping roots | Deterministic traversal, configurable symlink/depth/entry policy; authored 192-file nested exact-search corpus matches all 48 three-copy groups on macOS and Linux, with 48 equal-size negatives, overlapping roots, reversed order, limit diagnostics and cancellation/retry | scan_visual_roots integrates bounded discovery, aliases and diagnostics; broader symlink/platform and visual traversal corpus remains |
| Same pixels, different encoding/metadata | Canonical RGBA8/linear HDR equality; BMP metadata and TIFF lossless-compression matrix | confirm_pixels/confirm_pixel_groups integrated with fresh source validation; broader independently encoded cross-format corpus remains |
| RAW versus developed raster | Native full development; independent full-resolution DNG/NEF/ORF TIFF pairs pass strict spatial geometry/pixels; one unrelated RAW/TIFF pair rejects | Strict CR2 render still rejects; explicit display-projection mode accepts the known four-camera RAW/TIFF collection with 24 cross-scene negatives rejected. Broader camera/development/color and held-out/burst/look-alike qualification remain |
| Resize/recompression/exposure/color changes | Explicit filtered policies accept 20/20 real preview positives at radius 3 in both linear and encoded sRGB; strict residuals retained | Held-out calibration, real bursts/look-alikes, scalable collection search and confidence evidence remain |
| EXIF orientation/rotations/reflections | D4 fingerprint/index oracle; decoder orientation path | Eight-orientation independent TIFF matrix passes; other formats and broader orientation corpus pending |
| Crops/borders/watermarks/edits | Exact unscaled HDR/alpha region containment, explicit coordinates and budgets | Native translation-local corner/BRIEF extraction, mutual matching, geometry and bidirectional pixels tested together; dyadic scale pyramid and quarter turns tested; arbitrary-angle/continuous-scale and real edited-crop corpus pending |
| Uniform images and burst negatives | Information gates, informative checker collision and separate exact selected-frame discovery, including uniform PNG/BMP | Real captured uniform/burst negatives and semantic false-positive calibration |
| Transparency/HDR/color profiles | Canonical invisible RGB, signed-zero normalization, HDR precision, per-page TIFF ICC; independent LittleCMS normal/boundary grids expose and qualify single-gamma zero-endpoint correction | Broad ICC/alpha/HDR qualification, cross-profile precision policy and untagged color policy |
| Animation and multipage images | All composited GIF/APNG/WebP frames, exact timeline/repetition; DCX/ICO/CUR/TIFF pages and selected-page ICC | Timeline splits/zero-duration prefixes pass across GIF/APNG/WebP; full-container bounded batch/exhaustive search integrated; broad disposal/container corpus and scale performance remain |
| Unsupported/corrupt images | Exact scan independent of image decoding; explicit visual errors | Combined recursive directory/visual report integrated; broader malformed format corpus remains |
| Grouping | Complete-link partition tested on all 64 four-node graphs; all accepted edges retained | Pixel-confirmation groups now revalidate batch-wide source snapshots and remove changed-source edges; complete-container grouped scanner integrated; recursive complete-presentation grouping integrated with explicit per-path modes; automatic content-based mode detection/recursive grouping integrated; broader concurrent-mutation corpus remains |
| Repeated scans | Content/recipe/frame-keyed bounded checksummed cache, atomic save, per-file and batch reuse; mutation rejected before admission | Automatic complete decoder/recipe identity; broader platform/power-loss cache qualification (macOS process publication/exit fixtures below) |
| Resource budgets and scale | Byte/pixel/frame/cache/work/result budgets; streaming source reads and metric indexes | Indexed construction limits/cancellation details, decoder allocation completeness, measured scale/latency/memory |

## Current completion audit

Color-policy fixture corrections pass all 14 `decode` and five
`raster_equality` integration tests on native Linux with tracked sources
unchanged at terminal audit. Standard PPM is qualified as BT.709; strict
unknown-color refusals use genuinely untagged formats. This resolves the two
stale-fixture failures preserved in older isolated snapshots. A new isolated
all-feature/all-target macOS run passes all 340 tests across 55 groups, with
zero failures/ignored tests and all 2345 snapshot files unchanged at terminal
audit. That snapshot includes pyramid file/index/root integration and the color
fixture corrections; it precedes the probe's nine-million hit cap and the newer
complementary file API. Evidence:
`research/dedup-color-policy-refresh-linux-state.json` and
`research/dedup-atomic-pyramid-full-state.json`.

The managed oriented scale-pyramid primitive and its public photometric file
API now pass macOS targeted gates for retained feature ownership, admission,
cancellation, retry, identity and an unrelated-scene refusal. The public API
also passed an earlier 25-test macOS `projective_files` regression, including
existing collection and registration regression gates. This run uses the live
workspace with a private target directory, not an atomic source snapshot. It
also recovers Copydays strong query `200001.jpg` against its independent origin
with fixed three-level extraction, seeded projective geometry and constrained
color fitting, without supplied geometry or registration refinement. Fitted
pixel agreement is 24037/25546 forward and 84801/92220 reverse; managed
reservations return to zero. The complete strong-subset run finishes with all
229 pairs executed, 20 accepted and 209 omitted; all 386 normalized inputs pass
integrity checks. This exposes substantial remaining copy-recovery gaps.
The recovered print/scan query rejects all 156 different original-origin groups;
four targeted Linux pyramid/file/collection gates pass. Of the 209 omissions, 77 lack a geometry model and
132 fail pixel admission. This finite measurement is not
a general copy-recall or semantic-precision certificate. Managed pyramid
collection and recursive-root APIs are now implemented through the shared
source lifecycle. A three-file authored macOS gate passes direct-pair equality,
unrelated rejection, overlapping nested roots, reversed order, cancellation,
retry, invalid policy and pair limits. The same gate passes on native Linux.
All 54 current macOS `local_collection`, `local_scan` and `projective_files`
integration tests pass after the shared extractor change; all tracked sources
were unchanged at terminal audit. The isolated full regression snapshot also
passes as described above; independent all-229 collection results follow below.
Evidence: `research/dedup-copydays-public-pyramid-first-pair.json` and
`research/dedup-pyramid-file-api-state.json`,
`research/dedup-copydays-public-pyramid-state.json`.
Collection evidence: `research/dedup-pyramid-collection-state.json`.
The first independent indexed probe run exposes an explicit hit-budget refusal
for `207502.jpg`: its two-million hit admission omitted same-file retrieval
cost. With at most 3000 total features, accumulated distinct-ID hits are bounded
by 3000 squared (nine million). The probe cap is corrected with acceptance
unchanged. The original all-229 run is preserved: 228 successful outcomes match
direct candidate decisions and one is an explicit budget refusal. Corrected
targeted recovery of `207502.jpg` passes with identical native geometry/pixels.
The corrected all-229 indexed run completes without errors: all 229 candidate
decisions and all 152 geometry-bearing native results equal direct comparison,
with 20 copies recovered. All 386 normalized inputs pass integrity checks.
Evidence:
`research/dedup-pyramid-collection-budget-diagnosis.json` and
`research/dedup-copydays-pyramid-collection-original-comparison.json`,
`research/dedup-copydays-pyramid-collection-budget-state.json`.

The ordinary anchored/unanchored registration portfolio has also completed all
229 Copydays strong pairs: 23 are accepted. Paired fixed-policy comparison with
the pyramid accepts 17 through both methods, six only through registration and
three only through the pyramid (26 in their diagnostic union). Both source
manifests and all 386 normalized images are verified. This is evidence for
complementary searches, not an already-qualified combined file/index API or
full copy coverage. Evidence:
`research/dedup-copydays-pyramid-portfolio-comparison.json`.

A new public complementary file API executes both searches on shared decoded
views under explicit cumulative comparison, hypothesis and sample-pair caps.
Its macOS authored gate passes identity with both search results, unrelated
rejection, all cumulative refusals before I/O, incompatible decode policy,
overflow, phase work refusal after a successful pyramid search, memory refusal,
cancellation, source mutation and retry. Independent print/scan query `200001`
retains the same pyramid geometry/pixels with registration rejected. The new
API's earlier 27-test macOS regression and native Linux file gate have complete
passing test logs; their process handles subsequently disappeared, so process
exit codes are not claimed. The recovered print/scan query rejects all156
different publisher-origin groups. The all229 positive run stopped after78
completed pairs with no process left and resumed after validating pinned
probe/manifest, ordered prefix outcomes and all prefix input hashes. It now
completes all229 pairs successfully:26 copies recovered,203 omitted. All
reported per-family geometry, pixel counts, inlier/correspondence counts and
decisions exactly match the standalone measurements; all386 inputs revalidate.
Complementary indexed/root APIs now use merged descriptors only for proposals,
followed by original independent matching/verification. Their authored macOS
gate passes direct/index equality, unrelated rejection, nested overlapping
roots, reversed order, cancellation/retry and phase-budget attribution. Three
native Linux gates pass. Redundant collection checks were consolidated into
the stronger source-mutation/request-lifecycle test with an explicit physical
path edge assertion; that consolidated gate passes on macOS. Independent
production collection qualification remains pending.
Evidence: `research/dedup-complementary-files-macos.log` and
`research/dedup-copydays-complementary-first-pair.json`,
`research/dedup-copydays-complementary-negative-state.json`,
`research/dedup-complementary-collection-state.json`,
`research/dedup-copydays-complementary-state.json`.
Checkpoint continuation has a separate fake-probe protocol test: completed
prefixes are preserved, only remaining pairs execute, completed runs revalidate
without new child calls, and probe/manifest/mode/order/count mismatches refuse
before child execution. This is protocol evidence, not image recognition.
Evidence: `research/dedup-copydays-resume-protocol-adversaries.json`.

Nested recursive visual discovery now has a six-source, 19-file authored gate:
three copies of each source in separate nested branches plus one corrupt file.
All 18 expected copy edges, exclusion of cross-source edges, corrupt-file
attribution, overlapping roots, reversed order and a repeated cache-backed scan
pass on macOS and native Linux. All seven current `scan.rs` integration tests also pass on macOS.
This finite source cohort
does not establish general semantic precision or all traversal/platform cases.
Evidence: `research/dedup-nested-visual-state.json`.

A new six-source preview cohort (36 independently Pillow-generated files)
is evaluated with unchanged existing local/photometric thresholds and radius-2
filter. macOS exact comparison passes six self checks and 15 distinct-capture
negatives; all 14 unrelated visual scene pairs reject. Two related garden views
are explicitly excluded from unrelated-scene labels. All 30 transformed-positive cases (resize, crop, 17-degree rotation, encoded
brightness and JPEG recompression) pass on macOS with terminal exit 0 for the
three-test suite. A separate 12-file crop collection test also passes: indexed
results equal all 66 direct comparisons, six same-source crop edges are
recovered and unrelated scene edges are absent. Native Linux direct photo
qualification passes all four tests (30 positives, 14 unrelated negatives, exact
distinctions and related-view observation). The newer indexed collection test
also passes separately on Linux with terminal exit 0. This is an additional
cohort, not a universal semantic/burst calibration. Evidence:
`research/dedup-heldout-photo-audit.json`.

Current independent RAW qualification passes all 29 required files with terminal
exit 0: 27 pinned public cases plus two existing owner EOS R8 files using the
rebuilt native dumper and a 512 MiB managed budget. Missing public sources are
acquired into a durable task corpus and verified against manifest SHA-256
before decoding. Every case is required; the script does not skip or regenerate
independent LibRaw sample/color references. Sensor samples, CFA, WB and matrix
checks do not establish photographic rendered-color fidelity or held-out visual
search calibration. Evidence: `research/dedup-current-independent-raw-state.json`.
The subsequent full library development/fingerprint phase passes all 27 public
files and both pinned owner EOS R8 files, with terminal exit 0 for both runs.
It uses a 2 GiB managed budget and 180-second per-file process deadline. Source
hashes/dimensions, native development, finite normalized pixels, exact pixel
digest, visual fingerprints and managed release are checked for each file.
The same 29-file Linux gate now passes too; all normalized pixel digests and
dimensions exactly match macOS. A new same-process zero-budget refusal/retry
qualification now passes all 29 files on macOS with terminal exit 0, preserving
all baseline digests; the native Linux zero-budget run also passes all 29 files
with the same baseline digests after extending the probe; the old full-suite
source snapshot predates this example-only change. Evidence:
`research/dedup-current-raw-cross-platform.json`.
A stronger current macOS probe run additionally tests source+u16-sensor+1MiB
partial admission: it requires nonzero managed peak, explicit resource refusal,
used=0 after failure and successful adequate-budget retry. All 29 cases remain
required on each platform; strengthened qualification passes all 29 cases on
macOS and native Linux. Baseline source hashes, dimensions and normalized
digests agree after retry. See `research/dedup-raw-partial-budget-comparison.json`;
this does not certify total native RSS.
This establishes neither independent rendered-color equality nor held-out
visual-match accuracy.

Current macOS full native decoder unit run passes 773 tests, with 47 explicitly
ignored external/private corpus tests. Three ignored tests are separately executed and pass: EOS R8 allocation
admission/retry/last-owner release; pinned ERF bad-WB and truncation refusal;
pinned 3FR embedded calibration precision and zero-neutral refusal. Every ignored name/reason is retained
in `research/dedup-decoder-full-unit-state.json`; these are not counted as passes.
This does not qualify full camera/color behavior or replace independent RAW
oracle comparisons.

Current macOS native decoder raster_corpus qualification passes all 23 tests:
150 independent raster fixtures validate dimensions and per-channel oracle
bounds, managed output ownership/release; additional tests qualify native
precision, alpha/profile behavior and rejection of truncated containers. All
150 manifest encoded-file SHA-256 values were independently checked. Native
Linux qualification now also passes all 23 tests with terminal exit 0. This does not
run the other 797 decoder unit tests or prove all codec/color combinations.
Evidence: `research/dedup-independent-raster-corpus-state.json` and
`research/dedup-independent-raster-corpus-macos.log`.

WAL fingerprint persistence snapshots/hashes the external game palette and
revalidates it before cache return/admission. Fourteen decode tests pass on
macOS and native Linux, including palette change, removal, restoration and reuse.
A separate indexed-search regression reproduced stale pair decisions after a
late palette mutation. DecodeSourceSnapshot now retains and revalidates the
main source and palette across decoder, indexed pixel search, pair/group scan,
local scan/collection and prepared container paths. The indexed regression and
61 macOS tests across five affected integration suites pass after this change.
A separate direct selected-frame presentation test also passes on macOS:
palette mutation returns a source-change error, releases managed resources and
restored palette permits retry. Its native Linux qualification and the current
full-suite runs remain active. The macOS presentation suite also passes late external-palette mutation in
both exhaustive and indexed complete-link grouping: stale edges are removed,
the healthy pair remains and restoration recovers all three edges. The same
test covers dependency deletion in both modes, requires Io(NotFound) attribution
to the affected source and verifies restored retry. Two further cases atomically
replace the palette with a distinct file containing identical bytes; both search
modes report Changed, remove the stale source edges, preserve the healthy pair
and recover all edges on a fresh scan (macOS four-test suite passes). The WAL cache cancellation regression additionally sweeps all 48 observed
cold-cache and 22 warm-cache callback checkpoints on macOS. Every one-shot
cancellation reports cancellation, releases managed resources, preserves cache
admission invariants and permits retry. This does not qualify indivisible native
decoder latency or external process cancellation. Native Linux
qualification of all four WAL tests now passes with terminal exit 0, covering
the same direct/group mutation/removal/replacement, cancellation and byte-limit
cases. Wider mutation corpora and concurrent ABA remain incomplete. Evidence:
`research/dedup-wal-palette-cache-audit.json`.

HEVC decoder inventory/version now enters BMFF fingerprint recipes and is
revalidated before cache return and admission. Native Linux qualification shows
plugin availability: present misses and equals fresh, absent reports UnsupportedCodec
without cached evidence, restored hits and equals fresh. Twelve macOS decode tests
pass. See `research/dedup-native-resource-cache-audit.json`. This is capability
identity, not a digest of native library/plugin bytes; same-version replacements,
other native codecs and full resource qualification remain incomplete.

SVG fingerprint persistence now includes the immutable font snapshot used by the
actual renderer: font bytes, face metadata/order and generic-family choices.
Native Linux two-process qualification reproduces stale reuse before the fix;
after the fix, a changed font set misses and equals fresh decoding, while an
unchanged font set hits and equals fresh decoding. Twelve macOS decode integration
tests pass. See `research/dedup-runtime-font-cache-audit.json`. Complete native
library identity, font discovery allocation/cancellation and broader font corpus
qualification remain incomplete.

After the SVG font, HEVC capability, extension-routing and shared WAL dependency
changes, the macOS full rrrah-dedup all-feature/all-target run passes 288 tests
with terminal exit 0. No original-snapshot source drift was found. The four-test
WAL dependency suite was added after launch and passes independently; it is not
included in that 288 count. The corresponding native Linux full run also passes 289 tests with terminal
exit 0 and no original-snapshot source drift. Later WAL and new-photo tests
are separately qualified; they are not silently included in the full counts.
Evidence: `research/dedup-shared-dependency-full-state.json` and
`research/dedup-wal-palette-cache-audit.json`. The external palette also respects
the per-source encoded byte limit: a 440-byte WAL with a 1217-byte palette refuses
under a 440-byte limit even with a warm cache; restoring the policy reuses the
valid entry. This does not prove complete native allocation accounting.

Before the SVG font-resource change below, full
`rrrah-dedup --all-features --all-targets` regression runs passed on
macOS (285 tests) and native Linux (286 tests), with terminal exit 0. No
source drift was found against the saved source snapshot. Example targets compile
but their main workloads are not executed by this command. See
`research/dedup-current-full-revalidation-state.json` and
`research/dedup-current-full-linux-revalidation-state.json`. This regression
does not close the remaining qualification in the 18 rows above.

The independent associated-alpha TIFF regression is now fixed by selected-directory
ExtraSamples handling and in-place unassociation before color transfer. Equivalent
8/16/float TIFFs match an independent straight-alpha PNG on macOS and native Linux;
all five raster-equality integration tests pass on both. Fifteen TIFF encodings
cover Classic/BigTIFF and both endian layouts; four invalid alpha/metadata variants
refuse, release managed memory, and recover on same-path retry. A focused unit qualifies
16-bit rounding, unclipped signed HDR colors and invalid alpha/emission refusals.
See `research/dedup-associated-alpha-qualification.json`. Broader malformed metadata,
profiles and full associated-alpha coverage remain incomplete.

Large straight-alpha checks now qualify independent PNG/unassociated-alpha TIFF
at 1024/2048/4096 square sizes. Nine comparisons pass on each platform: TIFF
matches PNG, invisible RGB changes compare equal, and a visible partial-alpha
one-code edit compares different. Zero/one-byte budget refusal and successful
retry release managed reservations. Whole-process peak RSS is 812,253,184 bytes
on macOS and 644,288,512 bytes on Linux. See
`research/dedup-alpha-scale-qualification.json`. Associated-alpha, broad ICC/HDR,
RAW and full allocator qualification remain incomplete.

Large cross-encoding checks now include independently authored uncompressed
RGB TIFF with explicit untagged-sRGB assumption, alongside BMP and PNG at
1024/2048/4096 square sizes. Six cross-encoding direct pixel comparisons and
six one-byte-edit negatives pass on macOS and native Linux; zero/one-byte memory
refusals, successful retry and cache-hit isolation pass for both alternate codecs.
Whole-process peak RSS is 807,272,448 bytes on macOS and
617,648,128 bytes on Linux. See
`research/dedup-large-tiff-qualification.json`. This opaque gradient matrix does
not qualify alpha, ICC/HDR, RAW or all native decoder allocations.

Large cross-encoding selected-frame equality now directly compares BMP/PNG
normalized pixels at 1024/2048/4096 square dimensions and rejects a one-byte
middle-pixel edit at every size. Both macOS and native Linux pass, with managed
reservations released after paired comparisons. Whole-process peak RSS is
806,387,712 bytes on macOS and 616,992,768 bytes on Linux. See
`research/dedup-large-pixel-qualification.json`. This authored two-codec matrix
does not establish broad color-profile, RAW or native-allocation completeness.

Raster decode scale now qualifies authored 24-bit BMPs at 1024/2048/4096 square
pixels on macOS and native Linux: zero/one-byte managed budgets refuse without
cache admission, sufficient-budget retry succeeds and releases reservations,
and a zero-output-budget validated cache hit preserves the fingerprint.
At 4096 square pixels the single decode/fingerprint timing is 357.62 ms on
macOS and 586.70 ms on Linux. Whole-process peak RSS across all fixture generation,
decodes and cache hits is 516,227,072 and 345,366,528 bytes respectively. See
`research/dedup-raster-memory-qualification.json`. This is one BMP path, not total
allocator qualification across codecs or a controlled platform comparison.

Native Linux repeats both large-file probes on isolated tmpfs. Exact scans
confirm copies/reject changed-middle negatives at 1/8/64/256 MiB; the 256 MiB
median is 951.65 ms and process peak RSS is 2,244,608 bytes. All 20 cancellation
runs and retries pass; memory is measured by the process's own /proc VmHWM.
These are not controlled macOS/Linux performance comparisons, and do not
qualify decoder allocations or cold/network storage. The two scale artifacts
below retain separate platform measurements.

Exact-file cancellation scale now checks 1/8/64/256 MiB files, five attempts
per size. Cancellation at callback 25 returns an incomplete report without groups
after 671,744 bytes read; uncancelled retry confirms both copies at each size.
Maximum observed callback-to-return delay on macOS is 3,583 ns. Standalone
whole-process maximum RSS is 2,506,752 bytes. See
`research/dedup-exact-cancel-scale-qualification.json`. This synchronous callback
measurement does not include external cancellation during an in-flight OS read,
network/cold filesystem latency or decoder operations.

Exact-file scale now measures warm local scans of three independently generated
files at 1/8/64/256 MiB each, confirming two copies and rejecting a changed-middle
negative. Three measured runs follow one warmup per size. macOS medians are
3.07/22.97/180.63/706.08 ms; standalone whole-process maximum RSS is 2,572,288
bytes. See `research/dedup-exact-scale-qualification.json`. Cold/network filesystems,
decoder memory and cross-platform scale remain unqualified.

Physical identity now uses Unix device/inode or Windows open-handle volume/file
index; acquisition failures propagate rather than falling back to a path.
Portable fixtures distinguish physical hardlink aliases from byte copies and
reject replacement with identical bytes and restored modification time, then
verify retry. macOS passes 12 library and 15 exact-file tests; native non-root Linux passes
12 library and 16 exact-file tests, including the portable hardlink/replacement
fixtures. Windows focused
cross-check and Clippy pass. A native Windows CI workflow is prepared but has
not executed; runtime Windows behavior remains unqualified. See
`research/dedup-windows-identity-audit.json`.

Complete GIF/APNG/WebP animation cancellation now replays every callback
observed in successful decodes with latched cancellation. Every cancelled
attempt refuses admission and releases managed reservations; retry preserves
the baseline timeline digest. All seven animation integration tests pass on
macOS and native Linux. See `research/dedup-animation-cancellation-qualification.json`.
Indivisible-operation cancellation latency, broad corpus and total RSS remain
unqualified.

GIF/WebP malformed coverage now checks all 3463 truncated prefixes of two
authored animation fixtures, plus trailing data. Each truncation returns typed
decode refusal without retaining managed reservations; same-path restoration
permits decoding again. All six animation integration tests pass on macOS and
native Linux. See `research/dedup-animation-truncation-qualification.json`.
Broader malformed input, allocator completeness and total RSS remain pending.

Malformed APNG coverage now checks every truncated prefix of an independent
full-canvas animation, each chunk CRC, and trailing data. Typed decode refusal
retains no animation; managed reservations return to zero after every failure;
restoring the same path permits successful four-frame retry. All five animation
integration tests pass on macOS and native Linux. See
`research/dedup-animation-malformed-qualification.json`. Other malformed formats,
allocator completeness and total RSS remain unqualified.

GIF disposal modes 1/2/3 now match independently composited Pillow frame
oracles encoded as full-canvas APNG, including all four frame durations and
repetition. Pairwise differing-disposal timelines reject and managed memory
returns to zero. All four animation integration tests pass on macOS and native
Linux. See `research/dedup-disposal-qualification.json`. This tiny authored
corpus does not qualify all animation disposal/container cases.

Indexed recursive memory refusal qualifies explicit/automatic animation roots
with zero/one-byte budgets on macOS and native Linux: every preparation and pair
fails with attributed Memory issues, no false equal/different evidence appears,
and a sufficient-budget automatic retry recovers all labelled edges. macOS
passes all 14 container tests and scoped Clippy. Native Linux passes 28 targeted
tests (14 container, 12 decode, 2 presentation digest) after a terminal SIGKILL,
terminal internal-symbol linker failure and isolation of generated incremental
container-test artifacts. See `research/dedup-recursive-memory-qualification.json`
for logs, source drift and snapshot limits. These results do not qualify full
allocator/RSS completeness or all acceptance requirements.

Actual Linux cache storage refusals now qualify a read-only isolated ext4 bind
and a filled 64 KiB tmpfs. Saves return pre-publication ReadOnlyFilesystem or
StorageFull/ENOSPC, retain the exact prior cache and independently decoded
fingerprint, and remove temporary cache files. Writable remount and filler
removal respectively restore successful publication. The standalone probe and
scoped Clippy pass; see `research/dedup-cache-storage-qualification.json`.
This does not model a full physical disk, concurrent remount, quota, power loss
or kernel crash; the overall acceptance requirements remain incomplete.

Cache publication now qualifies an unwritable parent on macOS and persistent
Linux ext4: PermissionDenied occurs before replacement, previous bytes and all
64 independently labelled writer records remain, no temp files survive, and
retry succeeds after permissions are restored. All seven cache integration
tests pass on each platform, with scoped minimal Clippy passing. The permission
fixture explicitly requires non-root execution rather than silently accepting a
privileged DAC bypass. See `research/dedup-cache-permission-qualification.json`.
ACLs, physical disk-full and power-loss qualification remain pending.
Read-only ext4 and controlled tmpfs ENOSPC are qualified by the storage probe above.

Unix cache publication now synchronizes the parent directory after replacing the
file. `PublicationSync` distinguishes an already published file whose directory
sync failed. The injected-failure fixture independently reads the new complete
publication before returning an error, then verifies retry. Eighteen macOS
minimal library/cache and 43 Linux all-feature library/cache tests pass, plus all
six process/cache integration tests on persistent Linux ext4. Scoped minimal
Clippy passes. See `research/dedup-cache-directory-sync-qualification.json`.
Actual power-loss/kernel-crash durability and non-Unix directory sync remain
unqualified; the full acceptance table is still incomplete.

The complete all-feature/all-target suite now passes 274 tests on Ubuntu
24.04 ARM64 under uid/gid 65534, with none failed, ignored or filtered. This
includes all current native raster, ICC, animation/page, exact, partial-copy,
photographic and recursive/indexed tests. Examples compile without executing
their main routines; doctests are not included. See
`research/dedup-linux-full-qualification.json` for terminal logs, prelaunch
hashes and concurrent drift. Existing decoder/JPEG XR linker warnings remain,
so this is not hosted warnings-as-errors CI proof. Real-camera/full-RAW held-out
corpus, persistent storage, non-Unix and runtime identity requirements remain.

Native decoding now passes 27 targeted integration tests on Ubuntu 24.04 ARM64
under uid/gid 65534: 13 container, 12 decode/cache integration and two complete
presentation digest tests. This includes mixed renamed formats, corruption,
source mutation, cancellation and request-policy cache isolation. Case-sensitive
sysroot libraries are staged independently; CI now explicitly installs zlib1g-dev,
and build identity watches libclang/bindgen inputs. Two host context tests and
scoped Clippy pass. See `research/dedup-linux-native-qualification.json` for
terminal logs, setup details and source drift. Full Linux suites, real-camera
RAW/ICC and persistent filesystem/runtime identity evidence remain incomplete.

The complete minimal-feature all-target suite now passes 110 tests on Linux
ARM64 with Rust 1.98.0 under uid/gid 65534, with none failed, ignored or filtered.
Source is read-only; build and temporary test storage are disposable tmpfs.
See `research/dedup-linux-all-targets-qualification.json`. This extends actual
portable-library coverage beyond macOS, but decode-gated binaries run no tests,
and tmpfs does not qualify persistent filesystem crash durability or non-Unix
identity. Native Linux decoder and remaining acceptance evidence remain pending.

The complete all-feature/all-target invocation after cache source verification,
request-policy isolation and native build-context changes passes 273 tests,
with none failed, ignored or filtered. All example targets compile; their main
routines and doctests are not executed by this invocation. See
`research/dedup-all-targets-current-qualification.json` for terminal evidence,
prelaunch hashes and concurrent source drift. This verifies regression coverage,
not the unresolved real-capture, platform, runtime identity and scale requirements.

Automatic fingerprint build identity now includes native compiler/flags,
SDK/deployment and pkg-config context, including target-specific dash/underscore
variable forms. Absent settings are watched so introducing an override retriggers
identity generation. Canonical context tests verify relevant-input sensitivity,
order independence and exclusion of unrelated environment. Two minimal and 20
integrated tests pass; scoped Clippy passes. See
`research/dedup-native-build-context-qualification.json`. Dynamic library binary
replacement and runtime font/resource identity are still not qualified, so the
complete automatic recipe and overall acceptance remain incomplete.

The cache-hit scale example now explicitly requires `decode`, preserving the
minimal library build. The full no-default-feature all-target invocation passes
107 tests with none failed, ignored or filtered, and compiles the available exact
and visual scale examples. Decode-gated binaries intentionally execute zero
tests in this configuration; example mains are not executed by this command.
See `research/dedup-minimal-all-targets-qualification.json`. External native
library/build identity and the overall acceptance contract remain incomplete.

Cached request-policy isolation now qualifies strict/permissive untagged PPM
color admission, separate linear-assumption entries, all three TIFF page keys,
stricter directory-budget refusal after caching, and out-of-range selection.
Independent TIFF page fingerprints differ pairwise; repeated selection hits only
its own entry. All 18 decode/cache tests and scoped Clippy pass. See
`research/dedup-cache-policy-isolation-qualification.json`. Broader decoder/color
build identity and full acceptance qualification remain unfinished.

Current verified cache-hit source cost is measured on warm local 1/8/64 MiB
one-pixel BMP sources (one warmup, five runs each): median hit times are
1.118/8.670/69.344 ms, versus 0.542/4.476/34.367 ms for one current source
observation. All cached fingerprints match and managed reservations return to
zero; scoped Clippy passes. See `research/dedup-cache-hit-scale-qualification.json`
and `examples/cache_hit_scale_probe.rs`. This comparison is not a prior-version
benchmark; fixed order, warm files and background load limit interpretation.
Cold/network I/O, RSS, decoder and full acceptance qualification remain pending.

Multi-block cache-hit cancellation now qualifies an 8 MiB BMP source through all
263 caller checkpoints, with no retained managed reservation and a successful
cache retry. All 17 decode/cache tests and scoped Clippy pass. Observed maximum
callback-cancellation-to-return time was 26 microseconds on this warm local run;
this excludes waiting for an OS read and is not a cold-I/O latency bound. See
`research/dedup-cache-multiblock-qualification.json`. Large-pixel decoder and
platform/storage qualification remain incomplete.

Cache-hit cancellation now qualifies all nine normal callbacks for the tiny BMP
fixture through both caller cancellation and generation-token changes. Every
caller refusal is followed by a successful current-cache retry, with no managed
reservation retained. All 16 decode/cache tests and scoped Clippy pass. See
`research/dedup-cache-hit-all-cancel-qualification.json`. This is not large-file
latency or arbitrary concurrent mutation qualification; full acceptance remains
incomplete.

Fingerprint cache hits now repeat full source verification before returning a
cached result. The independent post-observation same-length mutation fixture
returns a source-change error; verification cancellation returns no evidence,
and retry reuses only current content. All 10 decode and six cache tests pass,
including persisted-cache/process tests; scoped Clippy passes. See
`research/dedup-cache-hit-guard-qualification.json`. Hits now read the full source
twice; this cost has not been measured. Observation cannot prevent later external
writes, and full regression plus broader acceptance qualification remain pending.

Mixed recursive indexed retrieval now qualifies all three presentation scopes in
one nine-file nested catalog with misleading `.data` extensions and overlapping
roots. Independently labelled GIF/APNG, classic/BigTIFF compression and PNG/BMP
pairs produce exactly three candidate/equal edges; changed frame, last page and
raster samples remain separate. Explicit and automatic modes agree without
issues and release managed reservations. All 13 container integration tests and
scoped Clippy pass. See `research/dedup-mixed-recursive-qualification.json`.
Held-out real captures and wider filesystem/container qualification still remain.

Indexed recursive explicit and automatic paths now qualify a deterministic
source mutation after first presentation release: all stale edges for that id
are removed, the healthy two-file group remains, and restoring the same path
then retrying recovers all three equal edges without old source errors. All 12
container integration tests and scoped Clippy pass; managed reservations return
to zero after each call. See `research/dedup-recursive-mutation-qualification.json`.
This covers one mutation window per route, not arbitrary concurrent mutation or
the complete acceptance table.

The complete all-feature regression after indexed grouping and recursive retrieval
passes 266 tests; the minimal-feature library, geometry and grouping
run passes 25. Neither run has failed, ignored or filtered tests.
See `research/dedup-full-after-indexed-roots-qualification.json` for terminal logs,
pre-invocation hashes and concurrent source drift. Existing corpus coverage is
not universal acceptance; all remaining requirements in the table still apply.

Recursive indexed explicit and automatic scans now qualify a recognizable but
undecodable GIF: the healthy GIF/APNG edge remains, corrupt-id preparation and
pair errors are retained, no negative identity evidence is invented, and unknown
signature candidate expansion refuses an insufficient budget. Repairing the
same path and retrying restores all three independently expected edges. All 11
container integration tests and scoped Clippy pass. See
`research/dedup-recursive-corrupt-qualification.json`; broader malformed
containers and full acceptance qualification remain pending.

Recursive indexed complete-presentation retrieval now supports both explicit
per-path modes and automatic content detection. Forty-six targeted tests pass;
all 577 explicit and 684 automatic callback cancellation checkpoints return no
partial report and release managed memory. The independently labelled nested
GIF/APNG fixture fits a one-candidate budget with overlapping roots; zero
candidate budget refuses on both routes. Scoped Clippy passes with existing
decoder warnings. See `research/dedup-indexed-roots-qualification.json`.
Broader recursive collision/mutation, real-capture and large-collection evidence
remain required; the full acceptance table is incomplete.

Complete-link grouping now indexes eligible group representatives before checking
every member. All 32,768 six-node graphs match an independent exhaustive oracle;
16,384 isolated entries require zero member-edge checks, and the 8,192-entry
chain retains all accepted edges. Five minimal-feature and 52 integrated tests
pass, with scoped Clippy passing. See
`research/dedup-sparse-group-selection-qualification.json` for source drift and
validation scope. Dense matching groups, recursive indexed integration and the
full acceptance table remain unfinished.

Candidate-only complete-presentation groups now share the existing source guards
and direct confirmation. Fifty targeted tests pass, including independent group
labels, collision/failure expansion, candidate-budget refusal, changed-source
invalidation, mixed scopes and all 976 normal cancellation checkpoints. The
indexed release probe's 18 measured runs retain all labelled groups/edges; scoped
Clippy passes. See `research/dedup-indexed-presentation-groups-qualification.json`.
Grouping work can still be quadratic and recursive indexed integration remains
pending, so the full acceptance table is not complete.

The existing complete-presentation search now uses per-source candidate keys,
with direct confirmation of equal keys, fallback after signature failure and
fresh source validation before a negative key decision. All 41 targeted tests
pass, including injected constant keys and signature errors. The 18 release
probe invocations preserve every labelled edge. Scoped Clippy passes; see
`research/dedup-presentation-key-routing-qualification.json` for staged source
scope and limitations. Full pair output and matching-bucket work remain
quadratic, so scalable retrieval and the full acceptance contract are unfinished.

Complete-presentation digest primitives pass 60 targeted unit/integration tests
and a final two-test integration rerun after a helper-only Clippy fix. Scoped
Clippy passes with existing decoder warnings. See
`research/dedup-presentation-digest-qualification.json`. These keys are not yet
integrated into retrieval, and digest overflow remains inconclusive; large-scale
whole-container search and the broader acceptance rows remain incomplete.

The current full `cargo test -p rrrah-dedup --all-features` invocation passes
249 tests, with none failed, ignored or filtered. The minimal-feature library
and geometry invocation passes 20 tests. This includes the newly revalidated
retained-presentation memory/cancellation guards and all current container,
partial-copy, natural-photo and recursive range/display regressions. No source
hash in rrrah-dedup changed after the staged full-run snapshot; a concurrent
`rrrah-decode/src/x3f.rs` edit detected after completion is not qualified by the
tested decoder build. There are no defined doctests.
Logs, hashes and the snapshot timing limit are in
`research/dedup-current-full-regression-qualification.json`. These tests prove
their fixture scope; the 18-row acceptance table remains incomplete.

Bounded left-presentation reuse passes all 23 current library unit tests and nine
container integration tests. The release probe's 18 measured invocations still
agree with every independent fixture label; 128-file medians are recorded in
`research/dedup-container-reuse-qualification.json`. Scoped Clippy passes with
existing warnings. Concurrent decoder `lib.rs`/`x3f.rs` drift prevents current
all-format claims. Reuse reduces preparations but does not complete scalable
whole-container retrieval or the full contract.

A release-mode full-presentation baseline verifies every pair at 8/32/128 files
for tiny GIF/APNG timelines and three-page TIFFs, with one warmup and three
measured runs per case. The 18 measured invocations all match independently
specified fixture labels. At 128 files, medians are 2.965 seconds for animation
and 20.169 seconds for pages; whole-probe maximum RSS is 9,469,952 bytes. This
qualifies this baseline, not scalable search. Concurrent `rrrah-decode/src/lib.rs`
and `pict.rs` drift limits source-state claims. See
`research/dedup-presentation-scale-baseline-qualification.json`.

Recursive reflected range/display qualification passes two focused tests,
including four complete directory/pair-oracle configurations and both modes of
frame/factory/cancellation/budget guards. Scoped Clippy passes with existing
warnings; no library runtime source changed in this milestone. See
`research/dedup-reflected-range-roots-qualification.json`. This extends directory
coverage on this Unix host without closing the broader platform/corpus rows.
A concurrent `rrrah-memory/src/lib.rs` edit differs from the live-validation
snapshot; these logs do not qualify that later dependency change.

Reflected range/display support passes a 94-test targeted invocation and one
additional recovery test. After simplifying private mode routing, all 39 current
library/collection tests and both modes of the natural-photo oracle pass again;
scoped Clippy passes with existing warnings. See
`research/dedup-reflected-range-qualification.json` for staged source hashes,
logs and the limits of these checks. This does not complete any broad acceptance
row by itself.

The ordinary alternative-model routing change passes 85 targeted tests (21 library,
9 geometry, 4 filtered, 16 collection, 11 file-scan and 24 natural-photo tests).
Its final unit retest and scoped Clippy also pass; Clippy retains existing warnings.
The validation snapshot and test-only differences are recorded in
`research/dedup-ordinary-alternative-qualification.json`. This is targeted
qualification, not a full crate or decoder/platform acceptance run.

An earlier full all-feature crate invocation completed 224 passing tests before the
reflected fitting-recovery change, with none failed, ignored or filtered; see
`research/dedup-alternative-geometry-qualification.json` for its exact scope and
supplemental current geometry runs. The subsequent fitting-recovery change is
qualified by 71 targeted unit/integration tests; see
`research/dedup-reflected-fit-alternative-qualification.json`. The newer reflected
filtered-routing validation passes 80 unit/integration tests, including all
library unit tests, geometry, filtered primitives, file/collection/directory
routing and natural-photo regressions. Library sources remain unchanged from its
234-file pre-launch snapshot. A concurrent PICT edit detected after completion
limits current decoder-wide claims; see
`research/dedup-reflected-filter-routing-qualification.json`. Neither scoped run
is an all-format decoder qualification. The decoder still emits three unrelated
`mef_sensor.rs` dead-code warnings.

The table remains incomplete. Passing primitive and integration tests do not
establish the full requested coverage. The next substantive gaps are:

1. Extend batch-wide source observation checks to complete-container grouping
   and directory integration; qualify more concurrent-mutation cases.
2. Independent full-resolution RAW/developed-raster pairs and broader ICC,
   alpha, HDR and cross-encoding fixtures, including declared color policy.
3. Arbitrary-angle/continuous-scale crop matching and independently labelled
   real positives/negatives for resize, compression, exposure, edits and bursts.
4. Measured latency/memory/throughput on large file collections, complete
   cancellation/allocation qualification and scalable full-container retrieval.
5. Linux/non-Unix filesystem behavior, unreadable paths, hostile containers,
   automatic complete recipe/version identity and cache crash/concurrent-writer evidence.

The present complete-container search exhaustively confirms bounded input pairs;
it is a correctness baseline with quadratic repeated-decoding work. Recursive
visual search still selects first frames. Neither is presented as the final
large-collection, every-format, calibrated similarity solution.

## Implementation boundaries

The `decode` feature uses existing `rrrah-decode` routing, full sensor development
and color management. The `raster` feature works with explicitly oriented decoded
rasters. Unknown color transforms produce errors. Selected-frame evidence never
implies whole-container equivalence. Full animation/page comparison is a separate
API; batch visual search currently uses selected frames only.

`ContentSnapshot` checks physical identity, length, time stamps and full content
before/after processing. These are filesystem observations, not an atomic snapshot.
Adversarial undetectable intermediate rewrites are not covered by this contract.

`FingerprintCache` keys require a fresh full-content digest and a recipe covering
all decoder/color/development/fingerprint options and versions. Resource policies
are included in per-file keys. Atomic save synchronizes file contents and replaces
from a temporary file in the destination directory. Directory-entry durability
across power loss is not claimed. Concurrent writers use last-publication semantics.

Global visual candidates use all four 64-bit feature channels and optional eight
transforms. Pigeonhole candidate union is compared against exhaustive search.
Visual radii are not calibrated duplicate probabilities. Complete-link groups
require every pair and do not infer missing edges from graph connectivity.

Exact crop search returns all canonical unscaled placements under work/result
budgets. Cancellation and budget exhaustion are inconclusive, never absence.
Uniform regions explicitly expose ambiguity. Approximate edited/rescaled crop
correspondence is still required.

Animation/page APIs retain normalized presentations under the shared memory budget.
Repeated selected animation decoding can replay previous frames and be quadratic.
ICO/CUR resources and selected TIFF directories are streamed through temporary
encoded files to reuse existing decoders. Disk capacity remains an I/O constraint.
TIFF linked directories are cycle/range/count bounded; embedded page ICC profiles
are explicitly admitted because the general TIFF decoder omits those profiles.
Untyped TIFF color is not assumed sRGB.

## Independent TIFF evidence

The saved TIFF fixture manifest records generator versions and SHA-256. Initial
32 fixtures were written and independently read by tifffile/imagecodecs with a
lcms-generated sRGB ICC profile. The passing matrix covers three pages, classic/
BigTIFF, both byte orders, none/Deflate/LZW/PackBits and unchanged/changed-last-page
pixels. BigTIFF signature recognition was added to the common raster decoder.

The expanded corpus adds uint16/float32 and eight orientation variants. All
48 fixtures pass their Rust comparison assertions, including one-bit uint16
and one-ULP float32 last-page differences in these samples. Hostile input, further sample
formats, photometrics, tiles and broad real-world corpus qualification remain.

## Validation

Use `cargo test -p rrrah-dedup --all-features` and
`cargo clippy -p rrrah-dedup --all-features --all-targets --no-deps -- -D warnings`.
These focused checks do not qualify all dependencies or the full workspace.
The separate general-decoder BigTIFF regression test passes (one targeted
test; the other decoder tests were filtered). Clippy for the dedup crate passes
with all features; dependencies still emit existing dead-code warnings.

`verify_similarity` now validates supplied one-to-one local correspondences
with deterministic exhaustive two-point scale/rotation/translation models,
explicit work/cancellation limits, residual scoring and a 2D support check.
Synthetic scaled/rotated/translated correspondences with an outlier pass;
duplicate and collinear support cannot inflate evidence. Descriptor extraction,
reflection/perspective models, overlap/pixel confirmation and independent image
crop/watermark corpus are still required. Geometry alone does not prove identity.

`verify_pixels` measures every source center in target overlap under supplied
similarity geometry, using premultiplied linear-light bilinear sampling.
`verify_bidirectional` covers both grids so enlarged-image edits between source
projections are not missed. Evidence includes compared/matched pixel counts,
coverage denominator and residuals; zero/limited overlap never proves whole
image identity. Fixtures cover crop overlap, changed pixels, transparent
interpolation and enlargement interior edits. Local descriptor extraction and
real resampling/photometric tolerance calibration remain required.

Native `local::extract` now detects nonmax-suppressed structure-tensor corners
and deterministic 256-bit BRIEF patches in linear premultiplied luminance.
Mutual unique nearest matching rejects ambiguous ratios on both sides and
checks comparison budgets before allocating matcher scratch. A synthetic
image/crop test exercises extraction, correspondence, estimated translation
and full bidirectional overlap verification; an independent seeded image is
negative. Flat/ambiguous/budget/cancel fixtures pass. The patch recipe now includes all four quarter-turn variants; scale and
arbitrary-angle normalization, richer photometric
features and independent real edited-image qualification remain required.

Quarter-turn local descriptors now match rotated crops without manual
correspondences. Tests estimate 90/180/270-degree crop transforms from extracted
features and verify every overlapping pixel in both directions. The synthetic
negative/ambiguity tests still pass. Arbitrary rotation, scale-pyramid matching,
reflection geometry and real photographic edits remain unqualified.

`extract_pyramid` now extracts quarter-turn local descriptors at bounded
dyadic scales and maps centers back to original coordinates. Downsampling
uses premultiplied linear area averages; odd edge rows/columns are excluded
only from reduced levels. Total visited pixels/features are capped. Synthetic
nearest-neighbor doubled images and doubled/quarter-turned images recover
scale, rotation and pixel-center translation without supplied matches. These
qualify the exact dyadic case, not arbitrary scales or resampling kernels.
Real photographic resize/crop/pyramid qualification remains required.

Color-conversion cancellation qualification: core sRGB/linear conversion checks
at most every 4096 pixels, including the managed-float reuse path. ICC preparation
checks between setup stages and splits each row transform into at most 4096-pixel
chunks; the external profile parser and transform constructor remain indivisible.
The selected-image, animation, DCX, TIFF and ICO/CUR adapters propagate both the
caller cancellation callback and generation token through this preparation.
Tests interrupt all core sample representations and 27 ICC checkpoints, asserting
that no partial raster is returned and all conversion reservations are released.
This does not establish cancellation latency inside every underlying decoder.

Exact-file race isolation: byte comparison now attributes failures to the input
that actually failed. A failed representative is removed and comparison retries
against the next surviving member; final admission retains healthy members
instead of discarding their entire group. A deterministic checkpoint sweep
mutates each of four identical files during traversal, hashing, byte comparison
and admission: the three healthy copies remain grouped, and diagnostics never
blame another member. Mutations after a member's final validation can still be
unobserved; this scanner does not provide an atomic filesystem snapshot.

Injected exact-hash collision qualification: a private test hook replaces both
sample and full digests with a constant after performing real bounded reads and
source validation. Fifteen empty, one-byte and multi-block files partition into
five independently specified byte-equality groups; unique unequal files remain
ungrouped. The public scanner keeps its fixed BLAKE3 policy. This directly tests
that sample/full-hash collisions cannot establish exact duplicate equality.

Unix path/type qualification: an ordinary file is replaced by a FIFO after
traversal; the scanner returns a Changed diagnostic and finds the two healthy
copies without waiting for a writer. A pre-existing FIFO is marked NotRegular.
Invalid UTF-8 bytes are preserved in an unavailable-path diagnostic on macOS.
The current macOS filesystem rejects creation of invalid-UTF-8 filenames with
EILSEQ, so actual duplicate discovery under such names remains qualified only
by an added non-macOS Unix test, not by an executed test on this host.

Timeline equality is now available separately from exact frame-sequence equality
through Sequence::same_timeline and Animation::same_timeline. Two-pointer rational
interval comparison permits different adjacent splits of identical displayed
pixels without rounding timebases. Repetition remains exact, pages retain their
ordered-frame contract, and zero-duration presentations contribute no time.
Intermediate rational values exceeding u64 return Timing rather than approximate
equality. Synthetic split/zero-delay/change/time/cancellation tests pass; real
cross-container fixtures and player minimum-delay semantics remain unqualified.

Cross-container timeline qualification: six authored encoded fixtures exercise
GIF, APNG and lossless WebP. Independently Pillow-encoded two-frame presentations
(200 ms red, 300 ms blue) match a separately written four-frame APNG timeline
(100+100 ms red, 150+150 ms blue) in both directions. Exact frame-sequence equality
remains false. One-millisecond duration and final-presentation pixel changes are
rejected for all three baseline formats. Generator/version/SHA-256 manifest are
saved alongside fixtures; Pillow independently reads the custom APNG frames.
This small corpus does not qualify every blend/disposal or minimum-delay policy.

Timeline arithmetic qualification adds 300 seeded eight-presentation sequences,
independently expanded into discrete sixth-second ticks. Split and changed-color
comparisons match that oracle in both directions (1200 comparisons), including
zero-time frames. u64::MAX seconds compare without overflow; coprime near-u64::MAX
fractional denominators return explicit Timing when an intermediate residual is
unrepresentable. Cancellation is checked before playback mismatch shortcuts.

Batch pixel confirmation: scan::confirm_pixels validates bounded requests and
candidate pairs, deduplicates/reorders pairs deterministically, freshly decodes
two selected frames at a time and compares their normalized pixels, sample scale
and hotspot directly. Both source files have full content snapshots checked
before/after processing. Equal, different and failed pairs are distinct outputs;
cancellation returns no partial report. Integration tests cover copies, changed
pixels, missing-source isolation, duplicate pairs, invalid self-pairs, cancellation
and released memory. This confirms supplied pairs only; it does not establish
exhaustive candidate retrieval or whole-container equality.

Pixel-confirmation failure qualification: mutation after managed decoding begins
returns a typed Source::Changed issue without equality/difference evidence.
Zero available memory produces an issue; a generation change while managed
buffers exist cancels the entire batch. All three paths release reservations.
Pair diagnostics now retain structured source/decode/normalization errors through
ConfirmationError and PixelIssue rather than discarding types into strings.

Confirmed grouping integration: confirm_pixel_groups combines fresh selected-frame
confirmation with bounded complete-link partitioning, retaining all accepted
pairs, differences and typed failures. A three-copy A-B-C chain remains a pair
plus singleton until A-C is directly confirmed; adding that edge admits all
three. Missing-source issues create no edges. Singletons do not prove uniqueness.
Confirmation now bounds raw pair input, including duplicate/reversed pairs, so
an endless duplicate stream cannot bypass the supplied work/storage policy.

Complete-presentation entrypoint: container::same_presentation explicitly accepts
SelectedFrame, Pages(DCX/TIFF/ICO-CUR) or Animation(GIF/APNG/WebP) modes. Incompatible
modes fail before file reads; cross-format animations use exact timeline equality.
Outer source snapshots revalidate both files after both decodes, covering a left
source changing during right processing. Integration proves that identical first
frames do not hide a changed final presentation, while GIF versus split APNG
matches; cancellation and final reservation release are checked. Complete page
integration and broad combined/batch-container corpus remain pending.

Complete-page entrypoint qualification: classic little-endian uncompressed TIFF
matches big-endian LZW BigTIFF with all three pages; a final-page change fails.
Two-page admission limit and zero-memory budget fail explicitly. DCX, ICO and
CUR page modes decode/compare through the common interface and release retained
memory. Selected-frame TIFF ICC extraction remains a concrete gap: the generic
selected-frame decoder omits this profile and returns a color error, while the
complete-pages adapter extracts it explicitly. This gap is not classified as
covered by the successful complete-page checks.

Selected TIFF page correction: pages::decode_tiff_selected enumerates bounded
classic/BigTIFF directories, decodes only the requested ordinal, applies that
page's ICC profile, and retains its original ordinal and complete page count.
container::same_presentation detects TIFF by source magic in SelectedFrame mode
and uses this corrected path regardless of filename extension. Tests cover all
three ordinals across endian/compression variants, changed last-page pixels and
out-of-range selection. The lower-level generic decode_selected_frame and visual
fingerprint/batch adapters still require routing to this path with explicit page
limits; their previous TIFF ICC gap is not yet globally resolved.

TIFF selected-page batch integration: fingerprint_file, scan_visual and
confirm_pixels now route by validated source magic to per-page TIFF ICC decoding.
FingerprintPolicy adds explicit max_frames for directory enumeration; the cache
recipe advances to v2 and includes that limit, preventing older/admitted entries
from bypassing stricter page policies. Integration tests fingerprint three selected
last pages, confirm cross-encoding equality and changed-pixel inequality, reuse
cache, reject reduced page limits and release memory. The low-level legacy
three-argument pixel-limit adapter remains unchanged; bounded batch routes and
the complete-presentation entrypoint use the corrected selected-TIFF path.

RAW/TIFF routing correction: selected TIFF routing now consults rrrah-decode's
authoritative image_source_kind under the shared memory budget. TIFF-shaped
camera inputs remain on the sensor/development path, including CR2 content under
a misleading JPEG extension and NEF extension candidates. Corrupt camera files
return decoder failures instead of falling through to ordinary-page extraction.
Regression tests cover these three negative inputs and recheck ordinary TIFF
batch/container positives; independent full-resolution RAW qualification is still
required for the broader camera corpus.

Recursive visual integration: exact::discover exposes the existing bounded
physical-file traversal without content hashing. scan_visual_roots maps its
ordered paths to per-run ids, retains aliases and traversal diagnostics, and
sends all regular files through selected-first-frame decoding/cache/indexing.
Unsupported contents remain decoder issues; filename extensions do not silently
filter them. A nested-directory fixture finds two visual copies, excludes a
hardlink alias from duplicate counts, retains a broken-file issue, exposes depth
omissions and supports cancellation. Ids are traversal-local, not persistent
identities; complete animation/page enumeration is still a separate operation.

Batch complete-presentation integration: container::confirm_presentations accepts
bounded per-id requests with explicit selected-frame/pages/animation modes and
bounded raw candidate pairs. It normalizes/deduplicates pair ordering, performs
fresh full-presentation comparisons, and retains separate equal/different/typed
issue outputs. Cancellation returns no partial report. This confirms supplied
pairs, not exhaustive retrieval of full-container candidates; callers must retain
the per-id presentation modes when interpreting equality or grouping results.

Exhaustive complete-presentation search: scan_presentations generates every id
pair, checks n*(n-1)/2 against the mandatory pair budget before decoding, and
retains deterministic equal/different/typed-issue results. This correctness
baseline has quadratic pair work and repeated decodes; it does not claim indexed
performance. No selected-first-frame or perceptual gate suppresses candidates.
A new authored APNG begins with a different zero-duration green frame before the
same red/blue timeline; the fixture targets the first-frame-filter false negative.


Group observation correction: confirm_pixel_groups captures full content/identity
snapshots before pair processing and revalidates every successfully observed
source after all comparisons. Edges and differences involving changed/unavailable
sources are removed before complete-link admission; typed per-id source issues
remain in ConfirmedGroups. An injected change after the first pair releases its
rasters removes the stale (1,2) edge while preserving the healthy (2,3) group.
This is a bounded observation contract, not an atomic snapshot; adversarial
intermediate writes restored before validation remain outside the guarantee.


Complete-presentation grouping: scan_presentation_groups performs bounded exhaustive
comparisons, captures batch-wide source snapshots, removes all equal/different
edges touching changed/unavailable sources, and admits complete-link groups with
separate typed source diagnostics. Three cross-format/split/zero-prefix animations
form one group; mutation between pair processing removes the changed GIF and
preserves the two healthy APNGs. This remains quadratic and observation-based.

Group cancellation propagation: both selected-pixel and complete-presentation
batch grouping retain every request generation token beyond pair decoding. The
combined callback covers initial snapshots, final source verification and
complete-link admission, so a generation change after the final comparison cannot
return a grouped result. Tests derive the final admission checkpoint from a
successful baseline, change the generation there and require cancellation plus
released decoder/conversion memory.


Recursive complete-presentation grouping: scan_presentation_roots combines shared
bounded physical-file discovery with exhaustive batch-wide validated grouping.
It returns per-run id/path/mode mappings, aliases, traversal diagnostics and full
comparison/group evidence. The caller explicitly chooses each presentation mode;
automatic content-based mode detection is still pending. The nested-directory
integration fixture groups GIF with split APNG, keeps the changed final-frame
APNG separate, reports incompatible selected-frame pairs and keeps hardlink aliases
out of copy counts. Pair limits remain quadratic-work admission controls.

Content-based mode detection: presentation_kind::detect_presentation recognizes
GIF, APNG acTL before IDAT, animated WebP VP8X, DCX, ICO/CUR and ordinary TIFF
from validated source bytes. PNG chunk traversal has an explicit max_chunks bound,
checks payload offsets and skips payloads without allocating them. TIFF-like RAW
remains selected-frame sensor processing through authoritative decoder routing.
Seven independently saved formats renamed to .bin retain their modes; misleading
CR2 .jpg stays sensor scope. Metadata limit/cancellation tests pass. Classification
is not format validation, and automatic recursive integration remains pending.


Automatic recursive integration: scan_presentation_roots_auto discovers physical
files, classifies content under chunk/file limits and groups complete presentations
without extension filters or user-supplied modes. Per-id classification failures
remain visible with absent modes and singleton entries. Source snapshots span
classification through final group admission, so changed classification sources
cannot retain earlier edges. Renamed .bin GIF/APNG fixtures group correctly, a
changed final presentation stays separate, and an impossible PNG chunk length is
retained as a detection issue. Exhaustive pair cost remains explicitly bounded.

Real RAW integration probe: examples/raw_probe requires authoritative sensor routing,
checks sensor dimensions against the pinned LibRaw oracle, develops full resolution,
validates normalized pixels, computes exact/visual fingerprints and checks final
managed-memory release. scripts/qualify-dedup-raw.py verifies every source SHA-256
before running it and records per-file results in dedup-raw-qualification.json.
The scope does not certify independently rendered color or RAW/raster equality.
The initial 27-file run passed 16 and failed 11 at a 1 GiB admission limit.
Its report is preserved as dedup-raw-qualification-before-memory.json.

After deriving live buffer sizes for tiled AHD, X-Trans, stage-3 opcodes,
highlight pyramids and full-resolution RGBA output, development admission now
reserves 35 bytes per sensor sample plus 8 MiB of bounded scratch, rather than
64 bytes plus 8 MiB. The rerun passes 24/27: NEF, CR2, both ARW, RAF, RW2, MOS
and IIQ now pass. All 16 previously passing developed pixel digests remain
identical; dedup-raw-memory-comparison.json records this comparison. DNG managed
peak falls from 832169984 to 470205440 bytes. These reservations are not RSS.

Remaining failures: 3FR and FFF still exceed this 1 GiB budget; Sony F828 SRF
requires four-color RGBE development outside current Bayer/X-Trans support.
All 11 development tests and the full rrrah-dedup all-feature test suite pass.
Strict core Clippy remains unsuccessful due to existing style diagnostics.
This is improved actual capacity evidence, not complete RAW/raster color proof.


RGBE full-resolution development now uses the existing four-plane calibration
and source-phase bilinear interpolation directly into cropped/oriented scene-linear
RGBA. Emerald remains independent; the 3x4 color transform retains its contribution.
Output is managed under the caller budget; no full RGB or normalized sensor scratch
is allocated. DNG opcode lists are explicitly rejected for this RGBE path; Bayer
spatial highlight reconstruction is not applied to four-plane data.

The pinned real Sony F828 SRF develops 3287x2460 pixels and fingerprints successfully
with 145907520 managed peak bytes, releasing all managed memory afterwards. The
separate report dedup-srf-development.json preserves source and binary SHA-256.
A synthetic full-development test compares the nonzero Emerald contribution to
independent LibRaw rgb_cam coefficients, including HDR/negative output, crop,
orientation, memory refusal and mid-development cancellation. Full rendered-camera
color equivalence remains unqualified. All 12 development tests and the full
rrrah-dedup all-feature suite pass; dedup example Clippy passes without dependency
linting. The earlier 24/27 report remains historical evidence from before this change.

Large RAW capacity was separately qualified with an explicit 2 GiB budget:
Hasselblad 3FR (8384x6304 sensor, 8272x6200 developed) and FFF (8282x6240
sensor, 8176x6132 developed) both pass full development/fingerprinting and
release memory. Managed peaks are 1963939840 and 1920536768 bytes respectively.
dedup-large-raw-qualification.json records both pinned sources and the executable
identity; its required count is 2, with manifest_required 27, so it is not a
27-file rerun. Both original 1 GiB refusals remain recorded. The probe and
qualification script now accept an explicit budget and selected filenames,
with unknown selections rejected. Together with the previous 24-file successes
and the new SRF proof, every current corpus file has full-development evidence
under recorded budgets; independent color and memory reduction remain open.


Independent lossless raster equality corpus: Pillow 12.3 encodes and reads back
29 authored CC0 opaque/straight-alpha base, one-code visible-change, and
invisible-RGB-change fixtures across PNG, TIFF, BMP, PPM, TGA, lossless WebP and
QOI. Raw authored RGBA oracles and file SHA-256 values are archived under
fixtures/raster-equality; two additional fixtures qualify declared linear QOI
and embedded sRGB ICC priority. All normalized bases match the raw-pixel oracle,
visible edits change equality/digests, and invisible edits preserve both.
A batch confirms all 21 pairs among seven opaque encodings and rejects all
seven pairs against a one-pixel edit, with no errors and zero retained memory.

DecodeRequest::assume_untagged_srgb explicitly permits conventional interpretation
of otherwise untagged TIFF/PNM/TGA. It defaults false; strict requests still reject
unknown color. Embedded ICC and explicit declarations take priority, and unknown
linear/HDR primaries are not overridden. The request setting is included in the
v3 file-fingerprint cache recipe. Tests prove strict TIFF requests cannot reuse
an assumed-color cache hit, while QOI linear declarations and PNG ICC remain
unchanged. Broad cross-profile precision/calibration and direct selected TIFF ICC
handling remain separate work; this corpus is not RAW/raster color qualification.

Validation: 89 dedup unit/integration tests, then the three raster corpus tests
again after adding embedded-profile priority assertions; nine existing decoder
raster tests pass. The fixture integrity check verifies all 33 hashes, including
raw oracles and declaration negatives.


Direct selected-frame TIFF ICC gap is now resolved. decode_selected_frame routes
through the same snapshot-aware selected-page path as fingerprinting and pixel
confirmation; it preserves each page's original index/count and its own ICC.
Its convenience limits are 1 GiB encoded source and 4096 TIFF directories;
decode_selected_frame_bounded exposes explicit source/pixel/directory limits.
Other formats use the native decoder directly after source observations; RAW TIFF
containers remain authoritative sensor inputs and do not fall back to raster pages.
The source is fully revalidated after decoding. A mutation after memory admission
is rejected, releasing all output memory.

Independent page-color fixtures use lcms-built linear-sRGB and standard sRGB
profiles, Pillow PNG and tifffile TIFF encoding. Both TIFF pages have identical
encoded samples but distinct profiles. Each selected page equals its separately
encoded matching PNG; replacing only the second profile changes full-page and
full-container equality. Assumed-untagged-sRGB cannot override either ICC profile.
The fixture/oracle manifest pins six file SHA-256 values and lcms identity.

An independent lcms float oracle verifies every one of the 20 pixels for each
profile. Maximum linear channel errors are 0.000238597393 and 0.000321269035,
within the explicitly declared 0.0005 interoperability tolerance. This tolerance
belongs to the oracle color check, not canonical exact pixel equality. This small
profile corpus is not full color-gamut/camera/rendered-image qualification.

The full 91-test suite passes after the TIFF routing/profile change; the added
source-mutation regression passes separately, bringing verified distinct tests to
92. Big/classic TIFF, both byte orders, compression, selected-page limits, changed
last pages, renamed containers, corrupt RAW routing and final memory release are
covered. dedup-tiff-profile-qualification.json records this scope and measurements.


Native arbitrary-angle local candidate extraction is now available as
local::extract_oriented. It estimates a circular intensity moment and steers
256 bilinearly sampled comparisons around each admitted corner. Border and
orientation-ambiguous patches are omitted. FeatureRecipe distinguishes the
original QuarterTurnBriefV1 from OrientedBriefV1; matching rejects mixed recipes
and checks cancellation while validating admitted input lists. Pixel/corner/
feature limits still apply, with mid-descriptor cancellation returning no list.

Pillow independently resampled a seeded, blurred linear-intensity texture by
17, -37 and 63 degrees. Native oriented extraction, mutual-ratio matching and
similarity verification recover all three transforms with 170-214 inliers.
Bidirectional verification compares at least 18000 pixels in each direction;
over 98% match at the explicitly supplied 0.03 per-channel candidate tolerance.
An independently seeded texture fails geometry and identity pixel-residual gates.
Ten fixture hashes, transform metadata, thresholds and measured counts are
preserved in the rotation manifest and dedup-oriented-qualification.json.

This evidence covers three synthetic arbitrary-angle cases, not all rotations,
continuous-scale normalization, natural-photo confidence, bursts or edits. It
never changes canonical exact-file or exact-pixel equality. All 94 full-feature
tests pass; seven local/pyramid/oriented tests pass again after tightening recipe
admission/cancellation checks. Source fixtures use explicitly linear code values,
so these algorithm tests do not qualify encoded-image color routing.


Non-dyadic scale candidate normalization now has a separate fixed recipe:
local::extract_multiscale_oriented uses patch radii scaled by 0.75, 1, 1.25
and 1.5, each with its own moment orientation. Matching OrientedScaleBriefV1
tries all 16 descriptor-variant pairs per admitted feature pair. Original
quarter-turn and orientation-only recipes keep their prior comparison semantics;
mixed recipes remain invalid. MatchPolicy::max_comparisons bounds feature pairs,
with the fixed maximum of 16 variant comparisons documented explicitly.

The orientation-only baseline failed the independent 1.35 resize (one match).
After fixed-scale normalization, independently resampled 1.35, 0.73125, and
1.35-plus-17-degree fixtures produce 149, 71 and 135 geometric inliers. Geometry
estimates the actual continuous transform; extraction receives no ground-truth
scale. Both-direction pixel comparisons exceed 10000 pixels and 90% matching
at the caller's 0.03 candidate tolerance. An unrelated texture fails geometry;
mixed recipes, pixel admission and mid-variant cancellation are qualified.

The rotation fixture manifest now covers 16 authored files including the new
resizes. The original ten-file manifest is archived with its unchanged SHA-256
as dedup-oriented-fixture-manifest.json; its earlier qualification remains
reproducible. dedup-scale-qualification.json records the new manifest identity,
thresholds and measured transform/pixel counts. All 95 full-feature tests and
strict dedup test Clippy pass. These three synthetic scales do not establish
arbitrary-scale/blur invariance, real-photo calibration or integrated file-level
local candidate discovery. Exact file and exact pixel equality are unchanged.


Selected-frame local file integration is now available through
local_scan::compare_local_files, scan_local_files and scan_local_roots.
The pair pipeline decodes with explicit source/page/pixel limits, extracts the
fixed multiscale oriented recipe, mutually matches descriptors, fits similarity
geometry and verifies both pixel grids. Acceptance requires caller-declared
minimum compared pixels, per-image overlap and per-direction matched fractions.
Candidates retain correspondence coordinates, geometry inlier indices and both
pixel residuals for review. They never become exact file/pixel/container identity.

The batch admits file count and exhaustive pair count before decoding, holds
only one decoded pair at a time, and retains rejected comparisons plus typed
per-pair/source errors. Initial and final batch snapshots remove all evidence
involving observed changed sources. Cancellation is latched, including one-shot
callbacks and a generation change during final admission; no partial report is
returned. Traversal reuses shared discovery, physical aliases and diagnostics,
including recursive overlapping roots. Per-file frame/color/generation options
are available through explicit DecodeRequests; recursive defaults remain strict.

Five integration tests pass: a real decode/color path for authored PNG original,
Pillow crop, independently scaled/rotated image, unrelated image and corruption;
file/pair/policy limits; mutation between pair comparisons preserving the healthy
pair; final generation/one-shot cancellation; and nested overlapping roots with
hardlink aliases and corrupt decode diagnostics. The five-file scan yields three
expected candidate pairs, three rejected healthy pairs and four corrupt-file pair
issues. Managed output memory releases to zero. The crop fixture is pinned by
source/file hashes and a saved independent Pillow generation script.

All 100 full-feature tests and strict dedup test Clippy pass. Measurements and
policy are in dedup-local-file-qualification.json. This is bounded exhaustive
local retrieval with repeated decoding/extraction, not scalable indexed retrieval.
Natural-photo calibration, broad transform/edited-crop coverage, complete animated
local comparisons and universal allocation accounting remain unqualified.

### Real-photo baseline: four independent camera preview scenes

`docs/research/dedup-photo-qualification.json` records the observational baseline
from `photo_probe`: 20 positives (75% resize, central crop, 17-degree rotation,
encoded RGB brightness 0.7, JPEG quality 60) and six distinct-scene negatives.
Geometry exists for all 20, but the existing strict pixel policy accepts only
10/20; all brightness variants fail and textured scenes expose resampling and
compression residual failures. There are zero accepted negatives in these six
pairs. This is evidence of incomplete real-photo coverage, not calibrated recall
or specificity. No thresholds were loosened to make this baseline pass.
The independently generated, checksummed fixtures are rendered embedded camera
JPEG previews from four pinned CC0 sources, not full RAW development evidence.
Reproduce them with `scripts/generate-dedup-photo-fixtures.py --corpus PATH` in a
Pillow environment and run the decode-feature `photo_probe` example.

### Opt-in photometric local-file verification

`compare_local_files_photometric` retains the strict bidirectional pixel residuals
and additionally exposes separately fitted forward/reverse per-channel gain,
offset, opaque sample counts and residuals. Alpha is never fitted. The model is
one global affine transform in linear RGB, with explicit positive gain/offset
bounds, opaque overlap minimum and per-channel variance minimum. Flat or
insufficiently opaque overlap and out-of-bounds fits are inconclusive errors.
Each direction takes two pixel-grid passes, constant model memory and no fitted
image allocation. Source snapshots and cancellation are shared with strict
local-file comparison; strict APIs retain their existing acceptance behavior.

`docs/research/dedup-photometric-qualification.json` records 14/20 accepted
real-preview positives (previously 10/20), including all four encoded-brightness
variants, and zero accepted distinct-scene negatives among six pairs. No strict
residual, coverage or matched-fraction threshold was loosened. These four scenes
are development fixtures, not a statistically calibrated evaluation. Six
resampling/rotation/JPEG cases remain rejected. `photo_probe` defaults to the
strict baseline; `--photometric` explicitly enables fitting. Two primitive tests
qualify known affine coefficients, local edits, alpha preservation, low texture,
resource bounds and cancellation; two file tests cover all four brightness pairs,
six different-scene negatives and rejection of invalid fitting before I/O.
Batch/directory fitting and broader photometric/alpha/HDR qualification remain.

### Geometry refinement and independent real JPEG diagnosis

The winning two-point similarity hypothesis is now refined once by centered
least squares over its inliers, then rescored against every correspondence.
Refinement is retained only if inlier count/residual ranking improves; degenerate
fits preserve the original hypothesis. Hypothesis limits still bound exhaustive
two-point search, with an additional bounded linear refinement/rescore pass.
Cancellation is checked throughout. Independently authored symmetric noisy
corners recover the known transform and exact analytic squared residual 0.2;
a zero-scale least-squares case tests preservation of the prior model.

`docs/research/dedup-resampling-diagnosis.json` records 14/20 real-preview
positives after refinement, with six failures still present. A geometry oracle
uses identity for JPEG and the known authored Pillow rotation, without feature
matching. Four failures (1084/1294 rotation and JPEG) persist even under this
known geometry, so inaccurate matching alone cannot explain them. The two resize
failures remain unqualified by this oracle. Native JPEG readback is separately
compared with independently decoded Pillow/libjpeg PNGs: all pixels of all four
JPEGs stay within the existing linear channel tolerance 0.03, with observed
maximum errors 0.01460, 0.02235, 0.02463 and 0.02614. This checks these four files,
not every JPEG implementation mode. Frequency-aware residuals must still be
qualified against edits, uniform content and bursts; no acceptance thresholds
were loosened and no full-coverage claim follows from this diagnosis.

### Explicit filtered photometric file-pair evidence

`compare_local_files_filtered` retains strict pixels and unfiltered photometric
residuals and additionally fits/verifies corresponding box-window means. Radius
is explicit (1..=8), in each source grid; each source window is mapped into the
target by the estimated transform. Both grids are checked independently. Full
windows must be inside both images, so borders reduce reported overlap rather
than receiving invented padding. Premultiplied RGB/alpha are averaged; alpha is
never color-fitted. This is a visual-candidate policy and can suppress genuine
high-frequency edits; it never proves exact pixel equality or containment of all
details. Default strict and unfiltered APIs do not automatically enable it.

The aggregate sample-pair admission covers both fit/residual passes on both
images: `2 * (left_pixels + right_pixels) * (2*radius+1)^2`. Each pair reads one
source pixel and bilinearly samples the target. Both grid pixel quotas and the
aggregate work cap are checked before sampling. The model uses constant working
memory; cancellation is checked for every sample. Local extraction/geometry and
native decoding retain their separate budgets and existing allocation limits.

`docs/research/dedup-filtered-qualification.json` records 19/20 accepted real
preview positives at radius 2 (5x5), with unchanged residual tolerance 0.03,
matched-fraction threshold 0.9 and overlap threshold 0.3. All sixteen resize,
crop, 17-degree rotation and encoded-brightness pairs plus three natural-photo
JPEGs pass. The color-swatch JPEG (1084) remains a measured miss; it is not used
as an expected rejection regression. Six different-scene pairs have no accepted
candidate, but they do not calibrate burst/repeated-pattern specificity.

Three primitive tests independently qualify analytic gradient/checker means,
retained strict high-frequency differences, broad content/alpha edits, opaque
flat-content rejection, complete work/target-quota admission and cancellation.
Three additional file tests qualify the nineteen successes, six scene negatives,
and discarding evidence after late cancellation, generation changes or source
mutation. All memory reservations are released. The full-feature library suite
passes 113 tests; targeted filtered tests also pass after the target-quota guard
assertion was added. Batch/directory fitting, broader real negatives, alpha/HDR,
remaining JPEG handling and the other open acceptance rows remain unfinished.

### Controlled signal-space and window-size qualification

`ColorFilterPolicy` and `FilterColorSpace` explicitly choose linear or encoded
sRGB for filtered visual comparison. `compare_local_files_with_color_filter`
retains strict linear pixels and the original linear photometric evidence. The
original `compare_local_files_filtered` API still selects linear light. No
automatic space/window fallback is introduced. Tolerances, fitted gains and
offsets belong to the selected space; equal numeric tolerances across spaces do
not imply equal sensitivity. The evidence records the space and window policy.

Encoded comparison applies the sign-preserving extended sRGB transfer before
premultiplication, bilinear interpolation and window averaging, with no HDR or
negative-value clipping. The reference is the informative conversion section of
[W3C CSS Color 4](https://www.w3.org/TR/2026/CRD-css-color-4-20260930/#color-conversion-code).
An independent 80-digit Decimal generator pins transfer landmarks and 17x17
RGBA32 fixtures. Primitive tests qualify the transfer order, hidden RGB, negative
and HDR landmarks, known encoded affine coefficients, and broad color/alpha
edits at a 7x7 window. Fine edits can still be suppressed by filtering; exact
identity remains a separate evidence class.

`docs/research/dedup-color-filter-qualification.json` records the controlled
experiment: linear 5x5 accepts 19/20, encoded 5x5 also accepts 19/20, while linear
and encoded 7x7 each accept 20/20. Thus increasing the window alone is sufficient
for the remaining color-swatch JPEG in this four-scene development corpus;
encoded transfer is not claimed as the necessary fix or as generally superior.
Six different-scene pairs yield no candidate in either 7x7 mode. These are not
look-alike/burst negatives and cannot establish broad specificity. Source/work
caps remain explicit; all original strict residuals are available for review.

The full-feature suite now passes 118 tests, including all twenty photo
transformations in each space at radius 3. The next qualification must use held-
out positives and real look-alike/burst/repeated-pattern negatives; full RAW,
batch/directory integration, platform/cache behavior and scale remain open.

### Partial-copy challenges and inconclusive fitting diagnostics

`photo-collages` contains 24 independently Pillow-authored combinations of the
four pinned CC0 previews. Each preserves an exact 40% or 70% left strip and
replaces the rest with another scene. These are challenges to whole-frame 90%
residual matching, not captured burst negatives. Their copied region is genuinely
related, so no claim of unrelated content/region uniqueness is made.

`docs/research/dedup-partial-copy-qualification.json` separates all outcomes.
The normal encoded 7x7 policy has 4 no-geometry results and 20 fitting-inconclusive
results, with no accepted candidate. A diagnostic relaxed fit guard (gain
0.01..100, offset magnitude <=1) retains the same residual/overlap gates: 16
pairs with at least ten geometry inliers are rejected by residuals, 4 lack
qualifying geometry, and 4 remain fitting-inconclusive. This does not recommend
relaxed production thresholds or count unknowns as successful negative decisions.
The default policy has not demonstrated residual discrimination on these pairs.

`WarpError::Fit(PhotometricFitFailure)` distinguishes insufficient opaque sample
counts, the RGB channel lacking variance, and a fitted change outside caller
policy. Invalid geometry/policy, source corruption, work excess and cancellation
retain separate errors. Valid input without a qualified model stays inconclusive.

A separate independent darkened texture fixture exposed a runtime error: the
auxiliary linear fit rejected its low variance before explicitly requested
encoded filtering could run. Selected filtering now proceeds after an auxiliary
Fit refusal, storing it in `LocalFileEvidence.photometric_failure`; strict pixels
remain available. The selected filtered fit still must qualify. Cancellation,
source changes, resource/policy errors and selected-fit failure still abort.
The unfiltered photometric API keeps its refusal behavior. The dark regression
passes with recorded LowVariance and fully matched filtered grids, releasing all
memory reservations. Full-feature validation passes 121 tests. Real captured
bursts, held-out calibration, batch/directory integration and other rows remain.

## Explicit constrained photometric fitting

`verify_filtered_photometric_constrained` and
`compare_local_files_with_constrained_filter` select the least-squares fit inside
caller-specified gain/offset bounds. The convex rectangle optimum is computed
from its interior solution or the analytic minima on its four edges; independently
clipping two fitted coefficients is insufficient. Evidence records the selected
fit mode and each channel requiring a boundary solution. Existing APIs retain
out-of-policy refusal. Too few samples and low variance remain inconclusive.

With the original gain `[0.2, 5]`, absolute offset `0.1`, encoded-sRGB radius 3,
and unchanged residual/coverage gates, all 20 existing camera-preview positives
are accepted. All 24 authored 40%/70% partial-copy challenges are rejected:
20 through filtered residual evidence and four without sufficient geometry.
None now requires widening coefficient bounds or treating a fit refusal as a
successful negative. These are four original scenes and authored transformations,
not held-out scenes or captured burst calibration. A separate optimizer test
checks four moment sets against an independently expanded objective over a dense
coefficient grid, including a counterexample to independent clipping.

Evidence: `docs/research/dedup-constrained-fit-qualification.json`. Whole-collection
fitting, confidence calibration and the acceptance matrix remain incomplete.

Constrained-fit validation: 123 all-feature tests pass; crate Clippy with tests
and examples passes under `--no-deps -D warnings`; existing decoder dependency
warnings remain. Source hashes and experiment commands are in the report.

## Explicit comparison policy in file and directory searches

`LocalSearchPolicy` carries the local geometry/acceptance policy and an explicit
`LocalComparisonMode`: strict pixels, unfiltered photometric fit, or a filtered
fit with declared signal space and fitting mode. `compare_local_files_with_policy`,
`scan_local_files_with_policy`, and `scan_local_roots_with_policy` share this
selection. Original entry points retain strict comparison. All policies validate
before consuming file requests or traversing roots; unsupported/invalid sources
remain diagnostics rather than unique-file claims.

Collection scans retain initial/final content snapshots, invalidate every pair
involving a changed source, preserve healthy pairs, and latch user/generation
cancellation through final validation. The new integration tests qualify both
filtered fit modes on an independently authored darkened camera-preview pair,
unfiltered photometric selection, recursive overlapping roots and corrupt-file
isolation. Constrained collection tests also qualify source mutation, final
request-generation changes and one-shot late cancellation. Strict residuals and
selected fit mode remain in every successful fitted result.

This closes comparison-mode wiring for bounded selected-frame batch/directory
searches. It does not establish a scalable local-feature index, whole-animation
transformation search, or held-out burst calibration. Exhaustive pair admission
and repeated decode/extraction remain quadratic.

Collection-policy validation: the all-feature suite passes 127 tests; the
subsequent local-scan run passes all ten tests, including the newly added
unfiltered-photometric batch regression, for 128 distinct passing tests. Crate
Clippy and diff checks pass. Reproduction and source hashes are in
`docs/research/dedup-local-search-policy-qualification.json`.


## Information gates and informative collisions through the file pipeline

Six independently authored PNG fixtures qualify low-information behavior and an
informative perceptual collision. A nine-cell gradient and the same gradient with
alternating encoded +/-30 checker pixels produce zero global fingerprint distance
with informative evidence. File search retains this candidate, but fresh full-
resolution pixel confirmation declares it different. Equal hashes do not become
pixel identity, even when the information gate passes.

Four flat opaque/transparent fixtures produce zero gated candidates and six
ungated candidates. Full-resolution confirmation accepts only the two fully
transparent images with different invisible RGB and rejects five other pairs.
All four selected-frame local comparison modes produce no geometric support on
these flat files and expose no fabricated pixel/color evidence. Their candidate
flags are false; this does not prove uniqueness. Exact or pixel confirmation is
needed to identify the equal transparent pair omitted by perceptual gating.

`information.rs` passes all three tests, fixture checksums and crate Clippy pass.
No production behavior changed in this qualification. These synthetic controls
add to the existing 128 independently run passing tests (131 distinct tests total);
no single fresh 131-test suite run is claimed. Captured burst/look-alike negatives
and calibrated similarity thresholds remain incomplete. Reproduction and hashes:
`docs/research/dedup-information-qualification.json`.

## Exact pixel discovery independent of visual candidates

`scan_equal_pixels` searches all admitted selected-frame pairs with direct native
full-resolution pixel comparison. `scan_equal_pixel_roots` applies the same search
to recursive discovery, preserving aliases and traversal diagnostics. These entry
points recover equal uniform/transparent pixels without disabling perceptual
information gates or supplying a candidate list. No perceptual score proves
identity. `PixelSearchPolicy` declares decoder, file and pair limits; fingerprint
recipe/cache fields of its decoder policy are unused.

All pair counts are admitted before source reads. Initial/final content snapshots
cover the complete scan; every equal/different decision involving a changed source
is discarded with an attributed source issue, preserving healthy decisions.
Cancellation is latched across both user callbacks and all request generation
tokens. Missing-source and decode errors stay visible. A scan with no candidate
list still does not claim unsupported or omitted files are unique.

The information fixture search discovers the equal transparent pair among all
15 pairs of six files and rejects the other fourteen. Independently encoded opaque
white PNG/BMP files are also discovered as equal despite lacking visual texture.
`pixel_search.rs` qualifies batch mutation, final generation/transient user cancel,
recursive overlapping roots, missing sources and corrupt-file diagnostics.

This implementation has quadratic pair counts, repeats decoding and retains only
two decoded frames per comparison. It is not a scalable canonical-pixel index,
whole-animation equality search, or a universal RAW/raster qualification.

Exact pixel discovery validation: all 136 full-feature tests pass, including
the independently encoded uniform PNG/BMP regression. Crate Clippy with tests
and examples and diff checks pass. Evidence and source hashes:
`docs/research/dedup-exact-pixel-discovery-qualification.json`.

## Canonical selected-frame pixel index

`NormalizedRaster::selected_frame_digest` keys the canonical normalized linear
RGBA pixels and dimensions plus the exact sample-scale bits and optional cursor
hotspot. It excludes container frame count/index, matching selected-frame equality.
Invisible RGB and signed zero use the existing canonical pixel rules. Digest
matches are candidates; `scan_indexed_pixels` directly confirms every same-key
pair. `scan_indexed_pixel_roots` integrates recursive discovery and aliases.

The index decodes each stable source once for its key, retaining one normalized
frame at a time during indexing. Its ordered digest buckets avoid comparisons
between unrelated keys. Six information fixtures require six indexing decodes
and one confirmation pair, yielding the same exact equal pair as the fifteen-pair
exhaustive search. A forced constant-key test confirms all six pairs of four files
and rejects five false matches. A collection of 128 independently authored unique
one-pixel BMP files needs 128 indexing decodes and zero confirmation pairs,
succeeding with a zero candidate-pair budget. These counts demonstrate pruning;
they are not measured large-image throughput or physical-memory qualification.

All initial source snapshots are recorded before the first indexing decode.
Final validation discards all equality/difference decisions and analysed entries
for changed sources, including a source changed before its own decode. Late
request-generation and transient user cancellation return no partial report.
Missing sources and corrupt contents remain separate diagnostics. Both direct
and indexed searches remain available as cross-checks.

`max_pairs` in the indexed search bounds same-key confirmations. Buckets of many
equal keys can still produce quadratic result counts, with explicit admission.
There is no persistent pixel-key cache or transformed/local-feature index. Broader
performance, RAW/raster color and held-out similarity qualification remain open.
Evidence: `docs/research/dedup-canonical-pixel-index-qualification.json`.

Canonical pixel-index validation: the current all-feature suite passes all 144
tests, including adversarial keys and changed-before-decode admission. All 25
focused index/raster/information/pixel-search tests also pass; crate Clippy with
tests/examples and diff checks pass.


## HDR/alpha/precision key-semantic qualification

Eleven independently labelled normalized frames qualify canonical keys against
selected-frame equality over all 121 ordered pairs. The seven exact-equivalence
classes include negative and above-one HDR RGB, sub-byte visible changes, visible
alpha changes, hidden RGB at alpha zero, signed zero and equivalent 8/16-bit sRGB
endpoints. Each normalized float view is also compared to the authored declared-
linear samples. This is a sample-semantic qualification, not captured HDR or
independent RAW/raster development-color proof.

Four authored PFM files vary byte order, a sub-byte RGB value and advisory sample
scale. PFM does not specify RGB primaries. Both strict requests and the existing
`assume_untagged_srgb` flag return `ColorTransformRequired`; the documented flag
only permits conventional untagged encoded TIFF/PNM/TGA and never overrides
unspecified linear/HDR primaries. The exact index reports four source decode
issues, and exhaustive confirmation reports six pair issues with no equal or
different claims. These unknown-color results must not be counted as negatives.
An explicit declared-linear-primary policy for untagged HDR file searches remains
an uncovered functional requirement; arbitrary sRGB inference is insufficient.

The new two regressions pass within the five-test raster and ten-test pixel-search
runs; crate Clippy passes. Production behavior is unchanged. Combined with the
previous full-feature 144-test run, 146 distinct tests are now qualified; a new
single 146-test full-suite run is not claimed. Evidence:
`docs/research/dedup-hdr-key-semantics-qualification.json`.

## Explicit untagged-linear sRGB primaries

`DecodeRequest::assume_untagged_linear_srgb` is a separate opt-in for samples
already known to have a linear transfer but unspecified primaries/white point.
It defaults to false. The caller may set it only when the source's linear samples
are known to use sRGB primaries and D65. It does not apply a transfer curve, clip
HDR, reinterpret encoded unknown samples, replace embedded ICC/declared color,
or bypass RAW camera-color development. The original `assume_untagged_srgb`
flag retains its distinct encoded TIFF/PNM/TGA meaning.

The PFM native decoder and shared raster color selection honor the new request
field only for `LinearRgbUnspecified`. Explicit requests propagate through exact
pixel discovery/indexing, local comparison and fingerprinting. Root convenience
APIs keep strict default requests; prepare explicit per-file requests when a
source-color assumption is known. Rust clients using request struct literals must
include the new field; `DecodeRequest::new` supplies its false default.

Four authored PFM files qualify exact discovery: opposite byte orders match,
sub-byte RGB changes differ, and advisory sample-scale changes differ. Indexing
selects one confirmation pair, matching exhaustive direct pixel equality. The
normalized view independently retains `[-0.1, 2.0, 0.1234, 1.0]` without a transfer
curve or clipping. Without the opt-in, all untagged-linear color errors remain
explicit. A source-policy unit test preserves encoded/declared/ICC metadata.

File-fingerprint recipe v4 includes both assumption flags. An explicit-linear
cache result can be reused under the same policy, but cannot satisfy a strict
request for the same source. Source hashing/snapshots, decode budgets and
cancellation still bracket normalization. Broader non-sRGB primaries, independently
captured HDR/RAW color and large-collection performance remain unqualified.

Explicit-linear validation: all 147 dedup full-feature tests pass, plus the
decoder policy unit and four targeted PFM tests. Crate Clippy and diff checks
pass. Evidence: `docs/research/dedup-untagged-linear-srgb-qualification.json`.

## Per-file requests in recursive canonical pixel indexing

`scan_indexed_pixel_roots_with_requests` accepts a request factory for each
physical discovered path. Callers can supply an explicitly known linear sRGB
assumption, selected frame and generation token independently for each file.
The factory must preserve the discovered path; substitutions fail before index
processing so aliases and reported identities remain tied to discovery. Invalid
limits reject before invoking the factory, and cancellation is checked around
request construction. The original root API delegates with strict default
requests.

A nested/overlapping-root PFM fixture qualifies strict color errors versus an
explicit-linear equal pair. Unavailable selected frames remain per-file errors;
stale per-file generation tokens cancel without a report. Tests also reject path
substitution and invalid limits without invoking the factory. This closes the
per-file request wiring for recursive canonical pixel indexing; it does not
extend source-color assumptions to unsupported primaries or prove real HDR/RAW
color calibration.

## Exact local-descriptor index and indexed mutual matching

`LocalDescriptorIndex` retrieves exact minimum BRIEF distances over the stored
recipe variants using the existing four-channel metric-index union. All stored
right-side variants participate; quarter-turn/oriented queries use their canonical
left descriptor, while multiscale queries use all four. The internal fingerprint
adapter uses only its metric storage; no pixel-information claim is exported.
Entries must share one recipe, IDs remain opaque, and results are ordered by
(distance,id) with explicit entry/result limits and cancellation.

`match_features_indexed` preserves exhaustive mutual unique-nearest matching and
the strict 3/4 ambiguity ratio. The radius `floor(4*max_distance/3)` includes every
neighbor capable of changing an accepted ambiguity decision. More distant missing
neighbors cannot change acceptance. Reverse lookup explicitly transposes the
original directed variant metric rather than assuming arbitrary supplied variant
arrays form a symmetric rotation group. Feature limits and total directed-radius
hits are bounded; metric-node visits are not included in the hit count.

The independent exhaustive oracle qualifies three recipes, four query seeds,
eight radii, 65 IDs including `u64::MAX`, collision buckets and mixed-recipe/error
conditions. Mutual matching agrees with the exhaustive implementation across
three recipes and eight thresholds, including duplicated features and an explicit
asymmetric-variant case. Native features extracted from the rotation/scale PNG
fixture retain the entire exhaustive correspondence list. Cancellation and entry,
result and hit-limit failures never return partial matches.

These are descriptor retrieval/matching primitives. They are not yet wired into
whole-collection transformed-file candidate discovery, do not prove geometry or
pixel equality, and have no measured large-corpus latency/memory qualification.
Existing exhaustive local file searches remain unchanged. Evidence:
`docs/research/dedup-local-descriptor-index-qualification.json`.

Local descriptor-index validation: all 153 full-feature tests pass, all five
focused descriptor-index tests pass, and crate Clippy and diff checks pass.

## Descriptor-indexed transformed-copy collection qualification

`scan_local_collection` extracts selected-frame local features once per admitted
file, retrieves necessary descriptor pairs, and applies mutual matching and
similarity geometry before scheduling fresh bidirectional pixel verification.
All source snapshots are taken before extraction; final checks remove changed
sources and their edges. Cancellation and generation changes discard the report.
Insufficient features and decoding failures are exposed rather than interpreted
as uniqueness. Uniform images still require the separate exact-pixel search.

The controlled four-file crop/scale/rotation collection produces 1,395 features,
44,837 descriptor hits and six proposed pairs. Geometry schedules three pixel
verification pairs, compared with six exhaustive pairs; both methods retain
exactly the same three positive edges. Independent descriptor pair retrieval
oracles cover three recipes, four radii and three shared-feature thresholds.
These counts establish fixture equivalence, not measured latency improvement.
Feature/index allocations, large-collection performance, real captured negatives,
whole-animation transformations remain unqualified. See
`docs/research/dedup-local-collection-qualification.json` for exact source hashes.

Recursive transformed-copy search now uses the shared physical discovery layer
through `scan_local_collection_roots` and its per-file request factory variant.
The factory preserves the discovered path and forwards selected page, color and
generation settings. The root integration fixture qualifies overlapping roots,
Unix hardlink aliases, corrupt-file isolation, insufficient features, unavailable
pages, substituted paths, invalid-policy ordering and stale generations. It does
not qualify non-Unix physical identity or measured large directory performance.
See `docs/research/dedup-local-root-qualification.json`.

Natural-photo collection equivalence is qualified on eight camera-preview files:
four existing scenes and their authored crops. Indexed and exhaustive filtered
search retain exactly four within-scene edges and reject every cross-scene pair,
with no file/pair issues. The index yields 3,381 features, 581,291 descriptor hits
and 28 proposals; geometry reduces fresh pixel-verification scheduling from 28
to four. This is existing-scene qualification, not held-out calibration, captured
burst negatives, RAW/raster equivalence or measured comparative performance.
See `docs/research/dedup-natural-collection-qualification.json`.

Descriptor-indexed two-file collection search now reproduces the constrained
pair-comparator refusals for all 24 authored 40%/70% partial-copy collages:
20 have geometry but fail pixel residuals, four have no geometry, zero are
accepted and zero become unknown errors. The indexed evidence preserves the
same geometry/filter presence and the chosen fitting bounds remain unchanged.
This does not qualify captured bursts or a full all-cross-collage collection.
See `docs/research/dedup-indexed-collage-qualification.json`.

Indexed recursive pixel discovery preserves the exact byte representation of an
unavailable non-UTF8 root in traversal diagnostics while retaining all three
healthy pixel-equal edges in an adjacent three-file collection. Qualified on
macOS with the Unix path API; this does not establish existing non-UTF8 filenames
on Linux, non-Unix path behavior or unreadable-permission handling. See
`docs/research/dedup-indexed-path-diagnostics-qualification.json`.

Actual mode-000 read denial is now qualified on the unprivileged macOS host for
exact-file scanning and recursive canonical-pixel indexing. Both preserve healthy
copies, expose the inaccessible file as an error and recover all three members
when mode-600 reading is restored. The pixel report excludes the inaccessible
file from analysed IDs and retains the one healthy equal edge. Privileged test
runners can bypass mode bits and do not qualify denial; Linux and Windows ACL
behavior and permission changes inside underlying decoders remain open. See
`docs/research/dedup-permission-qualification.json`.

Linux exact-file execution now passes 12 tests in an isolated aarch64 Linux
container as uid 1000, using unchanged exact implementation source with a
standalone no-decode manifest. Actual non-UTF8 filenames, hardlinks, symlink
cycles, FIFO replacement and permission denial/recovery are exercised. The first
run exposed coarse/coalesced tmpfs timestamps on rapid same-size writes; the
metadata-observable mutation fixture now explicitly changes modified time.
Metadata-invisible same-size changes in sample-pruned exact branches remain an
open correctness qualification gap, not resolved by that fixture adjustment.
Native decode integration on Linux and Windows behavior remain open. See
`docs/research/dedup-linux-filesystem-qualification.json`.

Exact duplicate groups now revalidate complete content against actual initial
full digests before final admission, independently of metadata and test-injected
grouping digests. A mismatching representative is similarly reread and removed
if changed, allowing healthy copies to be compared again. A metadata-indistinguishable
unit injection and checkpoint mutation sweep pass on macOS (7 unit + 11 exact
integration tests) and unprivileged Linux (7 + 12). Extra reads are included in
`bytes_read`. This closes the final positive-group and stale-representative gaps;
metadata-invisible changes in size/sample-pruned branches and writes after the
last observation remain outside this proof. See
`docs/research/dedup-exact-final-content-qualification.json`.

Independent full-resolution RAW/raster evidence now exists for four pinned
DNG/NEF/ORF/CR2 sources, comparing native development with separately rendered
LibRaw 0.22.2 TIFFs (camera WB, no auto brightness, sRGB, linear interpolation,
16-bit). The raster input explicitly declares untagged sRGB. Global distances
are 60, 20, 32 and 106; a radius-64 candidate gate would miss the CR2 positive.
This is an exposed development/normalization gap, not resolved by widening a
threshold. Local geometry/pixel qualification and independent color correctness
remain open. Managed reservations return to zero. See
`docs/research/dedup-independent-raw-raster-qualification.json`.

CR2 discrepancy diagnostics retain a canonical unquantized sample accessor and a
controlled optional highlight-recovery ablation in `raw_raster_probe`. Disabling
highlight recovery leaves the independent-render global distance unchanged at
106, so this switch alone does not resolve the known missed positive. Both runs
select identity orientation; active crops differ, and sampled red/blue means
show larger deviations than green. No WB/matrix/demosaic cause has been proven.
Four linear-view tests verify HDR/negative samples, transparent canonicalization
and bounds; the probe passes Clippy and releases managed reservations. See
`docs/research/dedup-cr2-render-diagnostics.json`.

Fresh CR2 native/LibRaw unpack comparison establishes all 51,008,256 sensor bytes
are identical (SHA-256 pinned), with matching CFA, white level, normalized WB
and camera-to-RGB coefficients within 1e-6/1e-7. This locates the distance-106
RAW/raster discrepancy downstream of unpacking and those color coefficients.
Black-level mapping, demosaic, active crop and raster color conversion remain
unresolved candidate causes. It does not establish rendered-color correctness.
See `docs/research/dedup-cr2-sensor-color-diagnostics.json`.

CR2 container isolation establishes independent LibRaw TIFF/PPM integer RGB
payloads are identical after endian normalization. Native RAW distance is 106
against profiled TIFF but zero against explicitly assumed-sRGB PPM. The TIFF
ICC stores a single gamma-1.93359375 curve, differing from sRGB transfer. Native
32x32-grid ICC means agree with independent Little CMS 2.19 within 4.6e-5.
This argues against a gross ICC conversion defect and does not justify ignoring
the declared profile. It exposes fingerprint sensitivity to a known same-source
render with approximated profile transfer; local verification remains necessary.
Grid means are not full-pixel ICC qualification. See
`docs/research/dedup-cr2-container-icc-isolation.json`.

Full-resolution CR2 versus independent LibRaw PPM now yields 45 mutual local
correspondences and a similarity transform with translation approximately
(11.93, 9.77), agreeing with the (12, 10) crop offset. This is geometric evidence,
not final pixel acceptance. ICC TIFF yields zero correspondences. A new bounded
`extract_oriented_pyramid` primitive shares area reduction/source mapping with
the dyadic quarter-turn pyramid; five levels yield 2,296/2,234 features but still
zero TIFF correspondences at the unchanged descriptor policy. Scale expansion
alone does not resolve transfer robustness. Three pyramid tests and selected
Clippy checks pass. File comparison/collection APIs do not yet use this new
primitive. See `docs/research/dedup-raw-local-pyramid-qualification.json`.

The CR2 ICC diagnosis now has an independent per-pixel regression fixture, not
only grid averages: 1,024 RGB16 samples, the generated LibRaw profile and Little
CMS 2.19 linear-sRGB references are pinned with checksums. Native conversion
maximum RGB error is 0.0004061 (mean 0.00002128), no channel differs by 0.001;
the 0.0005-bound regression and selected Clippy checks pass. This qualifies the
specific profile/samples and argues against gross coordinate/color corruption;
it does not solve local transfer robustness or qualify other profiles. See
`docs/research/dedup-icc-pixel-oracle-qualification.json`.

`extract_rank_oriented` adds a separate ordinal-luminance oriented-scale recipe,
without modifying source pixels or existing intensity recipes. Controlled opaque
neutral power transfers 0.5/2/3 preserve all feature positions/descriptors and
mutual correspondences; unrelated texture remains below ten correspondences.
Independent index/matching/file-pair oracles now cover four recipes, including
ordinal scale variants. Seven focused tests and selected Clippy checks pass.
Full-resolution CR2/ICC TIFF yields only one correspondence (496/494 features)
and no geometry at unchanged thresholds: ordinal luminance alone does not close
that missed positive. Crop/RGB-transfer robustness, sorting cancellation latency,
managed allocation completeness and default pipeline integration remain open.
See `docs/research/dedup-ordinal-feature-qualification.json`.

Known-crop coordinate diagnostics locate a detector-coverage gap in CR2/TIFF:
none of 497 selected RAW corners has a selected TIFF corner within two pixels of
the independently known (12,10) translation. PPM has 60 such corners, descriptor
distances 5..56, and 40 geometric inliers. Thus existing strongest-corner selection
already discards the expected ICC-position overlap; descriptors at unselected
coordinates remain unqualified. The expected shift is an oracle input used only
for diagnosis, not a production matching assumption. Selected probe Clippy
passes. Spatially balanced/adaptive selection and negative controls remain work.
See `docs/research/dedup-cr2-corner-selection-diagnostics.json`.

Spatially bounded oriented corner selection (`extract_spatial_oriented`) now
recovers the full-resolution CR2/profiled-TIFF geometry missed by global strongest
selection: a 16x12 grid, at most three corners/cell and 500 total yields 491/494
features, 104 mutual correspondences and 104 inliers, translation (12.03,9.98).
Match distance 64, geometry tolerance 2 and ten-inlier minimum are unchanged;
known crop translation is diagnostic-only and does not constrain matching.
Another real scene yields zero correspondences/geometry; spatial self/unrelated
texture controls and quota/admission/cancel checks pass. Eleven focused primitive
checks and 18 existing local/file/collection tests pass. File/collection APIs do
not yet select this primitive; independent bidirectional pixel qualification and
held-out corpus calibration remain open. See
`docs/research/dedup-spatial-selection-qualification.json`.

### Optional spatial file comparison: full-resolution pixel evidence

`compare_local_files_spatial_with_policy` now selects the spatial feature grid
explicitly while using the same snapshot, decode, bidirectional pixel verification
and latched cancellation pipeline as existing file comparisons. Existing defaults
are preserved. The collection feature stage still needs this explicit selection.

The CR2/independent ICC TIFF pair produced 104 geometric inliers. Strict verification
compared all 24,000,000 RAW-grid pixels and 23,985,854 TIFF-grid pixels; respectively
21,430,992 and 21,431,685 passed tolerance 0.03. The unchanged 90 percent requirement
rejects this pair. Maximum linear channel errors exceed 1384, so development outliers
and explicit photometric evidence remain unqualified; geometric overlap alone is
not acceptance. Managed reservations returned to zero, with peak 965,485,168 bytes;
this does not include every physical allocation.

Validation: 5 collection and 11 file tests passed, including spatial source mutation,
final cancellation, invalid policy, unrelated image rejection and pixel evidence.
Selected library/test/example Clippy passed. Evidence:
`docs/research/dedup-spatial-file-pixel-qualification.json`.

### Spatial selection shared by collection retrieval and verification

`scan_local_collection_spatial` accepts an explicit grid without changing the
existing collection policy or default entrypoint. The extraction stage and fresh
file-pair verification share the same selector. Invalid grids reject before
consuming input; resource, snapshot and cancellation checks remain shared.

An independent exhaustive six-pair comparison on the crop/scale/rotation/unrelated
fixture set produces the same accepted edges as descriptor-indexed spatial search.
The unrelated image has no accepted edge. This validates retrieval completeness
for this fixture set only; real RAW collection and recursive spatial entrypoints
remain to qualify. Six collection and eleven file regressions passed, and selected
library/test Clippy passed.

### Full-resolution RAW/TIFF unfiltered photometric diagnostic

The explicit `raw_raster_probe --verify-spatial-photometric` mode applies the
existing unfiltered linear fit with gains [0.2, 5], offsets bounded by 0.1,
minimum variance 1e-5, 1000 samples and residual tolerance 0.03. The same CR2/TIFF
pair fails with `Pixels(Fit(OutsidePolicy { channel: 0 }))`. Managed reservations
return to zero even on that failure (peak 965,485,168 bytes). The example preserves
this error and exits unsuccessfully rather than labeling it a duplicate. Selected
example Clippy passes. This is negative qualification of the existing model for
one pair, not proof of a particular cause or of general RAW coverage.
Evidence: `docs/research/dedup-spatial-photometric-qualification.json`.

### Full-frame range audit corrects the outlier diagnosis

The optional `RRRAH_FULL_RANGE=1` probe scans every RGB pixel for extrema,
coordinates, means and counts above 1/2/10/100. On the CR2/TIFF pair, native RAW
ranges from -11.2865 to 18.1781 across channels. The normalized embedded-ICC TIFF
instead has minima [-1383.7443, -1382.9362, -1382.843], all at y=0, while its maxima
are [1.60964, 0.999989, 1.10427]. Thus the extreme residual cannot be attributed
solely to RAW development; TIFF color normalization needs boundary qualification.
The prior 32x32 independent ICC grid omitted these extrema and is insufficient
for full-frame correctness. No clipping, profile bypass or threshold change was
applied. Evidence: `docs/research/dedup-full-color-range-diagnostics.json`.

### ICC gamma endpoint correction with independent regression

Boundary RGB16 samples from the independent render reproduce the bug directly:
native red -1383.7443 versus LittleCMS -0.00007171535. Source ICC single-entry
curveType gamma now becomes its mathematically equivalent parametric type 0
before transform construction, avoiding moxcms 0.8.1 pure-gamma approximate
evaluation at zero. No clipping or profile bypass is introduced. Two independent
1024-pixel ICC grids and six existing decoder color tests pass.

Full TIFF extrema now stay within [-0.0004414, 1.000532] across channels; global
RAW/TIFF hash distance changes from 106 to 0. Strict spatial verification accepts
21,474,249/24,000,000 and 21,473,013/23,996,128 pixels, still below the unchanged
90 percent threshold. Unfiltered photometric fitting now succeeds, but accepts
only 13,401,077 and 10,924,600 pixels, so still rejects the pair. Both modes release
managed reservations. Demosaic/highlight/render differences remain unqualified.
Evidence: `docs/research/dedup-icc-gamma-endpoint-qualification.json`.

### ICC endpoint fix across four pinned camera renders and cache identity

Re-running the same checksummed full-resolution DNG, NEF, ORF and CR2 inputs
against the same independent embedded-ICC TIFFs gives global distances 0, 0, 6, 0
instead of 60, 20, 32, 106. This establishes improved candidate evidence across
four cases, not decoded-pixel equality or calibrated acceptance. All source and
render hashes match the earlier qualification. The fingerprint cache recipe
changes from v4 to v5 because color-normalized outputs changed; old cached
fingerprints must not survive the correction.

Validation: 2 independent ICC grids, 6 collection and 11 local-file tests, plus
3 cache and 7 decode tests passed (29 total). Evidence:
`docs/research/dedup-icc-fixed-camera-corpus.json`. Broader full-pixel RAW/render
qualification, held-out negatives, and measured large collection search remain
open in the original acceptance matrix.

### Full-resolution strict RAW/render evidence after ICC correction

The unchanged spatial/strict file policy now accepts three independent render
pairs: DNG 12,043,261/12,212,224 and 12,042,307/12,206,993 matched pixels;
NEF 24,248,274/24,300,900 and 24,268,913/24,321,024; ORF
15,318,542/15,925,248 and 15,292,440/15,909,124. CR2 remains rejected at about
89.5 percent. The unrelated DNG versus NEF TIFF has no verified geometry and
is rejected. Full source/render hashes match the earlier independent corpus.
These four known pairs plus one negative do not qualify all camera/render cases.

A persisted pre-fix v4 fingerprint cache is independently constructed, saved
and reopened in a regression: the first v5 lookup misses and recomputes, while
the second hits. All eight decode integration tests and selected Clippy pass.
Evidence: `docs/research/dedup-strict-raw-camera-qualification.json`.

### Real spatial collection integration

The native probe now explicitly supports collection verification via
`RRRAH_VERIFY_COLLECTION=1`; it uses the same search/spatial policy as pair mode.
Three two-file full-resolution runs match independent pair decisions and pixel
counts: DNG/TIFF accepts (988 indexed features), unrelated DNG/NEF TIFF rejects
without geometry or fresh pixel verification (599 indexed features), CR2/TIFF
rejects despite geometry (986 indexed features). No source/file/pair issues were
reported, and managed reservations returned to zero each time. Selected example
Clippy passes. This is integration evidence on three two-file collections, not
large-scale or full-camera coverage.
Evidence: `docs/research/dedup-real-spatial-collection-qualification.json`.

### Distinct-camera scene negative matrix after ICC fix

All six unordered combinations of four camera scenes, directed from each earlier
RAW to the later independent TIFF, reject with no verified geometry. No decode
errors occur, and managed reservations return to zero. The same spatial/strict
policy as positive qualification is used. Reverse directions, close-looking
scenes, captured bursts and held-out calibration remain untested. Evidence:
`docs/research/dedup-raw-cross-camera-negatives.json`.

A complete all-feature regression is still running in this observation; no full
suite success is claimed from its passing prefix.

### Recursive explicit spatial collection entrypoint

`scan_local_collection_roots_spatial_with_requests` now routes discovered files
through the same spatial selector for retained features and fresh pair checks,
while preserving caller frame/color/generation settings and physical discovery
identities. Existing default entrypoints retain their signatures. Invalid grids
reject before request creation; a factory that substitutes a discovered path
returns InvalidPolicy.

Seven collection tests pass, including nested equal copies plus corrupt-file
diagnostics, explicit source-color declaration and substituted-path rejection.
Selected library/test Clippy passes. Large recursive real-camera collections,
spatial selected-frame animation cases and non-Unix paths remain unqualified.
The earlier complete all-feature regression has progressed beyond collage tests
and is still running natural-photo tests; it precedes this added entrypoint/test.

### Full all-feature regression after ICC and cache correction

`cargo test -p rrrah-dedup --all-features` exited successfully: 172 unit/integration
tests, zero failed/ignored/filtered, and zero doctests. It includes the independent
ICC endpoint fixtures and persisted old-recipe cache rejection. The recursive
spatial entrypoint was added while this run executed; its seven collection tests
and selected Clippy passed separately, so this is not a repeated full run of that
last change. Complete coverage remains unproved against the original matrix.
Evidence: `docs/research/dedup-full-after-icc-qualification.json`.

### Collection cancellation checks after live profiling

The collection cancellation closure now retains references only to supplied
generation tokens, rather than traversing every request for each inner-loop
check. All supplied tokens are still checked on each invocation; user cancellation
and the one-shot latch remain unchanged. Seven collection tests and selected
library/test Clippy pass. No performance improvement is claimed yet.

A second one-second sample of the still-running eight-file baseline shows
`match_features_indexed`/`nearest_rows` during pair preverification, in addition
to earlier descriptor retrieval. Source extraction and fresh pixel verification
remain separate stages; these snapshots do not establish whole-run timing shares.

### Eight-file camera collection result and pair preverification cost

The combined four-RAW/four-independent-TIFF collection completes successfully:
3187 indexed features, 3,311,185 descriptor hits, 28 proposed pairs and four
fresh geometry/pixel verifications. Accepted edges are DNG/TIFF, NEF/TIFF and
ORF/TIFF; CR2/TIFF has geometry but fails strict residual acceptance. All other
cross-scene edges reject without geometry, and no file/pair/source issues occur.
Managed reservations release to zero; peak is 973,130,992 bytes, excluding
untracked physical allocations. Evidence:
`docs/research/dedup-eight-file-camera-qualification.json`.

For already retrieved pairs admitted under the existing exhaustive comparison
cap, preverification now uses the equivalent direct mutual-nearest/ratio matcher
instead of rebuilding pair-local indexes. Collection descriptor retrieval remains
indexed; limits, recipes, geometry and pixel thresholds are unchanged. Seven
collection and six descriptor-index oracle tests pass, and selected Clippy passes.
A saved prior executable and current executable are undergoing sequential paired
runtime/output comparison. No speedup or full repeated-suite claim is made yet.

### Photo regressions after direct pair preverification

All three collage and seven natural-photo tests pass after replacing pair-local
index rebuilding with the equivalent direct matcher under the same comparison
budget. Coverage includes filtered positives, partial-copy residual rejection,
indexed collection equality, strict evidence retention, late source/generation
changes and cancellation. Observed durations were collected alongside another
CPU workload and do not prove a speedup. Evidence:
`docs/research/dedup-direct-preverify-photo-qualification.json`.

### Exact black and alpha endpoint under camera gamma ICC

A further regression uses the real camera-render ICC profile on exact black
RGB16 pixels with alpha zero, half and one. Prepared RGB remains exactly zero,
and alpha preserves the expected normalized f32 bits without color fitting.
All three ICC tests and selected Clippy pass. This complements the independent
LittleCMS sample grids; it is an endpoint invariant, not broader profile coverage.

### Before/after eight-file output comparison completed

Saved prior/current executables both finish successfully on the same eight files,
and their complete reports are byte-identical, including all 28 pair decisions,
pixel residuals and managed peak. Observed times are 225.8057 and 97.8074 seconds.
The earlier run overlapped photo regressions and the later run overlapped brief
ICC compilation; fixed order and one observation per variant also confound timing.
Therefore this proves report equivalence for this corpus and suggests a useful
optimization, but does not establish an isolated general speedup.
Evidence: `docs/research/dedup-eight-file-paired-runtime.json`.

### Fallible detector buffer growth

Corner, suppression-coordinate and output-feature buffers now reserve each
capacity increment fallibly before insertion, requesting no more elements than
the existing candidate/feature caps. Allocation failures return LocalError::Budget
and discard extraction rather than relying on infallible Vec growth. Existing
feature selection, sorting, recipes and matching are unchanged. Fourteen local,
spatial, descriptor-index, ordinal and pyramid tests pass; selected library Clippy
passes. This does not account these buffers against MemoryBudget or prove complete
physical memory limits; allocator overhead, retained frames and sorting cancellation
latency remain open.

### Full-resolution filtered CR2/TIFF outcome

The explicit 3x3 encoded-sRGB constrained fit completes within its declared
900-million sample-pair cap and rejects the pair. It compares 23,980,004 and
23,976,132 complete windows; 10,726,673 and 10,978,880 pass tolerance 0.03
(about 44.7/45.8 percent). Neither channel fit required boundary constraints.
Strict linear evidence remains exposed separately. Managed reservations return
to zero, with peak 965,485,168 bytes. This shows the current global affine color
model is insufficient for this pair; no threshold was relaxed. Saturation and
nonlinear-render effects require independent diagnosis and robust-model fixtures.
Evidence: `docs/research/dedup-cr2-filtered-full-qualification.json`.

### CR2 exact crop translation diagnostic

The explicit `--verify-known-shift` probe uses independently established crop
translation [12,10] solely as an oracle, with strict tolerance 0.03. Both directions
compare exactly 24,000,000 pixels and match 21,433,928 (about 89.3 percent), with
maximum channel error 17.3147. Thus removing estimated geometric perturbation
does not rescue the unchanged 90 percent threshold. This does not authorize using
known metadata shifts as production geometry evidence; development/render differences
remain to diagnose. Managed reservations release to zero and selected Clippy passes.
Evidence: `docs/research/dedup-cr2-known-shift-pixels.json`.

### Saturation-partition diagnostic before robust fitting

Exact metadata-aligned full-frame residuals partition into 22,756,604 samples
with both RGBs in [0,1], 1,227,903 with native RGB outside range, and 15,493
remaining target-out-of-range samples. Rejections at 0.03 are respectively
1,338,571, 1,213,742 and 13,759. The native-out-of-range group dominates squared
error although only about 5.1 percent of pixels. Diagnostic unsaturated affine
gains are [1.03796,1.04545,1.02249] and offsets about [-0.01083,-0.01177,-0.01025],
whereas native-out-of-range gains are [0.18658,0.36157,0.22896] with offsets above
0.54. Thus a single least-squares fit combines substantially different regimes.
No samples were excluded from residual evidence or production acceptance.
An explicit bounded fitting domain and independent saturation/lookalike fixtures
remain to implement and qualify. Evidence:
`docs/research/dedup-cr2-residual-partition.json`.

### Explicit bounded fit domain retains all residuals

`verify_photometric_bidirectional_in_range` and `FitSampleRange` now expose an
explicit inclusive straight-RGB fitting range, with the range attached to evidence.
All overlapping residual samples remain checked, including saturated pixels;
alpha is never fitted. Existing APIs retain unrestricted fitting. An authored
20-pixel clipping case fits 19 samples but compares all 20, exposing the saturated
pixel's error 3.0. Edits, alpha, cancellation, invalid range and insufficient fit
samples are qualified. Ten photometric/filter/warp tests and selected Clippy pass.

On full CR2/TIFF with exact metadata crop translation, [0,1] fitting uses
22,756,604 samples and matches 21,548,605 and 21,548,319 of 24,000,000 residual
samples (about 89.8 percent), still below the unchanged 90 percent requirement.
No clipping or acceptance fallback is introduced. The primitive is not yet
connected to file/collection comparison. Evidence:
`docs/research/dedup-fit-range-qualification.json`.

### Bounded fitting domain connected to file and collection search

`LocalComparisonMode::RangePhotometric` now routes standard and spatial file
comparison through the bounded-fit primitive, preserving strict residuals and
exposing `photometric_fit_range`. Indexed collection and recursive searches use
the same selected mode; preverified geometric rejections also expose the policy.
Invalid ranges reject before source/input consumption. Existing defaults and
source/cancellation gates are shared. Twenty-three file/collection/photometric
tests and selected library/test/example Clippy pass.

The real CR2/TIFF file comparison with estimated geometry still rejects at the
unchanged 90 percent requirement and releases managed reservations. Thus this
integration does not claim to solve nonlinear rendered saturation. Evidence:
`docs/research/dedup-file-range-qualification.json`.

### Grayscale scratch admission in shared file feature extraction

Standard and spatial file/collection extraction now reserve width*height*8 bytes
for the f64 grayscale plane through MemoryBudget. Detector and oriented stages
use that plane sequentially, so one reservation spans both. Overflow/pixel bounds
reject before admission, and RAII releases the reservation on errors/cancellation.
A 4200-byte fixture with another 1001-byte owner rejects the 3200-byte plane;
with a 1000-byte owner it admits, reaches peak 4200 and releases scratch while
preserving the other owner. Cancellation has the same release behavior.
Twenty targeted file/collection/ownership tests and selected Clippy pass.
This does not yet account corner, retained feature or index storage, allocator
overhead or all decoder buffers. Evidence:
`docs/research/dedup-feature-gray-memory-qualification.json`.

### Retained feature storage admission

Shared file and collection extraction reserves conservative feature storage before extraction and keeps its credit until the last shared feature owner is dropped. Temporary feature and grayscale reservations release on error and cancellation. The selected library/file/collection/index suites pass 35 tests; library Clippy passes. Evidence: `research/dedup-retained-feature-memory-qualification.json`. Detector corner/suppression arrays and index copies/storage still lack admission; this does not establish a complete memory or RSS bound.

### Shared feature handoff into collection retrieval

Collection retrieval now shares retained feature storage instead of copying every feature into an additional Vec. Descriptor entries are streamed rather than accumulated in another feature Vec; file and feature ordering is preserved. The public Vec-based retrieval API remains compatible. Eight collection and six index tests pass, including exhaustive retrieval and accepted-pair equivalence, source mutation and cancellation; library Clippy passes. Evidence: `research/dedup-shared-index-handoff-qualification.json`. Fingerprint and metric-index storage still need budget admission; no RSS or timing gain is claimed.

### Cancellation during metric-index construction

`HammingIndex::insert_with_cancel` checks cancellation inside tree traversal and collision-bucket traversal before modifying storage. Existing uncancelled insertion remains compatible; VisualIndex construction uses the cancellable path. Independent adversarial fixtures cancel inside a 1000-ID bucket and a 64-node one-bit chain, verify unchanged search results, then verify successful retry. The selected library/visual/local-index/collection suites pass 28 tests and selected Clippy passes. Evidence: `research/dedup-index-insertion-cancellation-qualification.json`. Allocator calls remain indivisible, and complete allocation/decoder-latency qualification remains open.

### Fallible index construction vectors

Descriptor fingerprints, collection slots and insufficient-file IDs, visual-index slots and accepted-pair materialization now use fallible bounded vector growth. Pair result limits are enforced before each push, and cancellation is checked during materialization. Eight collection, six local-index and four visual tests pass; library Clippy passes. Evidence: `research/dedup-index-fallible-vector-qualification.json`. Tree/map/set allocations still require separate handling and memory admission; this is not complete allocation qualification.

### Fallible metric-tree construction and search

`HammingIndex::try_insert_with_cancel` caps total stored IDs and fallibly reserves node, collision-ID and sorted child-edge vectors. Child distances are in 1..=64; binary-search lookup preserves exact metric traversal. `try_search_with_cancel` fallibly grows traversal and result vectors, enforcing result limits without partial success. VisualIndex uses both fallible APIs. Cancellation before publication and entry/result-limit failures preserve searchable contents; capacity may still grow before failure. Thirty selected all-feature tests and minimal-feature library/visual tests pass, with selected Clippy. Evidence: `research/dedup-fallible-metric-tree-qualification.json`. Legacy wrappers document panic on allocation failure. Higher-level maps/sets still need fallible handling and shared-budget admission; actual OS OOM, RSS and scale performance are not claimed.

### Fallible visual-index tables and descriptor query results

Visual fingerprint storage and candidate union now reserve hash-table capacity fallibly before insertion. Candidate IDs retain sorted comparison order via a fallibly reserved Vec; accepted results also grow fallibly. Descriptor query unions use fallible hash-table growth, reject excess new result IDs before insertion and retain minimum distances for repeated hits; output reservation is fallible and final ordering remains `(distance, id)`. Thirty selected all-feature tests and library Clippy pass. Evidence: `research/dedup-fallible-visual-tables-qualification.json`. Collection documents/counters/target sets and other higher-level storage still need handling, shared memory admission and measured qualification; actual allocator OOM has not been injected.

### Fallible collection descriptor-pair storage

Descriptor retrieval stores file owners in a fallibly grown sorted Vec, detects duplicate IDs via fallible set growth and uses fallible storage for distinct targets and pair counters. Targets and accepted pairs retain sorted order; pair shared-feature counters use checked arithmetic. A new fixture exercises file, feature, hit, pair-counter and pair-result caps independently and verifies complete retry results. Eight collection and seven local-index tests pass; library/test Clippy passes. Evidence: `research/dedup-fallible-pair-storage-qualification.json`. Scanner snapshot/report storage, other APIs and full shared-budget/memory/performance qualification remain open.

### Current coverage audit and platform drift

`research/dedup-current-coverage-audit.json` retains all 18 original requirements as not fully proved. The previous Linux exact-file qualification remains historical: the test source is unchanged but `exact.rs` differs from its pinned hash. Repeat Linux qualification on current source; do not treat the old result as a current platform pass. The fresh full suite exercises current memory/index changes and existing decoder/color/animation/cache integrations, but does not establish every remaining corpus, platform, resource or performance requirement.

### Current Linux exact-source requalification

The current native `exact.rs`, exact-file tests and ContentSnapshot test pass in a standalone no-decode wrapper on Linux 6.8 aarch64 / Rust 1.98 under UID 1000 on private tmpfs: two source unit tests, one snapshot integration test and twelve filesystem integration tests. All source hashes still match after the run. Evidence: `research/dedup-linux-exact-current-qualification.json` and its saved log. This supersedes the stale-source concern for exact-only Linux qualification, while native Linux decoder integration, Windows and sample/size-pruned invisible mutations remain unproved.

### Verified exact scan covers pruned observations

`exact::scan_verified` obtains full-content baselines for every admitted physical file before staged retrieval and revalidates every baseline before publication, including singleton size and sample-pruned files. Changed members are removed from groups; cancellation clears groups. Existing `scan` remains the staged path. Public stable fixtures preserve groups/aliases and account for extra reads; private metadata-indistinguishable singleton and same-sample middle-edit fixtures detect Changed. Twenty-one minimal-feature macOS tests and seventeen exact-only non-root Linux tests pass on current sources. Evidence: `research/dedup-verified-exact-qualification.json`. Full observations add at least two full reads per file and remain non-atomic; intermediate restored rewrites, scale timing, memory admission and other platform/decoder coverage remain open.

### Exact-file scale measurement with known group oracle

`examples/exact_scale_probe.rs` measures ordinary and verified exact scanning in release builds on 128, 1024 and 8192 synthetic 64-KiB files, with 25% of files in known four-file duplicate groups. Six warmups and 24 timed runs all match independent fixture labels. Four paired repetitions alternate mode order after both modes are warmed; fixture construction and assertions are excluded from timing. Source hashes, timings and actual read counts are in `research/dedup-exact-current-scale-qualification.json`; example Clippy passes. These are one-host warm-cache observations, not cold-cache/large-file/visual/RSS or general performance qualification.

### Verified exact cancellation across observed checkpoints

A two-group public fixture first counts every cancellation callback in a successful verified scan, then replays one-shot cancellation at each observed position. Every replay is incomplete and publishes no groups; uncancelled retry preserves both baseline groups. Thirteen macOS exact integration tests and eighteen Linux exact-only source/snapshot/integration tests pass, including this fixture; selected Clippy passes. Evidence: `research/dedup-exact-checkpoint-cancellation-qualification.json`. This covers the healthy fixture trace, not every error branch or decoder latency.

### Cache publication under competing processes and abrupt exit

Four independent writer processes each publish twenty 64-record caches while the parent loads the destination and validates that every observed version contains all records of exactly one writer. An abruptly exiting writer is tested during partial-record output and after file synchronization before replacement: the original destination remains byte-identical; partial orphan loading rejects, completed orphan loading succeeds without publishing it, and a later save recovers. The cache suite passes six test functions (including the child helper), and selected Clippy passes. Evidence: `research/dedup-cache-process-qualification.json`. This is macOS process-exit evidence, not power-loss, directory durability, cross-platform cache qualification, automatic recipe identity or orphan reclamation.

### Automatic workspace source/build recipe identity

Fingerprint file cache keys now incorporate a build-generated BLAKE3 identity over all local dedup/decode/core/memory crate files, workspace Cargo manifest/lock, Rust compiler identity, target/build flags and crate features. Existing caller salt/settings remain included. Persisted pre-automatic v4/v5 keys miss while repeated current requests hit; eight decode tests and library/build-script Clippy pass. Actual compiled build-script probes establish relocation invariance, source-change/new-file invalidation and restoration. Evidence: `research/dedup-built-source-recipe-qualification.json`. This covers current workspace builds and conservatively includes tests/assets; packaged layouts, native dynamic libraries, system fonts and other external runtime resources still need explicit identity/qualification.

### Isolated minimal-source recipe build

The recipe builder recognizes complete four-crate workspace context; without it, a no-decode source build hashes its own package under a distinct scope marker. Decode-enabled builds lacking the required source context reject explicitly. An isolated temporary Cargo project with the actual native source/build script, minimal normalized dependencies and transfer-oracle fixture passes 39 library/integration tests. Current workspace decode/cache tests pass eight cases and Clippy passes. Actual compiled build-script probes preserve identity under relocation, invalidate an own-source change and reject missing decoder context. Evidence: `research/dedup-portable-minimal-recipe-qualification.json`. This does not qualify package publication or independent full-decoder dependency identity.

### Explicit display-range projection primitive

`warp::verify_photometric_display_projection` fits only original unclipped opaque samples, projects fitted prediction and observed RGB into an explicit range during residual verification, and returns strict original bidirectional evidence separately. Alpha is preserved; nonfinite fitted values reject before projection. An authored HDR-to-display fixture matches all twenty projected samples while retaining strict error 3 on the clipped sample; alpha edits and fully saturated low-information inputs do not disappear. Eleven photometric/filtered/warp tests and selected Clippy pass. Evidence: `research/dedup-display-projection-qualification.json`. File/collection integration, current CR2, held-out positives/negatives and calibration remain required; display-range agreement is not exact pixel identity.

### Display projection file/collection policy integration

`LocalComparisonMode::DisplayProjection` runs through existing pair/spatial/collection source, decode, geometry and strict original pixel verification, using projected photometric residuals only for the explicitly selected candidate signal. Evidence exposes the chosen range and `display_projection` mode even for no-geometry rejections. A file-based authored crop/scale/rotation/unrelated fixture verifies indexed accepted-edge equivalence with all pair comparisons and range validation before input consumption. Twenty-five local file/collection/photometric tests and selected Clippy pass. Evidence: `research/dedup-display-file-policy-qualification.json`. Actual clipped HDR file, CR2/independent RAW corpus, held-out false-positive calibration and new-mode performance remain required.

### Full-resolution display-projection camera collection

The current native pair pipeline accepts the known independent CR2/TIFF case using explicit `[0,1]` display projection at unchanged tolerance 0.03 and bidirectional matched fraction 0.9: projected fractions are 0.922303 and 0.900390. Strict original CR2 evidence remains rejecting. An unrelated CR2/NEF-scene TIFF rejects geometry/candidacy. The full eight-file DNG/NEF/ORF/CR2 plus independent TIFF collection retrieves 28 pairs, freshly checks four geometric pairs and accepts exactly the four labelled RAW/TIFF pairs; all 24 cross-scene pairs reject, no issues occur and managed credit returns to zero. Evidence: `research/dedup-display-camera-qualification.json` and saved logs; diagnostic example Clippy passes. This is a known model-diagnosis corpus, not held-out calibration, burst/look-alike coverage, arbitrary development or physical memory/timing qualification.

### Native Linux decoder environment attempt

A four-crate source snapshot (420 files) and unchanged dependency lock were staged for a native Linux decode build on Debian 13 / Rust nightly 1.100. Dependency installation stopped with exit 100 before compilation because Docker overlay space is exhausted. Only the failed task-owned installer container was removed. The staged hashes remain intact, but live `rrrah-memory/src/lib.rs` changed independently after staging, so refresh the snapshot before subsequent current-source testing. Recipe, log and hashes: `research/dedup-linux-native.Dockerfile`, `research/dedup-linux-native-environment-attempt.log` and `research/dedup-linux-native-environment-attempt.json`. No native Linux decoder tests ran.

### Current shared-memory source revalidation

Current shared-memory staged allocation delegates `MemoryBudget::try_buffer` to `Reservation::try_buffer`. Thirty-nine dedup library/decode/file/collection tests and thirty-eight memory library/integration tests pass, including retained feature-owner credit, staged child-budget credit, cancellation and mutation handling; dedup library Clippy passes. Evidence: `research/dedup-current-memory-revalidation.json`. A fresh four-crate Linux input snapshot includes current memory source and matches its saved hashes, but Linux native decoder execution is still blocked by environment disk capacity. This is focused compatibility evidence, not a full suite, all-camera rerun or physical-memory qualification.

## Full-suite refresh with concurrent source changes

The all-feature suite completed with 190 passed, zero failed/ignored/filtered and
zero doctests. During the run, `rrrah-decode/src/xcf.rs` and
`rrrah-memory/src/lib.rs` changed independently. This pass therefore does not
qualify the current entire source state; those changes need revalidation.
Evidence: `research/dedup-full-refresh-qualification.json` and its full log.
All 18 acceptance rows remain subject to their stated remaining work.

## Concurrent dependency changes: focused revalidation

After the full-suite refresh, the changed shared-memory and XCF sources were
revalidated: 39 memory tests including documentation, 39 XCF tests and 39 dedup
library/decode/local-collection/local-scan tests passed. All 420 snapshotted
source files remained unchanged during this focused run. One XCF test requiring
an external pinned GIMP fixture was ignored and 700 unrelated decoder tests
were filtered. This is focused compatibility evidence, not a new complete
suite, camera-corpus or platform qualification.
Evidence: `research/dedup-concurrent-source-revalidation.json` and three logs.

## External XCF selected-layer fixture revalidation

The previously ignored selected-layer test passed explicitly with the existing
SHA-256-pinned GIMP fixture. It verifies two layers, a black mask, exact retained
byte accounting and rollback on cancellation after allocation. The source
files remained unchanged. The fixture had been present locally; the earlier
focused command omitted its environment path. This does not prove general XCF
flattening or end-to-end duplicate search for layered documents.
Evidence: `research/dedup-xcf-external-revalidation.json` and log.

## XCF selected-frame duplicate integration

Nine decode integration tests pass, including a new layered-XCF case through
`decode_selected_frame`: identical profiled content with XCF/CR3 names matches;
a 2x1 solid BMP differs; untagged XCF returns a color error; all results/errors
and cancellation release managed memory. This authored two-pixel fixture does
not qualify independent GIMP flattening, arbitrary blends/groups or real edited
document retrieval. Evidence: `research/dedup-xcf-file-integration-qualification.json`.

## Windows physical-identity gap audit

The stable-toolchain implementation still falls back to canonical paths outside
Unix and its existing alias/cycle fixture is Unix-only. Thus Windows hardlinks
are not established as aliases and distinct physical-copy counts are unqualified.
The cached same-file Windows adapter documents 64-bit file-index limitations on
ReFS and live-handle requirements; it must not be substituted without addressing
those cases. Windows native runtime/target validation is unavailable locally.
Evidence: `research/dedup-windows-identity-gap-audit.json`.

## Arbitrary-angle and intermediate-scale indexed integration

Ten collection tests pass with an added independently labelled file-level matrix:
17/-37/63 degree rotations, scales 216/160 and 117/160, plus 216/160 with
17 degrees. In each three-file cohort both exhaustive and indexed APIs retain
exactly the base/derivative edge and reject the two unrelated edges (six positive
and twelve negative comparisons per API). Existing thresholds are unchanged;
managed memory returns to zero. This authored texture matrix establishes these
file-level cases only, not general real-photo angle/scale calibration, held-out
burst/look-alike negatives, perspective or reflection handling.
Evidence: `research/dedup-arbitrary-collection-qualification.json` and log.

## Known natural-photo angle/scale collection qualification

Two focused photo tests pass across three eight-file cohorts of four camera
preview scenes: crop, Pillow 17-degree bicubic rotation with expanded canvas,
and 75-percent Lanczos resize. Each exhaustive/indexed result contains exactly
four independently labelled positive pairs and rejects all 24 cross-scene
pairs. Indexed verification performs four fresh pixel comparisons per cohort;
all 28 descriptor proposals are retained before geometry filtering. The explicit
radius-3 encoded-sRGB filtered photometric policy is unchanged. These are known
preview scenes, not held-out captures, full-resolution RAW development, bursts
or look-alike false-positive calibration. Evidence:
`research/dedup-natural-angle-scale-collection-qualification.json` and log.

## Pyramid result allocation and cancellation

Pyramid aggregation now reserves combined feature storage fallibly before
coordinate remapping and checks cancellation per feature. Six oriented/pyramid
tests pass, including exhaustion after the first level, one-shot late cancellation
and a retry preserving descriptors and positions. Dedup library Clippy with
`--no-deps -D warnings` passes; dependency-inclusive Clippy reports eight existing
rrrah-memory diagnostics. This does not establish actual allocator-exhaustion
injection, full shared-budget admission or measured RSS.
Evidence: `research/dedup-pyramid-admission-qualification.json`.

## Local matching allocation and checkpoint cancellation

Both neighbor tables now allocate fallibly and correspondence growth uses
bounded fallible reservation. Nineteen existing local/oriented/pyramid/collection
tests passed; four local tests then passed with the new four-descriptor fixture
replaying one-shot cancellation at every callback and validating healthy retry.
Dedup library Clippy passes. This does not qualify injected allocator exhaustion,
shared byte-budget admission of all matching tables or OS memory/latency bounds.
Evidence: `research/dedup-matching-admission-qualification.json` and logs.

## Geometry allocation and cancellation qualification

Coordinate uniqueness tables, hypothesis/refinement inlier arrays and fallback
evidence copies now allocate fallibly. Twenty-one existing geometry/local/
collection/pyramid tests pass; all five geometry tests pass after adding one-shot
cancellation at every checkpoint with exact healthy retry. Dedup library Clippy
passes. Hash-table iteration is never used for ranking; deterministic transform
selection and existing refinement oracles remain unchanged. No OS allocation
failure injection, shared byte-budget completeness or RSS measurement is claimed.
Evidence: `research/dedup-geometry-admission-qualification.json` and logs.

## Geometry support-spread cancellation

Both support-spread passes now check cancellation per point; candidate and
refined support checks propagate cancellation rather than publishing partial
evidence. Five geometry tests (including every-checkpoint one-shot cancellation
and healthy retry) and ten collection tests pass, as does dedup library Clippy.
This does not qualify full decoder/native/OS cancellation latency.
Evidence: `research/dedup-geometry-spread-cancellation-qualification.json`.

## Grouping fallible allocation and exhaustive small-graph cancellation

Grouping membership tables, sorted entry/edge buffers, and bounded group/member
vectors now allocate fallibly. Explicit sorting preserves deterministic complete-
link output. Thirty-five grouping/pixel-search/scan/sequence/visual tests pass.
The three grouping tests include all 64 four-node graphs with reversed input
orders/directions, every-callback one-shot cancellation and healthy retry.
Dedup library Clippy passes. Actual allocator exhaustion, complete shared
byte-budget/RSS accounting and large-sort cancellation latency remain unqualified.
Evidence: `research/dedup-grouping-admission-qualification.json` and logs.

## Exact-crop result allocation and cancellation

Placement buffers now grow fallibly within the admitted result count. Seventeen
crop/local/collection tests pass; the three final crop tests include transparent
uniform ambiguity at an exact six-result limit, one-shot cancellation at every
checkpoint, and healthy retry preserving placements and work counts. Dedup
library Clippy passes. This does not qualify allocator-failure injection, shared
byte-budget completeness or captured transformed-crop accuracy.
Evidence: `research/dedup-crop-admission-qualification.json` and logs.

## Pixel-index vector allocation qualification

Candidate buckets, diagnostics, analysed IDs and recursive request preparation
now allocate fallibly; changed-source membership uses fallible hash storage.
Twenty-three pixel-search/raster-equality/scan tests pass, then all fourteen
pixel-search tests pass after the final storage change. Dedup library Clippy
passes. Snapshot/request/bucket BTreeMaps still lack fallible admission; allocator
failure injection and complete shared byte-budget/RSS accounting remain open.
Evidence: `research/dedup-pixel-index-admission-qualification.json` and logs.

## Pixel-index table admission and deterministic ordering

Requests and pixel-key buckets now use fallible hash admission; source snapshots
use a bounded fallible vector. Explicit sorted IDs and digest keys preserve
previous traversal/confirmation order. Thirty-four lib/pixel-search/raster-
equality/scan tests pass; fifteen pixel-search tests then pass with the new
two-key-bucket/four-request-order oracle. Dedup library Clippy passes. Downstream
confirmation maps, request clones and decoder allocations still require review;
full shared byte-budget/RSS and allocation-failure injection remain unproved.
Evidence: `research/dedup-pixel-table-admission-qualification.json` and logs.

## Direct pixel-confirmation allocation admission

`confirm_pixels` now uses fallible request/candidate hash storage, explicitly
sorted pair traversal and bounded fallible result/diagnostic vectors. Twenty-four
pixel-search/raster-equality/scan tests and dedup library Clippy pass.
`confirm_pixel_groups`, other scan APIs, request clones, sorting and decoder
allocation remain separate gaps; full shared budget/RSS and actual allocator
exhaustion injection are not proved.
Evidence: `research/dedup-confirm-admission-qualification.json` and log.

## Group-confirmation allocation admission

Request/token/id/snapshot/diagnostic vectors and duplicate/invalid-source
membership now allocate fallibly. Snapshots are explicitly sorted for the
existing final-validation order. 24 scan/grouping/pixel-search tests and
dedup library Clippy pass, including stale-edge invalidation and final generation
cancellation. Other scanners, allocator-failure injection and full shared
byte-budget/RSS proof remain separate gaps.
Evidence: `research/dedup-confirm-groups-admission-qualification.json` and log.

## Visual selected-frame scan allocation admission

Request hash admission, sorted requests, fingerprint/diagnostic/pair vectors
and analysed output now allocate fallibly. Source ID order is preserved and
final cancellation follows output preparation. Thirty-six decode/pixel-search/
scan/visual tests pass, then six scan tests pass after the final cancellation
change; dedup library Clippy passes. Directory/full-container scanner allocation,
allocator-failure injection and full shared byte-budget/RSS proof remain open.
Evidence: `research/dedup-visual-scan-admission-qualification.json` and logs.

## Directory enumeration and exhaustive pixel-search admission

Discovered-file identifier vectors now allocate fallibly with per-entry
cancellation. Exhaustive selected-frame search uses fallible request/snapshot/
ID/diagnostic/invalid-source storage and preserves sorted source observations.
Twenty-four pixel-search/raster-equality/scan tests and dedup library Clippy pass.
Filesystem discovery storage, path/request clones, decoder allocation and full
shared memory/RSS guarantees remain separate gaps.
Evidence: `research/dedup-directory-admission-qualification.json` and log.

## Whole-presentation comparison allocation admission

Confirmation request/candidate/result storage and exhaustive scan ID storage
now allocate fallibly; sorted candidate pairs preserve comparison order.
Twenty-three animated/container/pages/presentation-kind tests and dedup library
Clippy pass. Grouped/automatic-directory container admission remains open;
exhaustive comparison still performs quadratic repeated decoding. No complete
shared-memory/scaling or actual allocator-failure injection claim is made.
Evidence: `research/dedup-container-admission-qualification.json` and log.

## Whole-container group admission

Grouped presentation request/token/id/snapshot/diagnostic/invalid-source storage
now allocates fallibly. Sorted source IDs preserve observation and validation
order. Thirteen container/grouping/presentation-kind tests and dedup library
Clippy pass. Automatic directory discovery, repeated quadratic decoding, actual
allocator failure and complete shared memory/RSS accounting remain open.
Evidence: `research/dedup-container-groups-admission-qualification.json`.

## Automatic directory presentation admission

Explicit/automatic directory file vectors and automatic admitted/snapshot/
diagnostic/invalid-source storage now allocate fallibly. Content detection and
ordered source observations remain unchanged. Twenty container/pages/
presentation-kind tests and dedup library Clippy pass. Filesystem discovery
internals, clones, decoder allocations, actual allocator failure and complete
shared byte-budget/RSS/scalable-container proof remain open.
Evidence: `research/dedup-auto-container-admission-qualification.json`.

## Ordered sequence frame admission

Frame admission now grows storage fallibly within declared count, mapping
allocation failure to the existing pixel budget error. Twenty-nine sequence/
animation/pages/container tests and dedup library Clippy pass. The new fixture
cancels once at all five admission checkpoints for four pages, returns no
partial sequence and verifies healthy reconstruction. Decoder frame allocation,
actual allocator failure and full shared byte-budget/RSS proof remain open.
Evidence: `research/dedup-sequence-admission-qualification.json` and log.

## Ordered page and metadata allocation admission

Page arrays, TIFF directory/cycle storage, bounded metadata and ICC vectors
now allocate fallibly; fixed ICO/CUR and TIFF pointer headers use stack arrays.
Twenty-two pages/container/raster-equality tests and dedup library Clippy pass.
Actual allocator failure, underlying decoder allocation, metadata capacity
accounting and complete shared memory/RSS guarantees remain open.
Evidence: `research/dedup-pages-admission-qualification.json` and log.

## TIFF metadata and ICC actual-capacity admission

Directory metadata and ICC reservations now cover reported vector capacity
before source reads. Twenty-two pages/container/raster-equality tests pass;
eleven final page tests include fourteen classic-TIFF/BigTIFF budget cases.
Each result/error releases all managed credit, managed peak stays within limit,
and healthy retry recovers three-page metadata. Dedup library Clippy passes.
This is managed-counter evidence, not OS RSS, complete native allocation coverage
or injected allocator rounding.
Evidence: `research/dedup-tiff-capacity-admission-qualification.json` and logs.

## Bounded source-read buffer allocation

Hashing, byte confirmation, prefix and ranged source copies share fallible read
buffer allocation, reporting OutOfMemory I/O errors. Thirty-seven lib/snapshot/
exact/pages tests and dedup library Clippy pass. An impossible-capacity fixture
verifies explicit refusal and healthy small-buffer retry. This does not inject
OS exhaustion or admit read buffers/traversal/maps/path clones under the shared
byte budget.
Evidence: `research/dedup-source-buffer-admission-qualification.json`.

## Current visual descriptor-index scale observation

The release visual-scale probe checks 1k/10k/50k fingerprints with eight
transform variants and radii 0/8/32/128/256. All sixty queries equal exhaustive
candidates. On this single-run uniform-random descriptor fixture, 50k build took
0.588 seconds; four radius-32 queries took 0.069 seconds indexed versus 0.00119
seconds exhaustive, revealing a broad-radius performance gap. These single-pass
synthetic timings do not qualify real-image throughput, RSS or statistical
superiority. Adaptive direct comparison needs evaluation across distributions.
Evidence: `research/dedup-visual-current-scale-qualification.json` and log.

## Full-radius visual retrieval bypass

Radius 256 (including clamped larger radii) now compares all fingerprints
directly: every 256-bit distance is admitted, so metric retrieval cannot prune.
Seven visual tests pass, including every direct-path cancellation checkpoint
with/without transforms and healthy retry; dedup library Clippy passes. All sixty
release scale queries remain exhaustive-oracle equal. On 50k synthetic entries,
four radius-256 queries took 0.00606 seconds versus 0.407 seconds in the earlier
single run. These are individual observed runs, not statistical superiority or
real-image throughput; radius 32..255 remains an unresolved performance gap.
Evidence: `research/dedup-full-radius-scale-qualification.json` and logs.

## Adaptive visual traversal qualification

Visual search now counts tree visits, child checks and collision-bucket emissions
across all four channels. After four operations per admitted entry it discards
partial retrieval and performs complete fingerprint comparison with the original
radius and transform policy. Allocation/result-budget failures and cancellation
remain errors, distinct from work exhaustion. Output ordering remains distance/id.

The 13 library and eight visual tests pass, including dense collision distributions,
independent exhaustive equivalence and one-shot cancellation at every observed
checkpoint with a successful retry. Library-only Clippy passes. The scale probe
checks 60 queries over 1,000/10,000/50,000 synthetic entries against full comparison.
All match. The threshold is a heuristic; timing is one synthetic run, without RSS,
real-photo calibration or statistical confidence. Evidence:
`research/dedup-adaptive-visual-qualification.json` and associated logs.
The full acceptance table remains incomplete.

## Visual distribution scale follow-up

The reusable visual probe now covers random, two-bucket collision and clustered
descriptors at 1,000/10,000/50,000 entries. Each radius has two warmups and four
paired alternating-order runs of four queries. All 810 oracle comparisons agree.
Another 24 grouping/pixel-search/scan integration tests pass. Evidence:
`research/dedup-distribution-scale-qualification.json` and logs.

Collision-heavy construction exposes an independent remaining performance gap:
`HammingIndex::try_insert_with_cancel` scans every existing ID in an equal-hash
bucket for duplicate admission, causing quadratic repeated insertion work.
This baseline does not qualify real-photo throughput, RSS or decoder latency.

## Collision-bucket construction correction

The previously measured linear duplicate-ID admission scan is replaced by
per-hash HashSet membership. Reservations precede insertion, and cancellation
is checked before publication. Existing ID/hash pairs remain no-ops at capacity;
the same ID at another hash remains a distinct searchable entry. Search continues
to sort complete output by distance/id, independent of table iteration order.

Fourteen library and eight visual tests pass, including shuffled IDs, duplicate
admission at capacity, distinct-hash identity and cancelled insertion rollback.
All 810 random/collision/clustered exhaustive oracle queries pass again.
The single 50,000-entry two-bucket construction measurement is 0.0491 seconds,
versus the preceding 44.6757 seconds baseline. This is an observed synthetic
build comparison, not a statistical real-photo speed claim. Hash tables increase
sparse-bucket storage overhead; RSS and shared byte-budget admission remain open.
Evidence: `research/dedup-collision-membership-qualification.json` and logs.

## Process RSS baseline after collision admission change

Nine isolated release-process measurements cover three synthetic distributions
and three sizes. Each process verifies all five radii with warmups and paired
search runs (810 exhaustive oracle comparisons in total). macOS `time -l` reports
peak RSS of 476,102,656 bytes for random 50,000 entries, 91,815,936 for two-bucket
collisions and 100,679,680 for clustered descriptors. These are whole-process
peaks including inputs/oracle/results, not index-only memory or budget accounting.
One run per case does not establish a statistical bound. Sparse singleton hash
buckets are an identified next storage optimization. Evidence:
`research/dedup-process-rss-qualification.json` and raw stdout/stderr.

## Singleton index storage and full-suite refresh

Each metric node now stores its first ID inline and admits a hash table only for
additional equal-hash IDs. Fourteen library/eight visual tests and library-only
Clippy pass. All 810 scale-probe oracle comparisons pass. In a single isolated
50,000-random-entry process, peak RSS is 427,982,848 bytes versus the preceding
476,102,656-byte baseline. This is whole-process RSS including input and oracle
storage, not a statistical index-only bound or shared byte-budget qualification.
Evidence: `research/dedup-singleton-storage-qualification.json`.

The preceding full-suite process completed with 203 passed, zero failed/ignored/
filtered. Its source snapshot was taken after launch and later changed in dedup
`lib.rs`, decoder `ciff.rs` and `crw_entropy.rs`; it is evidence for that build,
not proof of every current source. See
`research/dedup-post-index-full-qualification.json`.

## Current CRW dependency revalidation

Five CIFF tests, nine entropy tests and 31 dedup library/visual/decode tests
pass. Four normally ignored real-fixture tests were explicitly run: D30
structure, 10D auto-WB, D30 complete sensor entropy/managed ownership against
the independent sensor oracle, and 10D low-plane layout. Source and oracle
checksums match the checked-in fixture registry. An initial concurrent entropy
source edit required repeating its nine tests plus both real entropy tests;
the final before/after entropy source hash is unchanged. This is targeted
revalidation, not full developed-color or general duplicate coverage. Evidence:
`research/dedup-crw-current-revalidation.json` and associated logs.

## Overlay collection qualification

Independent Pillow-authored overlays on the pinned CC0 160x160 texture cover
an 800-pixel opaque watermark positive and 6,400-pixel edit negative under the
unchanged 90% matched-pixel threshold. Indexed and exhaustive scanners produce
the same labelled accepted pairs and reject the unrelated texture. Both edited
pairs reach geometry and pixel confirmation; the negative is not merely pruned
by candidate retrieval. All 11 collection tests pass and managed credit returns
to zero. This is authored-fixture evidence, not held-out natural edit/watermark
calibration. Reflection and perspective geometry remain unsupported. Evidence:
`research/dedup-overlay-collection-qualification.json`.

## Reflected similarity geometry foundation

`verify_reflected_similarity` now verifies orientation-reversing similarity
models using a fixed source x reflection plus the existing robust similarity
fit. Returned evidence maps original source coordinates explicitly and retains
original support indices. Hypothesis limits apply to this reflected search,
independently of any separate ordinary search; allocation/work/cancellation
errors remain inconclusive. Six geometry tests and library-only Clippy pass,
including authored reflected support, outliers, original-coordinate mapping,
work exhaustion and every observed one-shot cancellation checkpoint.

This is only a geometry primitive. Mirrored descriptor extraction, pixel-warp
verification and collection integration remain required before claiming reflected
file support. Perspective remains unsupported. Evidence:
`research/dedup-reflected-geometry-qualification.json`.

## Reflected strict pixel verification

`verify_reflected_pixels` and `verify_reflected_bidirectional` now compare the
original linear premultiplied pixel grids using a reflected similarity and its
exact inverse. No mirrored image buffer is allocated. Existing ordinary pixel
verification shares the same loop without changing its coordinate mapping.
Eighteen geometry/warp/photometric/filtered tests and library-only Clippy pass.
Authored horizontal and diagonal reflections cover HDR/negative channels, partial
alpha, invisible RGB, visible edits in both directions, pixel-work caps and
every observed one-shot cancellation checkpoint with identical healthy retry.

Reflected descriptor extraction, strict file/collection integration and reflected
photometric/filtered modes still need work; this primitive does not establish
natural-photo reflected duplicate support. Evidence:
`research/dedup-reflected-pixel-qualification.json`.

## Reflected oriented feature extraction

`extract_reflected_oriented` reads source pixels in reversed x order through
the existing bounded detector and oriented BRIEF pipeline. It returns features
in original image coordinates without allocating a mirrored RGBA image. The
recipe matches ordinary oriented features, but callers must verify reflected
geometry and pixels. On the seeded texture, ordinary base descriptors equal
reflected extraction of its mirrored image; recovered reflected geometry gives
25,600 matched pixels in both directions. Selected early/intermediate/final
one-shot cancellation checkpoints return no partial features, and retry succeeds.
Twenty-two local/oriented/pyramid/collection tests and library-only Clippy pass.

File/indexed-collection integration, reflected spatial/multiscale extraction and
real-photo/negative calibration remain incomplete. Evidence:
`research/dedup-reflected-features-qualification.json`.

## Explicit reflected file comparison

`compare_reflected_local_files` connects strict reflected comparison to actual
selected-frame decoding, managed multiscale descriptors, reflected geometry,
bidirectional pixels and final observation of both source snapshots. A separate
result type keeps reflection explicit. Ordinary comparison APIs are unchanged.
Both orders of the seeded base/mirror PNG pair match all 25,600 pixels each way;
the unrelated texture rejects. Managed credit returns to zero on success and
final one-shot cancellation, and a zero-byte budget refuses. Thirty-one
collection/local/oriented/local-scan tests and library-only Clippy pass.

Indexed collection integration, spatial reflection, photometric/filtered reflected
modes and natural reflected-image qualification remain incomplete. Evidence:
`research/dedup-reflected-file-qualification.json`.

## Known natural-scene reflected file qualification

Four CC0 camera-preview scenes (830/898/1084/1294) now have independently
Pillow-generated lossless horizontal reflections with pinned input/output
checksums. Explicit strict reflected file comparison accepts all eight directed
same-scene comparisons and rejects all 24 directed cross-scene comparisons.
Accepted pairs retain reflected geometry and pixel evidence with at least 98%
of each original grid matched; managed credit returns to zero after every pair.
The targeted photo test passes with eight other photo tests filtered. Evidence:
`research/dedup-natural-reflection-qualification.json`.

This known rendered-preview corpus is not held-out mirror photography, burst/
look-alike calibration, reflected edits/resampling or full RAW development.
Indexed reflected collection retrieval remains required.

## Strict reflected indexed collection search

`scan_reflected_local_collection` indexes the union of ordinary/reflected
multiscale descriptors per file, counting both orientations against the total
feature cap. Candidate retrieval is followed by fresh explicit reflected file
comparison. Combined descriptor storage is fallibly reserved and retained under
the shared memory budget. Final batch observations discard changed-source
pairs and analysed IDs, and cancellation suppresses the entire report.

Fourteen collection tests and library-only Clippy pass. The reflected base/
mirror/unrelated accepted pair set equals complete reflected pair enumeration;
feature exhaustion and final one-shot cancellation release memory, and mutation
during the scan removes the changed source and incident pairs with attribution.
Natural reflected collection retrieval, scale/RSS, fitted-color reflection,
spatial reflection and recursive roots remain pending. Existing tree-based
request/source bookkeeping remains an allocation-admission gap. Evidence:
`research/dedup-reflected-collection-qualification.json`.

## Known natural-scene reflected collection oracle

Eight known rendered-preview PNGs (four scenes, each base plus mirror) are
checked against all 28 reflected pair comparisons. Indexed retrieval preserves
exactly the four labelled same-scene edges and rejects the other 24 pairs; all
eight files are analysed with no issues and managed credit releases to zero.
The index contains 7,076 features, observes 3,032,048 descriptor hits and proposes
all 28 pairs. Thus this proves equivalence, not candidate reduction or scalable
confirmation throughput. Peak managed credit is 4,375,552 bytes and excludes
unmanaged index/bookkeeping allocations; it is not process RSS. The focused
test passes with nine photo tests filtered. Evidence:
`research/dedup-natural-reflected-collection-qualification.json`.

Held-out reflected captures/edits/bursts/look-alikes, larger collection latency/
RSS and useful candidate pruning remain pending.

## Reflected geometry preverification before fresh decoding

Combined feature storage now preserves the ordinary/reflected split. Retrieved
pairs first use exactly the ordinary-left/reflected-right feature slices used by
fresh reflected file comparison. Failure to establish geometry produces explicit
noncandidate evidence without fresh decoding; valid geometry still requires
fresh source/pixel verification. Final batch source observations remain mandatory.

Fourteen collection tests and library-only Clippy pass. The known natural
eight-file 28-pair oracle keeps the same four mirror edges; descriptor counts/
hits stay 7,076/3,032,048, while fresh pixel-confirmation pairs fall from 28 to 4.
This is decoder-work reduction, not proven wall-time acceleration: preverification
adds matching/geometry work, and the 63.78-second test includes the exhaustive
oracle and concurrent build/test contention. Index pruning, isolated repeated
throughput/RSS and held-out reflected captures remain open. Evidence:
`research/dedup-reflected-preverification-qualification.json`.

## Recursive reflected collection integration

`scan_reflected_local_collection_roots` and its request-factory variant connect
strict reflected retrieval to the existing physical discovery. Reports retain
paths, aliases and traversal diagnostics. Enumeration/request vectors reserve
fallibly, factories must preserve paths, and selected frames/color/generation
settings are passed to the existing snapshot-aware collection pipeline.

Fifteen collection tests and library-only Clippy pass. The macOS fixture has
nested overlapping roots, a hardlink alias, followed symlink cycle, base/mirror/
unrelated images and corrupt input: four physical files are admitted, one mirror
pair accepted and corrupt decode attributed. Unavailable-frame requests produce
four file errors; substituted paths, early cancellation and a file cap refuse.
Windows/Linux native decoding, spatial/fitted reflected policies, large directory
resource/latency qualification and held-out corpus remain incomplete. Evidence:
`research/dedup-reflected-roots-qualification.json`.

## Collection bookkeeping allocation admission

Ordinary/reflected collection request and feature tables now reserve HashMap
storage fallibly before insertion. Source observations use a fallibly grown Vec
in sorted ID order; final invalid IDs use a fallibly reserved HashSet. Sorted
analysed IDs and descriptor retrieval preserve deterministic output. Generation
references, source/file/pair diagnostics, ordinary result vectors and recursive
request enumeration now reserve fallibly as well.

Twenty-nine library/collection tests and library-only Clippy pass. After adding
a reversed-input check, the selected reflected retrieval test passes again with
unchanged analysed IDs and accepted edges. Mutation/cancellation/root alias
regressions remain covered. This is allocation-error propagation, not actual
allocator/OS failure injection or full shared-byte accounting; request/path/error
clones and downstream allocation qualification remain open. Evidence:
`research/dedup-collection-table-admission-qualification.json`.

## Spatially capped reflected file and collection search

Reflected spatial multiscale extraction now applies the existing detector quotas
in mirrored extraction-grid coordinates while retaining original output positions.
`compare_reflected_local_files_spatial` and
`scan_reflected_local_collection_spatial` share that policy between indexed
retrieval, geometry preverification and fresh strict pixel confirmation.

Thirty-one collection/local-scan/oriented tests and library-only Clippy pass; the
focused spatial test also passes after the final private signature adjustment.
The seeded base/mirror/unrelated cohort keeps exactly the exhaustive spatial
reflected accepted pair set, bounds features by both orientations and cell caps,
and refuses a zero-column grid before consuming inputs. Spatial reflected roots,
fitted/filtered reflection, natural spatial reflection calibration and broad
resource qualification remain pending. Evidence:
`research/dedup-reflected-spatial-qualification.json`.

## Reflected explicit linear color fitting

`verify_reflected_photometric_bidirectional` reuses the bounded per-channel
linear gain/offset fit on both reflected grids with the exact inverse; alpha
remains unfitted. `compare_reflected_local_files_photometric` connects it to
source-observed, managed selected-frame comparison while retaining original
strict pixel residuals separately. Ordinary/strict reflected paths remain intact.

Thirty-one regression tests and library-only Clippy pass. The authored affine
reflection recovers known gains, checks visible edits, work refusal and every
observed one-shot cancellation checkpoint. Four known natural mirrored brightness
derivatives accept in fitted mode while strict mode rejects and its pixel evidence
remains identical; six cross-scene comparisons reject. Reflected fitted collection/
spatial policies, filtered/range/display projection and broad held-out color/edit
qualification remain open. Evidence:
`research/dedup-reflected-photometric-qualification.json`.

## Reflected linear fitting in indexed collection policies

Reflected collection policy now accepts strict or explicit linear Photometric
comparison, propagating the selected fit into fresh confirmation while retaining
strict pixel evidence. Spatial collection selection and recursive discovery use
the same policy routing. Other comparison modes remain explicitly refused.
A spatial fitted reflected file entrypoint provides the matching pair oracle.

Sixteen collection tests and library-only Clippy pass. On eight known natural
base/mirrored-brightness files, indexed and exhaustive fitted search preserve
exactly four scene pairs and reject the other 24, with strict/fitted evidence in
accepted results. There are 6,858 features, 2,763,278 hits, 28 proposals and four
fresh confirmations. Four natural spatial fitted file positives also pass and
six cross-scene fitted negatives reject. Fitted spatial-collection/root runtime
qualification, reflected filtered/range/display modes and held-out calibration
remain pending. Evidence:
`research/dedup-reflected-fitted-collection-qualification.json`.

## Natural fitted spatial-collection and recursive-root routing

Two focused runtime tests now qualify the previously pending policy routing.
The eight-file natural mirrored-brightness spatial collection equals all 28
spatial fitted pair comparisons: four scene edges accept and 24 negatives reject.
There are 3,780 features, 745,464 descriptor hits, 28 proposals and four fresh
confirmations. Managed peak is 3,874,560 bytes, excluding index allocations/RSS.
These cohort counters are not a general quota recall or statistical speed claim.

A four-file/two-scene recursive fitted collection under nested overlapping roots
equals all six explicit fitted comparisons, with two accepted edges and four
negatives. Paths and strict/fitted evidence are retained, and managed credit
returns to zero. The request factory's unavailable frame index produces four
explicit decode errors, confirming settings are propagated. Both focused tests
pass; other photo tests were filtered. Spatial reflected roots, broader modes,
held-out photographic calibration and full resource guarantees remain pending.
Evidence: `research/dedup-reflected-fitted-natural-routing-qualification.json`.

## Recursive reflected spatial policy integration

Default and request-factory reflected spatial root scanners now share the
existing discovery/request admission pipeline and pass spatial quotas to the
indexed scan, cached geometry preverification and fresh strict/fitted comparison.
Grid validation precedes discovery and request callbacks.

Sixteen collection regressions and library-only Clippy pass. A focused known
natural recursive spatial fitted test agrees with all six pair comparisons
(two scene positives/four cross-scene negatives), retains strict/fitted evidence
and four physical paths, and preserves unavailable-frame settings as four decode
errors. The strict spatial-root regression also passes with overlapping roots,
Unix hardlink aliases, symlink cycle, corrupt file attribution and invalid-grid
refusal before factory invocation. This macOS evidence does not establish
Windows/Linux native decode, held-out calibration or full memory/latency coverage.
Evidence: `research/dedup-reflected-spatial-roots-qualification.json`.

## Reflected filtered pixel primitive

Reflected filtered photometric verification now shares the bounded original-grid
window sampler with ordinary filtering, with explicit linear/encoded-sRGB choice
and reflected inverse. Both reject-outside fitting and constrained least squares
are exposed, without reflected RGBA allocation or implicit clipping.

Thirty library/filter/encoded/photometric/warp tests and library-only Clippy pass.
Authored mirrored gradients match all 49 interior windows both ways in both
spaces; constrained fitting also matches, alpha edits remain visible, exact work
limit admits and one-less refuses, and selected early/intermediate/final one-shot
cancellation checkpoints publish no evidence with an identical healthy retry.
File/collection routing and natural reflected resampling/compression calibration
remain required; this primitive does not establish those cases. Evidence:
`research/dedup-reflected-filter-qualification.json`.

### Spatial reflection qualification correction: geometry ranking counterexample

The current symmetric 4x4, 20/cell reflected file policy reproduces a missing
positive for scene 1084 mirrored with encoded brightness adjustment. Earlier
four-positive spatial evidence used uncapped right-side reflected features in
file confirmation and does not prove the corrected symmetric pipeline.
The exhaustive eight-file spatial oracle currently accepts three of the four
required positives; the assertion remains unchanged and failing.

The failing pair has 294 mutual descriptor correspondences: 289 are exactly
consistent with the fixture's reflection x -> 319-x, four are displaced by one
pixel, and one has squared displacement five. Count-first geometric ranking
prefers a slightly distorted model admitting all 294 points, even though that
model fails the unchanged bidirectional photometric residual threshold. This is
a model-selection counterexample, not evidence to relax pixel acceptance.
`reflected_brightness_1084_known_fixture_geometry_passes_unchanged_pixel_policy`
separately checks the independently known fixture mapping against the existing
pixel policy. Runtime discovery still needs a bounded way to retain and confirm
alternative geometric hypotheses; supplying the fixture mapping is diagnosis
only and must not become a fixture-specific runtime fallback.
See `research/dedup-spatial-reflection-geometry-diagnostic.json` for the model,
counts, source hashes and explicit correction of previous spatial claims.

### Bounded alternative geometry resolves the spatial reflection counterexample

`verify_similarity_candidates` and `verify_reflected_similarity_candidates`
retain at most two eligible models during one exhaustive two-point search with
one caller-supplied hypothesis cap. The original count-first winner is preserved;
the second minimizes total capped squared residual over every correspondence.
Both retain the same minimum-support and non-collinearity requirements and each
receives at most one least-squares refinement. Identical final transforms are
returned once. The legacy single-model geometry APIs preserve their ranking.

Reflected file comparison first verifies the capped-residual model and then, if
needed, the count-first model against the same freshly decoded source pixels.
At most two bidirectional confirmations run; source snapshots and latched
cancellation cover both. Negative evidence retains the first tested model.
Strict and fitted residuals refer to the selected model and remain separate;
matching, overlap, gain/offset and pixel acceptance thresholds are unchanged.
Collection preverification remains a conservative existence check over the same
spatial features; fresh file confirmation uses the new bounded model choice.

The original eight-file spatial natural-photo oracle now recovers all four
labelled mirror/brightness positives and rejects all 24 cross-scene pairs.
Indexed collection retrieval has identical accepted labels, with 3780 combined
features, 745464 descriptor hits, 28 proposed pairs and four fresh confirmations.
The previously missed 1084 model now has a=1.0000610698374428,
b=-0.000009253005673095763 and translation
[319.0090101537495,-0.008937696083236801]; its bidirectional fitted residuals match
all compared pixels. The test also checks all four authored mirror corner
positions within 0.1 pixels, rather than acceptance alone.

An independently authored 26-point geometric example has 21 exact matches,
four one-pixel corner localization errors and a marginal extra outlier. Both
ordinary and reflected candidate APIs retain the 26-support count winner and a
25-support capped-residual model. Independent centered normal equations predict
scale .992 and squared inlier residual 3.36. Additional tests cover exact
hypothesis admission/refusal, selected one-shot cancellation checkpoints and
identical retries, duplicate/collinear rejection and model deduplication.
This finite two-model policy is not proof of universal transformation recall;
perspective geometry, broader held-out photographs and the other contract rows
still require implementation and qualification.

Validation for this change: the all-feature crate invocation completed 224
passing tests with none failed, ignored or filtered. Its geometry binary predates
the final additional degeneracy/deduplication test and expanded reflected
cancellation checks; separate final geometry runs each pass all nine current
tests with all features and without default features. The focused file,
collection and directory regression invocation passes 43 tests. Library Clippy
passes with warnings denied; existing decoder dead-code warnings are external
to this library check. `research/dedup-alternative-geometry-qualification.json`
archives the logs, source hashes and exact scope. Dependency stability snapshots
were captured during the full run, not before its launch. The only library source
change afterwards was API documentation. Ordinary file routing, recovery between
models after fitting failures, reflected filtered routing and all broader
acceptance requirements remain open.

### Reflected model confirmation recovers from model-specific fitting failures

The reflected file confirmation stage now tries the second retained geometric
model after `InsufficientSamples`, `LowVariance` or `OutsidePolicy` fitting
failure on the first model. These failures depend on the model's overlap and
sample distribution. Successful recovery still requires the unchanged support,
coverage and bidirectional fitted-pixel thresholds; the selected model retains
its strict original-pixel residuals. No fixture-specific transform is used by
runtime discovery.

If neither model verifies and any fitting attempt was inconclusive, the first
fitting failure is returned rather than a negative candidate. Invalid inputs,
work-budget exhaustion and cancellation remain terminal and do not trigger an
alternative-model retry. At most two retained models share the existing decoded
images, and the file caller still checks both source observations and latched
cancellation before publishing successful evidence.

Five confirmation-stage tests use independently authored 32x32 gradients and
vertical stripes with a known mirror and RGB gain .5 plus offset .02. A first
model with 352 opaque overlap samples fails a required 800-sample fit; the second
model verifies all 1024 pixels in both directions. Separate tests cover first
model variance and coefficient-bound failure, two failed fits preserving the
first reason, an inconclusive first fit plus a residual-rejected second fit,
selected one-shot cancellation checkpoints with identical retry evidence and
terminal fitting work-budget refusal. Models in these unit fixtures are supplied
to the private confirmation stage; they qualify recovery semantics rather than
end-to-end feature/model discovery. The natural-photo collection regressions
exercise discovery and file confirmation together.


The fitting-recovery validation passes all 71 selected unit/integration tests
with none failed, ignored or filtered; library Clippy passes with warnings denied.
`research/dedup-reflected-fit-alternative-qualification.json` archives the logs,
current library hashes and dependency-stability caveat. A concurrent edit to
`rrrah-decode/src/pict.rs` occurred during the run and is preserved. PICT is not
exercised by this regression scope, so this evidence does not assert current
all-format decoder coverage or completion of the overall contract.

### Explicit filtered reflection through file and indexed collection APIs

`compare_reflected_local_files_with_policy` now accepts strict, linear
photometric, or explicit filtered comparison plus optional spatial quotas on
both images. The same selected signal routes through reflected collection
confirmation. Linear-sRGB and encoded-sRGB filtering, rejecting out-of-policy
coefficients and constrained least squares are explicit choices; no implicit
clipping or acceptance-threshold relaxation is introduced. Range fitting and
display projection remain unsupported in the reflected policy entrypoint.

`ReflectedLocalFileEvidence` exposes original strict pixel residuals, optional
ordinary linear photometric residuals, an auxiliary linear fitting failure when
applicable, and selected filtered evidence separately. Selected filtering decides
candidate acceptance. A model-specific selected-filter fit failure can try the
second bounded geometry; invalid data, budget exhaustion and cancellation remain
terminal. Auxiliary linear fit failure does not suppress a successful explicitly
selected filtered-space comparison and its reason is retained.

Four pinned compressed camera-preview JPEG derivatives are independently mirrored
losslessly with Pillow into PNG fixtures. The generator verifies input hashes and
records output hashes, dimensions and CC0 provenance in
`filtered-reflections-manifest.json`. These fixtures qualify reflected JPEG
compression artifacts, not a reflected JPEG decoder path or held-out photographs.
Direct comparison tests exercise all four labelled scene positives and two
cross-scene negatives in both spaces and both fitting modes: 16 positives and
8 negatives. A four-file/two-scene spatial collection is independently compared
with every one of its six file pairs under all four signal/fit choices, retaining
both true pairs and rejecting all four cross-scene pairs in each choice.

An independently authored dark-gradient unit fixture confirms that low-variance
ordinary linear fitting can coexist with qualified selected encoded-space
filtering: the failure reason remains explicit while all 676 complete 7x7
windows match in both directions. The exact admitted sample-pair cap is 200704.
Additional file tests cover invalid filter validation before source or collection
access, selected-filter work-budget refusal with released managed memory, late
one-shot cancellation, final generation invalidation and mutation before source
revalidation. Controlled recursive filtered qualification is recorded in the following
section; wider directory/platform and alias-retarget mutation coverage remains
open. Existing strict/fitted recursive paths are exercised by the regression scope.


The final filtered-routing regression invocation passes all 80 selected tests,
with none failed, ignored or filtered. Library Clippy with warnings denied and
format validation pass. Library sources remain unchanged from the pre-launch
snapshot. All 234 hashes matched at the last during-run check; a later PICT edit
has a modification timestamp after the test log completed and is preserved.
The evidence, snapshot caveat and fixture hashes are archived in
`research/dedup-reflected-filter-routing-qualification.json`. These checks do not
complete the broader acceptance table or prove the current all-format decoder.

### Recursive filtered reflection with physical aliases and per-path requests

The existing filtered reflected collection routing is now exercised through
both recursive entrypoints, with and without spatial quotas. A temporary nested
directory contains four healthy pinned camera-preview fixtures for scenes 830
and 1084 plus a corrupt file. Overlapping parent/child roots, a Unix hardlink to
the 830 base and a symlink cycle must preserve five physical-file entries and
one alias group, rather than duplicate work or traverse the cycle repeatedly.
The request factory receives every returned discovered path exactly once.

All eight configurations (ordinary/spatial selection, linear/encoded sRGB,
rejecting/constrained fitting) are compared with all six direct healthy file
pairs under identical policy. Each discovers the two labelled mirror/JPEG
pairs, rejects the four cross-scene pairs, proposes six descriptor pairs and
performs two fresh pixel confirmations. Every accepted pair retains strict
pixel residuals and the requested filtered space and fit mode. The corrupt file
has its own attributed file issue without suppressing the two healthy matches.

A separate factory-control test selects unavailable frame one on all PNG files
and requires five explicit file errors, no analysed files and no candidate
pairs. It also checks invalid filtering and grid rejection before callbacks,
path substitution rejection, early caller cancellation, file-count refusal
before callbacks, total feature-count refusal without a partial report, and a
cancelled generation token supplied by the request factory. Managed memory must
be released after every successful, failed or cancelled invocation.
This controlled macOS/Unix qualification does not establish arbitrary directory
layouts, non-Unix identity, complete-container matching or held-out photographs.


Final labelled matrix and factory-guard invocations each pass their selected
test. The matrix assertion explicitly requires one positive for scene 830 and
one for scene 1084, so alias duplication cannot disguise a missing scene.
The guard invocation predates that additional assertion in the separate matrix
test; the guarded function itself stayed unchanged, and the matrix was rerun.
Library implementation is unchanged from the previous 80-test routing
qualification. Format validation passes; no redundant whole-library rerun is
claimed for these test additions. Logs, labels, source/function hashes and exact
scope are archived in `research/dedup-reflected-filter-roots-qualification.json`.
The overall acceptance contract remains incomplete.

Concurrent changes to `rrrah-decode/src/pict.rs` and `rrrah-memory/src/lib.rs`
are detected between the guard snapshot and artifact creation and preserved.
The targeted logs qualify their compiled versions; they do not prove the current
entire decoder or memory subsystem. The artifact records this dependency caveat.


### Ordinary spatial brightness uses bounded alternative geometry

The ordinary selected-file pipeline now uses the same bounded pair of geometric
models as reflected comparison: capped residual first, count-first second. Each
model is independently confirmed against the original decoded views with the
selected strict, photometric, range, display-projection or filtered policy.
Acceptance thresholds and fit limits are unchanged. Fit failures allow the other
model to be tested; cancellation, invalid input and budget failures remain
terminal. An unresolved fit failure remains inconclusive rather than becoming a
negative result. Returned residuals and fitting evidence belong to the selected
model. Legacy single-model geometry APIs retain their previous ranking.

A pre-change reproduction rejected the known scene-1084 brightness derivative
with spatial quotas (4 by 4, 20 features per cell): one marginal correspondence
favoured a slightly distorted count-first model. After the change, all four
labelled brightness derivatives pass. The eight-file indexed collection agrees
with all 28 direct spatial comparisons: exactly four same-scene positives and
24 negatives. Its scene-1084 transform maps all four authored image corners
within 0.1 pixel of identity. The managed budget returns to zero. These fixtures
are derived embedded camera previews, not held-out photographs or real bursts.

A separate private confirmation-stage test supplies two explicit models and
qualifies recovery from insufficient fit overlap in photometric, range and
display modes, selected cancellation checkpoints, deterministic retry and
terminal residual-budget refusal. Supplying those models does not independently
qualify feature extraction or geometric discovery. Logs and source-state limits
are recorded in `research/dedup-ordinary-alternative-qualification.json`.


### Reflected range fitting and explicit display projection

Reflected selected-file and indexed collection entrypoints now accept
`RangePhotometric` and `DisplayProjection` rather than refusing those valid
policies. Evidence exposes the fitting range and whether display projection was
selected, including collection pairs rejected by geometric preverification. The
existing range fitter shares one sampling pipeline with an explicit orientation
flag and exact orientation-reversing inverse. Range fitting excludes clipped
samples from model estimation but still verifies all overlap. Display projection
clamps RGB residuals in the explicit range; alpha remains unprojected and strict
original pixel evidence remains separate. No mirrored image buffer is allocated.

An independently authored 5-by-4 to 4-by-5 mapping `(x,y) -> (3-y,4-x)` qualifies
reflection plus rotation and translation. One source channel is 4 while its
displayed counterpart is 1: 19 fit samples in both directions; ranged residuals
and strict pixels match 19/20, projected residuals match 20/20. Changing alpha
remains a negative residual under projection. Every cancellation checkpoint in
this small display comparison returns cancellation without partial evidence;
retry agrees, invalid ranges fail, and insufficient residual budget refuses.

For each of the two modes, eight known camera-preview base/mirror-brightness
files are compared over all 28 pairs and then indexed: four independently
labelled same-scene pairs, 24 different-scene negatives, unchanged strict
thresholds and zero retained managed bytes. A separate supplied-model unit test
checks alternate-model fitting recovery and selected-range metadata. Invalid
ranges are refused before the collection iterator is consumed. These tests do
not establish held-out photo accuracy, broader recursive range-mode/platform qualification,
full-resolution reflected RAW/development equivalence or all-format HDR support.
See `research/dedup-reflected-range-qualification.json` for staged source state
and validation logs.


### Recursive reflected ranged/display qualification

The recursive reflected range/display entrypoints are now exercised through
overlapping parent/nested roots. The fixture contains two independently labelled
base/mirror-brightness pairs (scenes 830 and 1084), one corrupt PNG, a Unix
hardlink alias and a followed directory symlink cycle. Both explicit modes are
qualified with ordinary feature selection and spatial quotas (four total
configurations). Each configuration discovers five physical files, analyses four,
attributes one decode error to the corrupt path and preserves the alias group.
All six healthy-file direct comparisons produce exactly two labelled positives
and four cross-scene negatives; indexed accepted edges agree with that oracle.
Strict pixels and the selected range/display metadata are retained and managed
budget usage returns to zero. These are authored derivatives of embedded camera
previews, not real bursts or held-out photo qualification.

Both modes also qualify unavailable frame selection (five explicit file issues),
invalid range/grid refusal before the request factory, substituted path refusal,
early caller cancellation, file-budget refusal before the factory, total-feature
budget refusal and stale request-generation cancellation without a partial
report. Evidence and staged source state are in
`research/dedup-reflected-range-roots-qualification.json`. Platform scope is the
current macOS/Unix host; non-Unix physical identity and broad filesystem/decoder
coverage remain incomplete.


### Complete-presentation scale baseline

`examples/presentation_scale_probe.rs` qualifies the existing exhaustive
`scan_presentations` path in release mode on 8, 32 and 128 separately copied
files, for both GIF/APNG timelines and three-page TIFF variants. Fixture labels
independently declare that base and differently encoded/split versions are equal
and last-frame/page changes are different; every unordered pair is checked
against these labels after each invocation. Preparation is outside timing; input
cloning, admission, snapshot checks, complete decoding, comparison and report
construction are included. Each case has one warmup and three measured runs.

The probe records wall time and peak managed-buffer usage, and refuses any pair
issue, wrong accepted/rejected edge or retained managed allocation. OS peak RSS
is measured separately for the whole probe process (including setup, warmups
and all cases); it is not per-case RSS. Managed-buffer numbers do not account
for every decoder, collection and allocator allocation. Cold caches, large
images, larger file collections, concurrent mutation and cancellation latency
are not established by this baseline. Full-search implementation still performs
quadratic pair comparisons and repeated decoding.

Evidence is in `research/dedup-presentation-scale-baseline-qualification.json`.
Two initially overlapping exploratory invocations were terminated and excluded
from timing evidence. Only the subsequent single sequential release process is
used. Concurrent changes to decoder sources prevent claiming qualification of
the later decoder state; the built executable and source snapshots are hashed.


### Bounded reuse of complete presentations

`confirm_presentations` (and the existing exhaustive/grouped/directory callers)
now retains one left presentation across each sorted row of pairs. The shared
preparation function preserves selected TIFF/RAW classification, complete page
order and complete animation timeline comparison. No new public search entrypoint
is required. Each right presentation is decoded and released per pair; the old
left is released before preparing a different left. Existing managed decode
budgets remain in effect, so this does not retain the entire collection.

Before reuse the retained content snapshot is verified; both source observations
precede new decoding and both are verified after comparison. A latched callback
and both requests' generation tokens preserve cancellation. An altered cached
source yields an explicit source error. This is per-pair observation, not an
atomic filesystem snapshot or a replacement for batch grouping invalidation.

A test-only preparation counter independently checks three preparations for
two comparisons rather than four. The same fixture qualifies cancellation/retry,
mutation rejection and final memory release. Existing container tests continue
to qualify page/animation negatives, zero-duration timelines, source mutation,
recursive aliases/errors and automatic scope detection. The search still has
quadratic pair enumeration and right-side decoding; linear/scalable full-container
retrieval remains required. Current evidence is recorded in
`research/dedup-container-reuse-qualification.json`.


### Retained-presentation resource guards (revalidated)

Two additional tests exercise memory exhaustion while reusing a prepared left
file, retry after releasing the artificial reservation, release before a new
left id with incompatible modes, exhaustive one-shot callback cancellation
positions and stale request-generation cancellation. The initial run passed the
cancellation/generation test but exposed an overly shallow expectation for the
explicit memory error. The error propagates through the decoder source-memory
wrapper; that assertion has been corrected. Current revalidation and Clippy
are blocked by concurrent X3F dependency compilation errors, so the corrected
memory test is not yet counted as passing. No runtime library logic was changed
in this guard milestone. Exact logs, source drift and remaining checks are in
`research/dedup-container-reuse-guards-state.json`.


The temporary X3F compilation blocker above is resolved in the current worktree.
Revalidation passes all 25 library unit tests and nine container integration
tests, including the corrected nested memory-capacity assertion, retry, row
eviction, every small-fixture cancellation callback position and request
generation. Scoped Clippy passes with existing decoder warnings. Evidence is in
`research/dedup-container-reuse-guards-revalidated.json`; the older state/logs are
retained as diagnostic history. Broader scale/corpus/platform requirements remain.


### Complete-presentation candidate digest primitives

`Pages::page_digest` now hashes every ordered selected-frame digest and the page
count under a versioned domain. The selected-frame key preserves the sample
scale and cursor hotspot used by direct page equality. `Animation::timeline_digest`
hashes the normalized displayed timeline under a separate versioned domain: exact
repetition semantics, canonical pixel digests, zero-duration omission, adjacent
equal-key run merging and reduced rational duration sums. Animation keys use
pixel equality semantics, matching `same_timeline`, rather than selected-frame
metadata that the existing timeline comparator does not compare. Both keys are
candidates only; direct pixels/pages/timeline comparison remains mandatory for
equal digests, including collisions.

Run summation uses checked u128 arithmetic, reduction and bounded u64 rational
components. A fixture with two identical adjacent frames of duration u64::MAX
proves that direct timeline equality can succeed while key summation refuses
with an explicit timing error. Such a signature failure is inconclusive and any
future retrieval pipeline must fall back to direct confirmation; it must not
exclude that pair. The digest primitives allocate no additional frame/image
collection and retain existing normalized-view pixel bounds.

Six independently labelled GIF/WebP/APNG fixtures qualify all 15 pairs: base,
split and zero-prefix encodings agree; changed time and changed pixels differ.
Three TIFF variants qualify cross-compression/byte-order equality and a last-page
negative. Private fixtures independently retain page order and count even when
the first page is unchanged, empty displayed timelines ignore pixels but retain
repetition, and rational-sum overflow is inconclusive. Every cancellation
callback position is tested on the small page and split-animation digest
fixtures with explicit cancellation errors and deterministic retry.

At the primitive milestone these keys were not wired into collection retrieval;
the routing milestone below records their subsequent integration. Indexed discovery, injected
key collisions, per-source failures, mutation/cache admission and large-collection
qualification remain required. Logs and source scope are recorded in
`research/dedup-presentation-digest-qualification.json`.


### Complete-presentation candidate keys in existing search

`confirm_presentations` now prepares one complete signature per participating
source in compatible admitted pairs, releasing its decoded presentation before
preparing the next source. Signature equality still invokes the existing direct
comparison with bounded left reuse. Signature failure (including checked
rational overflow, source/decode/resource errors) is inconclusive and falls back
to direct confirmation, preserving per-pair diagnostics. A latched callback and
request-generation tokens cancel the batch without a partial report.

Different signatures may reject a pair only after both original content
snapshots are verified again. A changed source yields an explicit source error.
Pairs with incompatible modes bypass filtering; sources participating only in
incompatible pairs are not prepared and retain the mode error before reading. Metadata storage is fallibly allocated and
bounded by admitted files; existing file/pair/decode budgets still apply. This
is an in-call filter, not a persisted cache or a digest-based identity claim.

Test-only injected constant signatures and injected signature timing failures
both produce the independently labelled three equal and three different pairs
in a four-file animation fixture. Real signatures reduce preparation calls from
13 to 9 on that same fixture. A private mutated-source test prevents an old
negative signature decision from being admitted. Existing complete-page,
animation, recursive grouping and source-mutation regressions pass. Evidence is
in `research/dedup-presentation-key-routing-qualification.json`.

Full pair enumeration and explicit different-pair output are still quadratic,
and matching-key buckets still require direct confirmation; all-identical data
can receive additional preparation work. A scalable candidate-only/grouping
path, larger independently captured corpora, complete allocation accounting and
platform/cache qualification remain required.


### Whole candidate-key pipeline cancellation and error guards

The existing complete-presentation scan is qualified under real keys, injected
constant keys and injected signature timing failures. On a labelled three-file
fixture, every callback position (409/681/666 respectively, 1,756 total) is
cancelled with a one-shot true response. Every run returns cancellation rather
than a partial report and releases managed memory. Fifteen selected request
generation changes also cancel; restoring generation allows deterministic retry
with the same independently labelled equality/negative edges.

Zero- and one-byte managed budgets produce explicit typed capacity issues for
all three pairs and no equality/difference classification. A corrupt third GIF
retains the healthy base/split equality and two attributed pair issues; it is
not treated as unique. All 32 library tests, nine container integration tests and
two digest integration tests pass (43 total), plus scoped Clippy with existing
decoder warnings. These tests add no runtime logic changes. Evidence is in
`research/dedup-presentation-key-pipeline-guards-qualification.json`. Long
indivisible decoder calls, OS allocator faults, full RSS and every-format
cancellation/resource qualification remain incomplete.


### Candidate-only complete-presentation grouping

`scan_indexed_presentation_groups` is an explicit candidate-only variant sharing
admission, source observations, direct confirmation and final invalidation with
`scan_presentation_groups`. The exhaustive public API retains its complete-pair
budget and report. Indexed retrieval places successful full-presentation keys
in scope-separated buckets (selected frame, ordered pages, displayed animation).
Only within-bucket pairs are proposed. Any signature failure proposes all
compatible partners and remains an attributed preparation issue. The candidate
union is deduplicated, fallibly allocated and capped by `max_pairs` before direct
confirmation. A cap exceeded by collisions or inconclusive fallback refuses
instead of returning partial/unique classifications.

Initial snapshots cover every admitted id; final observations remove all edges
from changed sources before complete-link grouping. The shared group workflow
now latches caller cancellation and every request generation across phases.
`signature_preparations` counts only the initial retrieval pass, not subsequent
confirmation key work. Singletons remain unproven uniqueness. Omitted different
buckets are not fabricated negative-pair entries. Existing explicit exhaustive
recursive entrypoints continue to use the exhaustive mode.

Independent four-file labels produce the same groups and three equal edges as
six-pair exhaustive search using only three candidates. Constant-key collision
and signature-timing-failure injection expand to six candidates and preserve
those groups; a three-candidate budget then refuses explicitly. A mixed-scope
fixture preserves three separate selected/page/animation groups even under
constant keys. A changed first source is removed while the healthy equal pair
remains. Every normal indexed-group callback position (976) returns cancellation
without partial output and releases managed memory; retry agrees.

The existing release probe accepts `--indexed-groups`; all 18 measured runs
verify the independently labelled groups and accepted edges at 8/32/128 files
for tiny GIF/APNG and TIFF fixtures. This mode includes indexed output-label
validation within its measured interval, so its times are descriptive and not
a controlled comparison with the earlier exhaustive probe. Preparation/copying
is outside timing. No current speedup or large-corpus proof is inferred.

Evidence is in `research/dedup-indexed-presentation-groups-qualification.json`.
The complete-link grouping engine still checks unrelated groups quadratically
on distinct inputs; matching/collision buckets still emit and verify bounded
pairwise evidence. Candidate fallback can also be quadratic. Optimization and
large independently labelled corpus/allocation/platform qualification remain
required; indexed recursive wrappers are not yet integrated.


### Indexed complete-link eligibility preserves deterministic grouping

`complete_link_groups` now indexes accepted lower-id neighbours and maps each
group's first member to its group index. A group can admit a new id only if its
first member has an accepted edge to that id, so groups lacking that necessary
edge are skipped without member-pair probes. Candidate group indices are sorted
to preserve the original earliest-group choice, then every member is checked
under the existing complete-link rule. All accepted edges, including edges
between different output groups, remain in the returned evidence.

`max_pair_checks` counts actual member-edge probes after indexed eligibility.
The zero-check budget test now uses an accepted edge to demonstrate refusal;
isolated ids need no member checks and can succeed with a zero pair-check
budget. Admission, raw edge limits, fallible metadata storage and cancellation
remain independently enforced. The additional index stores bounded metadata
proportional to accepted edges and entries, rather than decoded images.

An independent simple exhaustive reference agrees on all 32,768 six-node
graphs, in addition to the existing 64 four-node order/cancellation fixtures.
A 16,384-id isolated fixture succeeds with zero member checks and 32,769
callback checkpoints; an 8,192-id chain preserves every accepted edge and the
expected adjacent-pair partition under an 8,192-check budget. These prove
operation/budget behavior on those fixtures, not an OS RSS or universal runtime
speed claim. Integration with presentation grouping, collisions/fallback, source
invalidation and callback cancellation remains qualified by the targeted suite.
Evidence is in `research/dedup-sparse-group-selection-qualification.json`.

Dense accepted graphs and matching/collision buckets still require bounded
pairwise evidence, and metadata allocation/RSS, independently captured large
corpora and recursive indexed search integration remain incomplete.

### Recursive indexed complete-presentation retrieval

`scan_indexed_presentation_roots` and `scan_indexed_presentation_roots_auto`
share discovery, aliases, per-path mode selection/content detection and final
source validation with their exhaustive counterparts. Their report wraps the
existing directory result and preserves candidate count, initial signature
preparation count and attributed preparation issues. Automatic mode retains
classification snapshots through confirmation and regrouping, including ids
whose classification failed. Omitted pairs are inconclusive, not proven different.

The independently labelled nested GIF/APNG fixture includes overlapping roots:
base GIF and split APNG are equal, while changed pixels are a separate singleton.
Both entrypoints must fit a one-candidate budget where exhaustive admission
requires three pairs; a zero-candidate budget refuses without a partial result.
Current validation is recorded in
`research/dedup-indexed-roots-qualification.json`. This is not broad malformed
container, burst, filesystem-platform or measured large-scale qualification.

### RAW partially admitted memory refusal and recovery

The native RAW probe now exercises both zero managed budget and a partial
budget admitting reservations before refusal. For each of the 27 pinned public
RAW files and two owner CR3 files, refusal must identify memory/budget failure,
release every managed reservation, and permit adequate-budget development in
the same process. Both macOS and Linux completed all 29 cases successfully.
The recovered normalized pixel digest, dimensions and source hash agree with
the original successful baseline for all 29 files on each platform. The partial
case additionally requires a nonzero managed peak before full release.

Evidence: `research/dedup-raw-partial-budget-comparison.json` and its two
platform reports/logs. Targeted example Clippy completed successfully; dependency
dead-code warnings remain. This gate does not measure all native allocator RSS,
prove arbitrary-camera support, or provide independent rendered-color fidelity.
Earlier full-suite counts predate this example-only strengthening.

### RAW cancellation after managed admission

The existing RAW probe also cancels via the library callback at the first
checkpoint observing a nonzero managed peak. Each file must return an explicit
cancellation error and release all managed reservations before a healthy
same-process retry. All 29 pinned files pass on macOS and native Linux with
terminal exit 0. Recovered source hashes, sensor/developed dimensions and
normalized pixel digests match the original successful baselines in all cases.
The source and actual executable hashes are retained in
`research/dedup-raw-cancel-comparison.json` and
`research/dedup-raw-cancel-state.json`; the Linux report probe hash itself refers
to the wrapper. Targeted example Clippy also completed successfully.

This checks the library callback boundary after managed admission. It does not
prove native decoder-internal cancellation responsiveness, a latency bound,
all native allocator RSS or arbitrary RAW camera coverage. The stronger probe
is newer than the previously recorded full-suite snapshot.

### Native RAF cancellation checkpoint sweep

The synthetic Bayer RAF decoder test first counts callbacks during successful
decoding, requires more than one checkpoint, then injects one-shot cancellation
at every observed checkpoint. Each attempt returns typed DecodeError::Cancelled;
healthy retry produces the original sensor pixels. The strengthened test passes
on macOS and native Linux, followed by the full RAF test module on both platforms.
Counts and source identity are recorded in
`research/dedup-raf-cancel-checkpoints-state.json`. Initial non-Sync test-counter
compile errors were repaired with an atomic counter before either passing run.
This establishes callback behavior for the synthetic Bayer RAF path, not real
compressed RAF cancellation latency, universal RAW coverage or native RSS.

### Native camera cancellation sweep

Synthetic CR2, ordinary/compressed NEF, ORF, ARW, RW2 and PEF pixel
decoding now receives one-shot cancellation at every observed successful-path
callback, followed by a healthy retry matching baseline sensor pixels. macOS
and native Linux pass 151 camera tests; 22 external-corpus tests stay explicitly
ignored. Counts agree across platforms: CR2 11, NEF 5 in each path, ORF 3,
ARW 2, PEF 3 and RW2 1. The tiny RW2 fixture therefore qualifies only initial
refusal; larger RW2 and other compressed-path/real-file latency qualification
remain open. Evidence: `research/dedup-camera-cancel-sweep-state.json`.

### RW2 cancellation within unpacked strips

Unpacked RW2 previously observed cancellation only before each strip; one strip
may contain a complete sensor frame. It now checks every 4096 decoded samples
inside each strip. The new 8192-sample single-strip fixture verifies independently
constructed sample values, all three observed one-shot cancellation checkpoints
and healthy retry, including refusal after partial pixel decoding. All 151 camera
tests pass on macOS and native Linux; 22 external-corpus tests remain ignored.
Evidence: `research/dedup-rw2-block-cancel-qualification.json`. The bound is in
sample operations between callbacks, not measured wall-clock cancellation latency
or a guarantee for every other RW2 compression path.

### Real RAW regression after RW2 cancellation change

The rebuilt native sensor dumper again passes all 29 pinned RAW cases against
the unchanged independent sample/color-metadata references at a 512 MiB managed
budget. The rebuilt full-development probe also passes the real GH2 RW2 on
macOS and native Linux: normalized dimensions/digest match the pre-change
baseline, and resource refusal, library callback cancellation, release and
same-process retry pass. Evidence: `research/dedup-rw2-real-regression.json`.
This does not time generation-token cancellation inside the real decoder or
certify independent rendered photographic color.

### Configured camera regressions with actual pinned sources

Seven existing optional-input regressions no longer merely return early in
this gate: CR2, NEF, ARW, ORF, PEF, RW2 and RAF receive explicitly configured
pinned public sources. All seven pass on macOS and native Linux without skip
messages. They exercise actual routing, sensor/metadata consistency, managed
source/output ownership, retention until the last owner drops and insufficient
output-budget refusal. Source SHA-256 is checked against the corpus manifest.
Evidence: `research/dedup-configured-real-camera-qualification.json`. These
checks are internal consistency tests, not new independent sample/color oracles;
they do not reduce the separately enumerated ignored external/private tests.

### Real-camera generation cancellation checkpoints

The seven pinned configured camera files additionally qualify native pixel
decoder cancellation from GenerationToken at first, middle and final observed
callback. Each reports typed Cancelled; stale requests also refuse before source
reading, and healthy managed retry returns baseline samples and releases its
reservation. All seven tests pass on macOS and native Linux. Callback counts
match across platforms: RAF 3288, RW2 3472, ARW 4024, PEF 2868, NEF 9970,
ORF 3472, CR2 16872. Evidence:
`research/dedup-real-generation-cancel-qualification.json`. These deterministic
changes exercise the real pixel paths, not asynchronously timed whole-request
cancellation, wall-clock latency bounds or all native allocator RSS.

### OS-reported full RAW process memory on macOS

Sequential full RAW probes measured by /usr/bin/time -l preserve baseline
pixel digests for GH2 RW2, X1D 3FR and owner EOS R8 CR3. OS maximum RSS is
687472640, 2277277696 and 1040646144 bytes respectively; managed peaks are
619877248, 1963939840 and 951687328 bytes. In particular the 3FR process exceeds
the 2147483648-byte managed limit in OS RSS. The current managed budget therefore
must not be presented as a complete process/native-allocator memory cap.
These are three measured workloads, not a universal upper bound or a Linux/
Windows measurement. Evidence: `research/dedup-raw-rss-macos-report.json` and
retained time stderr logs. Complete allocation accounting remains open.

### Earlier RAW scratch release with full native regression

Corrected sensor samples are now dropped immediately after demosaicing; the
clipping mask is dropped immediately after highlight reconstruction. They no
longer remain live through output RGBA allocation. Measured macOS full 3FR
development peak RSS changes from 2277277696 to 2118713344 bytes (158564352
bytes lower) with identical normalized pixel digest. This single workload
measurement does not prove a universal total-process bound. The native core
all-feature/all-target suites pass all 82 tests on both macOS and Linux without
ignored tests. Both native full 29-file RAW development gates pass, including
zero/partial managed-budget refusal, library cancellation, full release and
healthy retry; every source hash, dimension and pixel digest matches the
pre-change baseline. Evidence: `research/dedup-raw-scratch-lifetime-state.json`.
Full process accounting, native Windows and wider independent color/scene
qualification remain required by the original acceptance matrix.

### Planar projective geometry and pixel primitives

Normalized four-point homography fitting and bounded exhaustive four-point
consensus now expose a separate planar evidence class. Supplied projective
pixel verification reuses premultiplied linear bilinear sampling, reports overlap
counts and rejects singular matrices or a horizon crossing the source rectangle.
Analytic HDR perspective pixels, edited negatives, all observed one-shot pixel
cancellation checkpoints, work refusal and existing similarity/reflected warp
regressions pass on macOS and Linux (eight tests each). Evidence:
`research/dedup-projective-pixels-qualification.json`. Local-feature/file/search
integration, bidirectional confirmation, real perspective fixtures, calibration
and numeric conditioning beyond this initial cohort remain incomplete; this is
not yet general perspective duplicate support.

### Independent perspective derivative gate remains failing

Six Pillow-authored projective derivatives of pinned camera previews expose
failures beyond identity/minimal synthetic tests: only one currently passes
strict bidirectional candidate thresholds. Full-support normalized Givens QR
refinement improves feature geometry, but is insufficient for these pixels.
A diagnostic using the exact generator homography and half-pixel convention
shows reverse matched fractions around 97-99 percent while forward fractions
range roughly 70-92 percent at unchanged tolerance 0.03. This isolates a
resampling contribution independently of feature estimation. Evidence:
`research/dedup-projective-generator-geometry-diagnostic.json`. Explicit filtered
projective verification, alternative-model pixel selection, negative calibration
and collection integration remain required; strict thresholds are not relaxed
and no general perspective support is claimed.

### Explicit pixel registration for planar candidates

`compare_local_files_projective_registered` now exposes a sampled, fixed-domain
registration policy independently of final candidate acceptance. Corner controls
are fitted to projective matrices; the objective can use encoded-sRGB box
windows. Trial windows cannot silently drop initially admitted samples. Work
counts include sampled grid visits and conservative initial/trial window costs;
cancellation and budget exhaustion return errors with no partial candidate.
The registered mapping is exposed separately from original keypoint evidence.
Strict residuals and full bidirectional filtered residuals remain available.

The six Pillow-authored derivatives pass on macOS with registration radius 1,
stride 8, 128 rounds and 64 million sample-work allowance. Final tolerance 0.03,
minimum coverage 0.3, minimum matched fraction 0.9 and verification radius 1 were
unchanged. All 14 independently labelled unrelated-scene pairs are rejected;
the two garden views are excluded from that negative label. These six images
have been used during algorithm development and are not an untouched evaluation
cohort or captured perspective pairs. macOS full file gates pass 11 local-scan
and 5 projective-file tests; a subsequent phase-specific cancellation/budget
and strict identity retry test also passes. Both platforms pass 12 primitive
geometry/warp tests. Linux full file qualification also passes 11 local-scan and 5 projective-file
tests in `dedup-projective-registration-window-files-linux.log`. The subsequent
phase-specific lifecycle test has not yet been executed on Linux.

This does not close collection/index integration, captured perspective/burst
calibration, broad transform recall, or measured latency/global memory limits.

### Indexed planar collection qualification

`scan_projective_local_collection` reuses descriptor retrieval, source/dependency
snapshots, native selected-frame extraction and final batch invalidation through
one generic collection lifecycle. Ordinary similarity reports retain their
existing default evidence type; projective reports carry planar/registered
mapping evidence without pretending it is similarity geometry or exact identity.
On macOS and Linux, a twelve-file corpus (six pinned previews plus six authored
perspective derivatives) produces the same candidate edge set as all 66 direct
pair comparisons. Each of the six authored positive pairs is required. These
are algorithm-development fixtures, not independent captured-pair calibration.

Both platforms pass indexed pair/work-limit refusal, one-shot final cancellation,
and mutation during initial decode, preserving the healthy pair and releasing
managed resources. macOS additionally passes generation invalidation at the final
report check, restored-generation retry, and mutation at the last source hash's
pre-stamp-validation check after pair comparison. All stale edges are removed;
Linux validation of these extended adversaries also passes. Filesystem mutations
after the final observation and ABA are not certified by these tests.

### Spatial planar collection and recursive discovery

Explicit spatial projective entrypoints now apply identical feature quotas to
index extraction and fresh pair confirmation. The macOS three-file gate matches
all direct candidate pairs and requires the known identity pair while rejecting
the unrelated scene. Recursive spatial gates preserve physical aliases across
overlapping roots, damaged-file isolation and caller-selected image indices.
Invalid grids are refused before request construction; substituted paths, file
limits and immediate cancellation yield no report or retained managed resources.
The targeted macOS tests and library Clippy pass. Linux gates are queued behind
the ongoing full run; no Linux result is claimed yet. This does not establish
independent perspective recall, broad false-positive calibration, Windows
behavior, or large-scale memory and latency bounds.

### Sampled planar search and independent selection diagnostics

Explicit seeded fixed-trial projective sampling now shares source/decode,
registration, residual confirmation and final source invalidation across direct
files, indexed collections and recursive roots. Primitive tests pass on both
platforms, including a 200-correspondence synthetic case with 80 outliers and
every observed primitive cancellation checkpoint. Recursive request/alias/error
gates pass on both platforms. macOS recovers all six authored perspective pairs
and the indexed twelve-file result matches all 66 direct comparisons using 500
features and 2048 trials. The additional fourteen unrelated-source assertions and indexed/direct
equivalence gate now pass on macOS and Linux: all six authored positives are
required and all fourteen unrelated source pairs reject. Sampled collection
late generation/cancellation/source mutation and retry gates also pass on both
platforms. These are finite development fixtures. Fixed trials imply no
statistical confidence, guaranteed recovery, or bounded global allocator RSS.

The complete 580-pair HPatches spatial selection diagnostic found 60 geometric
models and 5 candidates versus 98 models and 8 candidates for global top-32
selection, with no candidate gains and three losses. Spatial selection remains
explicit rather than promoted as an improvement. The global top-500 sampled
comparison is still running. Publisher correspondence labels are not duplicate
labels, and these counts do not prove duplicate recall or unrelated-pair precision.

## Anchored projective registration lifecycle evidence

The explicit one-pixel corner-control registration mode passes the 12-file development gate on macOS and Linux: all 66 indexed pair outcomes match direct comparison, all six authored perspective positives are required, and all fourteen unrelated base-capture pairs reject. Separate lifecycle gates on both platforms cover late generation invalidation, late cancellation, work/pair refusal, removal of changed-source edges, healthy-pair preservation and managed memory release. Evidence: `research/dedup-registration-anchor-exhaustive-state.json` and `research/dedup-registration-anchor-lifecycle-state.json`. These finite fixtures do not certify filesystem ABA safety, continuous displacement bounds, captured burst accuracy, or the full contract. The all-580 independent corpus and fresh current-source broad regressions remain in progress.

## Explicit perspective and photometric mode

`verify_projective_photometric_filtered` retains unfitted strict/window evidence beside a bounded per-channel affine fit on both perspective grids. `compare_local_files_projective_photometric` and the indexed/recursive photometric collection APIs share existing native decode, source/dependency snapshots, cancellation and managed resource lifetimes. Primitive tests pass on macOS and Linux; the four-file direct/index gate passes on both, and recursive/lifecycle development gates pass on macOS. Six independently authored combined perspective/light positives and fourteen unrelated negatives are under evaluation. An unconstrained affine fit outside caller bounds remains an explicit refusal; constrained least squares never changes those bounds or residual thresholds. This mode is not yet qualified on captured perspective/light pairs, burst/look-alike labels, all platforms, or the complete contract. Evidence: `research/dedup-projective-photometric-fit-state.json`, `research/dedup-projective-photometric-collection-state.json`, `research/dedup-projective-photometric-roots-state.json`, `research/dedup-projective-light-photos-state.json`.

## Two-lane registration evidence

The explicit portfolio keeps anchored bounded-color and unanchored absolute-color registration models separately, reserves the sum of both work caps before processing, and requires complete strict/window verification on both bidirectional grids. Selected-frame files, metric-index collections and recursive root APIs share source/dependency validation and atomic cancellation. On the independent `v_bees/4` regression, the anchored lane rejects while the unanchored lane passes unchanged final admission, recovering the prior candidate. This is one recovery diagnostic, not duplicate accuracy. Lifecycle development gates pass on macOS and Linux; macOS direct/index and recursive gates pass. The all-66 development corpus and complete all-580 independent portfolio measurement remain running. Integrity verification checks both lane matrices/counts, fixed admission flags, selected primary evidence and explicit error coverage. Native Windows, captured burst/look-alike calibration, broader color/RAW/resource qualification and the other contract rows remain open. Evidence: `research/dedup-registration-portfolio-lifecycle-state.json`, `research/dedup-registration-portfolio-files-state.json`, `research/dedup-hpatches-v-bees-portfolio-macos.json`, `research/dedup-hpatches-portfolio-state.json`.
