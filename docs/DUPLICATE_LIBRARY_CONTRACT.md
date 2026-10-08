# Duplicate search library acceptance contract

The requested library covers both byte-identical files and visual image matches.
Exact file identity, normalized-pixel equality, region containment and visual
similarity are separate evidence classes. A score never proves identity. The
library reports results and errors and never deletes files.

Completion requires every row below, including independent positive and negative
fixtures. Partial coverage is not silently promoted to general support.

Current four-search integration checkpoint: the shared-view registration,
two binary-pyramid filters and gradient-pyramid file API now passes the explicit
edited macOS compound test. It checks all four identity decisions, sticky
first/middle/final cancellation, each cumulative budget one unit short before
allocation, and failure of the fourth lane even when earlier lanes accept.
The two native Linux complementary-file/collection tests also pass with all
selected sources unchanged during the run. All229 Copydays constituent-parity
measurements now finish:35 candidates versus the original30, five gains and no
lost candidates. Every reported constituent decision, geometry and pixel count
equals its fixed standalone reference; all386 normalized inputs rehash.194
publisher-origin copies remain omitted. Evidence:
`research/dedup-four-lane-full-parity-audit.json`. A
subsequent empty-input adversary exposed a missing final cancellation checkpoint
in gradient matching. The fix passes all eight gradient tests on both macOS and
Linux, including empty/one-sided-empty cancellation and retry; original failing
evidence is retained in `research/dedup-gradient-empty-cancel-state.json`. The separate
public gradient file API finishes936 negatives across the control and all five
new-gain queries, with exact parity to diagnostic native fields and no candidates.
The integrated four-search API also finishes the same six-query936 negative
suite with no candidates, errors or timeouts. A separate terminal audit checks
all six report hashes, ordered different-origin labels, zero retained managed
memory and all386 normalized image hashes. Its evidence remains finite and does
not qualify all-query semantic/burst precision:
`research/dedup-four-lane-six-query-negative-terminal-audit.json`.
The HPatches budget-corrected measurement now finishes all580 pairs, with579
successful executions,12 candidates and one explicit `Pixels(Invalid)` refusal.
All696 prepared images and the final record invariants verify. The original441
records reconstruct the byte-identical pre-resume checkpoint. Independent
denominator signs prove that the retained refusal's global500 native model
crosses the full source rectangle. A fixed spatial4x4/32-per-cell diagnostic
removes this refusal on that case and preserves two existing candidates; it is
not promoted as a default or claimed broadly qualified from three cases.
Evidence: `research/dedup-hpatches-portfolio-budget-full-integrity.json`,
`research/dedup-hpatches-tools3-horizon-audit.json` and
`research/dedup-hpatches-spatial-portfolio-three-cases.json`.
These finite checks do not establish semantic/burst negatives,
collection integration, native Windows, allocation completeness or every row.
Evidence: `research/dedup-four-lane-state.json`,
`research/dedup-gradient-public-negative-parity-state.json` and
`research/dedup-hpatches-portfolio-budget-state.json`.

An explicit alternative gradient descriptor now interpolates contributions in
both spatial dimensions as well as direction. Eleven primitive tests pass on
macOS and Linux, including the original seven gradient tests and empty-input
cancellation. Independent fixtures show smaller descriptor change for a
one-pixel edge-localization error, quarter-turn/affine-light consistency and
flat/alpha/work/cancellation refusals. This is not a scale detector or
SIFT-equivalence claim. Explicit recipe selection now integrates extraction,
pyramids, managed output and the source-guarded file API. Seventeen extraction/
pyramid tests pass on macOS and Linux; the macOS file gate checks identity,
unrelated rejection, first/middle/final cancellation and limits. On two real
controls, the fixed recipe's native fields equal the earlier frozen reference.
Interpolation retains200001 but loses201501 and214102. Its all229 real-copy
measurement completes with28 candidates versus fixed27: three gains and two
losses.207302 and208102 are new to the four-search API's35. Each rejects all156
different publisher-origin groups (312 comparisons, no candidates/errors).
The third gradient gain214501 is already found by the existing four-search API;
its156 negatives also complete without candidates or refusals, giving468
audited comparisons across all three gains. Evidence:
`research/dedup-gradient-interpolated-three-gain-negative-audit.json`.
The native shared-view five-search file run now recovers37 of229, retaining
all35 four-search candidates and adding207302/208102. All constituent reported
fields equal the fixed standalone references;386 normalized inputs rehash.
192 origin copies remain omitted. This is not an indexed full-collection result. Interpolation is not
promoted as a replacement default. Evidence:
`research/dedup-gradient-interpolated-full-paired-audit.json` and
`research/dedup-gradient-interpolated-two-new-gain-negative-audit.json`.
Source method reference:
https://www.cs.ubc.ca/~lowe/papers/ijcv04.pdf (section6.1).
No upstream implementation was copied. Evidence:
`research/dedup-gradient-interpolated-state.json`.

The automatic regional file API now fits geometry and confirms its generated
4x4 regions within one decoded-source lifecycle. All 229 Copydays pairs match
the previous pinned regional diagnostic in geometry, coordinates, pixel counts
and decisions: 27 whole candidates, 118 region-supported pairs, 672 supported
regions; all 386 input images rehash. Evidence:
`docs/research/dedup-native-region-grid-full-audit.json`. Indexed regional
collection admission, mutation removal, cancellation/retry and direct parity
pass the authored three-source fixture on macOS and Linux. Its real corpus
retrieval-loss audit completes all 229 two-source pairs with no lost whole or
regional confirmation; all 386 inputs rehash. One fixed regional-only query
also completes 156 different-origin negative comparisons with no whole or
regional positives. See `research/dedup-indexed-region-grid-full-audit.json`
and `research/dedup-indexed-region-grid-negative-full-audit.json`. These frozen
executables predate later memory-lifetime accounting changes. Regional support does not establish
whole-image equality, independent region-mask truth or semantic/burst precision.


The explicit `scan_projective_local_collection_five_all_regions` path retains
binary-grid evidence and independently verifies regions under both native gradient
models. All three regional phases are admitted before I/O; the existing whole
candidate remains independent. macOS fixture parity, first/final cancellation,
gradient-phase cancellation, source mutation/removal and restored reversed-order
retry pass. Supplied-model file/grid primitives pass five native Linux tests;
The joined collection also passes three native Linux fixture tests with its
selected sources unchanged (`research/dedup-five-gradient-regions-linux-state.json`). All 58 previously
unconfirmed model-bearing Copydays pairs complete native regional confirmation:
26 have supported gradient-model regions (40 fixed and 52 interpolated region
supports; these counts do not represent union area). Whole and binary-regional
fields remain equal to their pinned references. Evidence:
`research/dedup-extra-gradient-regions-58-full-audit.json`. The recovered-query
negative control completes all 156 different-origin originals with zero whole
or regional positives (`research/dedup-gradient-region-negative-full-audit.json`);
this is one query, not all-query precision. The later unique-union memory change passes
two macOS and two native Linux fixture tests and three real native control
comparisons with all native fields equal except the managed peak; evidence:
`research/dedup-five-union-memory-state.json` and
`research/dedup-unique-union-controls-audit.json`. Checkpoint audits are finite prefixes, not full recall
or precision qualification. Evidence: `research/dedup-five-all-regions-state.json`,
`research/dedup-region-transform-linux-state.json`,
`research/dedup-extra-gradient-regions-58-checkpoint-audit.json` and
`research/dedup-gradient-region-negative-checkpoint-audit.json`.

The frozen unique-union executable now completes and independently audits all
229 manifest-order two-source confirmations and all 386 normalized image hashes.
Whole candidates remain 37; 146 pairs have native regional support, including
all 26 previously recovered gradient-region pairs. All whole/binary constituent
fields retain pinned parity and gradient domains/counts/admission are independently
checked. The remaining 83 pairs split into 51 without either gradient model and
32 with a gradient model but no accepted region. This does not promote regional
support to whole-copy identity, qualify precision, or test one 386-source
collection. The executable predates subsequent metadata/managed-tree/descriptor
union changes. Evidence: `research/dedup-five-all-regions-229-full-audit.json`
and `research/dedup-five-all-regions-unconfirmed-83.json`.


Five-family retrieval also reserves its outer binary-file list and temporary
insufficient-ID list before allocation and reconciles actual capacity. Two
macOS and two native Linux fixture tests pass; these gates predate the following
visual-index change. Evidence: `research/dedup-five-metadata-memory-state.json`.
The visual index now skips identical complete variants within each application
ID, retaining original fingerprints for exact final comparison. Its two-ID,
two-distinct-variant fixture owns four internal slots, and the storage/cancellation
unit plus 15 independent metric/collision/limit/oracle tests pass on macOS.
Two further macOS collection parity/admission tests now pass with selected
sources unchanged. Native Linux qualification now passes all 55 selected
library/metric tests with the selected index sources unchanged
(`research/dedup-distinct-variants-linux-state.json`); no throughput or
RSS reduction is claimed. Evidence: `research/dedup-distinct-variants-state.json`.
Native binary-index maps, buckets, query scratch and decoder allocations still
need complete memory accounting and full collection scale qualification.
The current 229-pair all-regional corpus uses the frozen unique-union executable
from before these metadata-credit and distinct-variant changes. It cannot
qualify their memory behavior or performance.


The frozen indexed five-search plus binary-grid collection run completes all
229 two-source Copydays pairs: 37 whole candidates, 118 region-supported pairs,
no loss relative to the pinned constituents, and all 386 images rehash. Evidence:
`research/dedup-five-regions-full-audit.json`. This run predates later memory
changes and is separate from the still-running all-gradient-regions corpus.
The latest metadata/distinct-variant executable also preserves all 15 proposals,
feature/hit counts and edge decisions on six sources in one collection. Its
managed peak is 47,002,800 bytes; independently measured process RSS is
70,860,800 bytes, exceeding 64 MiB despite the managed peak staying below it.
Thus managed-credit admission is not a full-process RSS ceiling. Evidence:
`research/dedup-distinct-variants-six-file.json`. This finite measurement does
not qualify a full 386-image collection, broad performance, decoder allocation
coverage or a hard RSS limit.


Five-family binary retrieval now owns its flattened feature-to-file owner table
through managed storage. The table reservation precedes allocation and remains
live through indexing and candidate lookup. A native macOS fixture reproduces
legacy pair/hit/insufficient-ID outputs, confirms an exact 48-byte owner-table
limit, rejects a 47-byte limit before owner allocation, and releases credit on
first/mid/final cancellation. Empty input and duplicate-ID refusal preserve the
zero-credit contract. All 15 minimal-feature metric/oracle tests also pass.
Two further macOS collection parity/admission tests now pass with selected
sources unchanged. Native Linux qualification passes all 58 selected tests
(41 library, 15 metric/oracle and two collection/admission tests), with selected
source hashes unchanged; evidence is in
`research/dedup-binary-owner-slots-linux-state.json`. Three fresh real-pair
controls also preserve all native evidence fields except managed peak memory;
see `research/dedup-binary-owner-slots-controls-audit.json`. This addition does not
cover native tree/hash-map/query-scratch or decoder allocations. Evidence:
`research/dedup-binary-owner-slots-state.json`. The current 229-pair corpus
executable predates this owner-table change.

The subsequent construction-staging change reserves the exact flattened
fingerprint capacity before allocation and retains its credit through native
index construction. Owner-table credit continues through retrieval. Its updated
fixture checks combined exact capacity, one-byte-short staging refusal, mixed
recipes and cancellation release. This fixture and all 14 current minimal-feature
metric/oracle tests pass on macOS with selected implementation sources unchanged.
Both current macOS collection/admission tests also pass. Three fresh real-pair
controls preserve all native evidence fields except managed peak; audit:
`research/dedup-fingerprint-staging-controls-audit.json`. Native Linux
qualification passes all 58 selected tests with the tracked implementation
sources unchanged. The fresh six-file collection preserves all 15 pair
outcomes and the same three positive edges. Managed peak is 47,002,800 bytes
with zero retained credit, while measured process RSS is 74,612,736 bytes;
the managed budget is still not a complete process-memory ceiling. Audit:
`research/dedup-fingerprint-staging-six-file-audit.json`. The preceding
owner-only results do not qualify this change. Native
trees, maps and query scratch remain outside this reservation. Evidence:
`research/dedup-fingerprint-staging-state.json`.

Fresh-executable precision qualification now runs a sequential batch for the
other 25 gradient-region recoveries plus a separate 208101 query control, each
against all 156 different-publisher-origin originals. Every query must finish
its native comparisons and independent full report audit before it is counted
as complete; native errors are never counted as rejections. The batch uses the
fixed 26-query recovery selection from the prior 58-pair audit, not a sample
chosen after observing new negatives. Partial checkpoints do not prove all-query
precision. Evidence: `research/dedup-staging-recovered-gradient-negatives/state.json`
and `research/dedup-staging-gradient-negatives-208101-state.json`.
The separate 208101 control now finishes and independently audits all 156
comparisons and 157 image hashes, with no whole-image or regional positives.
It uses the frozen fingerprint-staging executable, predating the managed-tree
integration; the other 25-query batch is still running. Full audit:
`research/dedup-staging-gradient-negatives-208101-full-audit.json`.

The subsequent binary-index implementation now uses flat budget-owned BK-tree
nodes for all four channels, including collision members and bounded child
links, plus retained fingerprint/owner storage and budget-owned visual search
scratch/results. Its primitive macOS test matches independent exhaustive
Hamming search across the declared radii and checks retained result credit,
work fallback, refusal and release. Integrated descriptor parity/admission and
all 14 current minimal-feature metric tests pass on macOS. A separate complete
recipe oracle passes both transform settings across all declared radii,
including repeated variants, `u64::MAX` IDs, duplicate refusal, zero-budget empty
input and cancellation at every observed construction/search checkpoint.
Both macOS collection/admission tests pass; the only source change during that
gate was addition of the cfg(test) oracle module, whose removal reconstructs
the exact pre-gate source hash. Three fresh native real-pair controls preserve
all evidence fields except managed peak; audit:
`research/dedup-managed-index-controls-audit.json`. Native Linux qualification
passes all 60 selected tests with tracked implementation sources unchanged.
The fresh six-file measurement preserves all native fields
except managed peak, including all 15 outcomes and the three positive edges;
managed peak is 47,002,800 bytes with zero retained credit, and process RSS is
59,260,928 bytes. This is one pinned workload measurement, not a universal RSS
ceiling or an isolated causal/performance comparison. Audit:
`research/dedup-managed-index-six-file-audit.json`. These finite controls
do not qualify the full 229-pair or 386-source collection on this version.
Descriptor-level union maps, per-feature targets and pair-count metadata remain
outside this reservation, as does decoder memory. Evidence:
`research/dedup-managed-metric-state.json` and `research/dedup-managed-index-state.json`.

The subsequent descriptor-union change replaces the managed retrieval path's
temporary ID/distance hash map with admitted flat storage for actual hits,
deduplicating opaque IDs by minimum distance before deterministic ordering.
An isolated same-dependency harness passes independent variant-pair distance,
result-limit, shared-credit and every-checkpoint cancellation tests. The helper
is now connected to native file-pair retrieval; native qualification is running.
The preceding 60-test Linux gate predates this union change. Per-feature target
sets, pair-count and document metadata remain outside this reservation. Evidence:
`research/dedup-managed-descriptor-union-state.json`.

| Requirement | Current evidence | Remaining qualification/work |
| --- | --- | --- |
| Renamed copies, arbitrary types, empty files | `exact_files.rs`: streaming BLAKE3 and full byte confirmation | Broader filesystem/platform corpus; injected sample/full collisions now pass byte-confirmed partitioning |
| Same size, different contents | Equal-size and equal-edge/different-middle negatives | Injected constant sample/full digest fixture passes; wider platform qualification remains |
| Repeated paths, symlinks, hardlinks | Unix overlap, aliases and followed-cycle fixtures | Windows volume/file-index implementation cross-checks; native Windows identity/hardlinks/replacement tests and other non-Unix targets remain pending |
| Missing, unreadable, changing files | Per-file errors, snapshot validation, equal-length mutation, decode-time mutation | Representative attribution/retry and FIFO replacement pass; non-UTF8 diagnostics pass on macOS; current Linux non-root standalone exact-source tests qualify non-UTF8 files and read denial; native decoder and non-Unix coverage remain |
| Cancellation and large files | Bounded exact reads, index-node/bucket cancellation, explicit resource errors | Core/ICC conversion cancellation checkpoints pass; indivisible ICC setup, underlying decoder latency and large-file measurements remain |
| Recursive directories and overlapping roots | Deterministic traversal, configurable symlink/depth/entry policy; authored 192-file nested exact-search corpus matches all 48 three-copy groups on macOS and Linux, with 48 equal-size negatives, overlapping roots, reversed order, limit diagnostics and cancellation/retry | scan_visual_roots integrates bounded discovery, aliases and diagnostics; broader symlink/platform and visual traversal corpus remains |
| Same pixels, different encoding/metadata | Canonical RGBA8/linear HDR equality; independent seven-format opaque and five-format alpha fixture collections pass indexed retrieval in both orders on macOS/Linux (21 opaque and 55 alpha equal pairs; visible edits excluded); metadata/compression and associated-alpha gates | confirm_pixels/confirm_pixel_groups integrated with fresh source validation; broader independently encoded cross-format, HDR/ICC and real-world corpus remains |
| RAW versus developed raster | Native full development; independent full-resolution DNG/NEF/ORF TIFF pairs pass strict spatial geometry/pixels; one unrelated RAW/TIFF pair rejects | Strict CR2 render still rejects; explicit display-projection mode accepts the known four-camera RAW/TIFF collection with 24 cross-scene negatives rejected. Broader camera/development/color and held-out/burst/look-alike qualification remain |
| Resize/recompression/exposure/color changes | Explicit filtered policies accept 20/20 real preview positives at radius 3 in both linear and encoded sRGB; strict residuals retained | Held-out calibration, real bursts/look-alikes, scalable collection search and confidence evidence remain |
| EXIF orientation/rotations/reflections | D4 fingerprint/index oracle; decoder orientation path | Eight-orientation independent TIFF matrix passes. Sixteen CMYK/YCCK JPEG EXIF cases also pass on macOS/Linux against orientation1 normalized pixels; independent JPEG color decoding, other formats and broader orientation corpus remain |
| Crops/borders/watermarks/edits | Exact unscaled HDR/alpha region containment, explicit coordinates and budgets | Native local geometry and bidirectional pixels, dyadic/intermediate scales and quarter turns tested. The fixed six-search Copydays gate recovers 155/229 (74 misses); the explicit source-resolution area/grid8/radius7 recipe recovers seven accepted regions in 200101.jpg and preserves exact file/two-source collection evidence parity. The six-source/all-15-pair single-recipe collection passes exact fresh parity, recovers first/second crops with7/15 regions and misses the third. The explicit compound recipe recovers the third in a two-source collection with17 correspondences/10 inliers/47 regions and exact fresh file parity. Same-recipe156-negative gates are running. Arbitrary-angle coverage, remaining real crop misses, full corpus collection and broad precision remain unqualified |
| Uniform images and burst negatives | Information gates, informative checker collision and separate exact selected-frame discovery, including uniform PNG/BMP | Real captured uniform/burst negatives and semantic false-positive calibration |
| Transparency/HDR/color profiles | Canonical invisible RGB, signed-zero normalization, HDR precision, per-page TIFF ICC; independent LittleCMS normal/boundary grids expose and qualify single-gamma zero-endpoint correction | Broad ICC/alpha/HDR qualification, cross-profile precision policy and untagged color policy |
| Animation and multipage images | All composited GIF/APNG/WebP frames, exact timeline/repetition; DCX/ICO/CUR/TIFF pages and selected-page ICC | Timeline splits/zero-duration prefixes pass across GIF/APNG/WebP; full-container bounded batch/exhaustive search integrated; broad disposal/container corpus and scale performance remain |
| Unsupported/corrupt images | Exact scan independent of image decoding; explicit visual errors | Combined recursive directory/visual report integrated; broader malformed format corpus remains |
| Grouping | Complete-link partition tested on all 64 four-node graphs; all accepted edges retained | Pixel-confirmation groups now revalidate batch-wide source snapshots and remove changed-source edges; complete-container grouped scanner integrated; recursive complete-presentation grouping integrated with explicit per-path modes; automatic content-based mode detection/recursive grouping integrated; broader concurrent-mutation corpus remains |
| Repeated scans | Content/recipe/frame-keyed bounded checksummed cache, atomic save, per-file and batch reuse; mutation rejected before admission | Automatic complete decoder/recipe identity; broader platform/power-loss cache qualification (macOS process publication/exit fixtures below) |
| Resource budgets and scale | Byte/pixel/frame/cache/work/result budgets; streaming source reads and metric indexes | Indexed construction limits/cancellation details, decoder allocation completeness, measured scale/latency/memory |

The shared-view fifth-search file API is now implemented explicitly, preserving
the existing three/four-search entry points. Its macOS compound gate checks all
five identity decisions, original fixed-gradient coordinates, sticky cancellation,
each aggregate work limit one unit short before I/O, and refusal of the last
search after earlier positive decisions. The full229 native positive measurement completes with37 candidates, two gains
over the four-search API and zero losses;192 origin copies remain omitted. The integrated five-search
nine-query1404 different-origin suite completes with zero candidates, errors or
timeouts; terminal integrity checks native decisions, input hashes and available
four-search/interpolated references. This is not all-query semantic/burst precision. Native
Linux complementary-file tests pass with selected source hashes unchanged. A
separate late source-change/retry gate passes on macOS and Linux: changing the
source after the four-search baseline checkpoints yields the specific
`Source(SnapshotError::Changed)` error on macOS and Linux, discards all evidence, releases
managed reservations and permits a successful retry after restoration. Selected
Linux source hashes remain unchanged. The report verifier rejects
seven forged prefix reports (mode, ownership, peak, boolean decisions, candidate,
geometry and order). Evidence: `research/dedup-five-lane-state.json` and
`research/dedup-five-lane-verifier-adversaries.json`. The complete integrated run independently verifies37; evidence:
`research/dedup-five-lane-full-parity-audit.json`.

The spatial HPatches run independently verifies all580 ordered pairs:
12 retained candidates,567 shared rejections and one execution recovery.
The recovered execution is separate from a candidate gain. Semantic/burst precision and default suitability remain unqualified. Evidence:
`research/dedup-hpatches-spatial-portfolio-paired-full.json`.

## Current completion audit

The fixed regional-grid diagnostic completes all229 strong publisher-copy
pairs:27 whole candidates,118 pairs with supported regions,91 regional-only
pairs and672 supported regions. Geometry/whole decisions equal the fixed
radius3 reference and all386 normalized inputs rehash. The separate all-query
negative suite also completes all35724 comparisons against different
publisher-origin groups: all are successful, with zero whole candidates and
zero regional supports. Terminal audit rechecks229 report hashes, ordered
labels, native support counts, released reservations and386 input hashes.
This is finite publisher-origin evidence, not semantic/burst precision or
region-groundtruth-mask validation. Evidence:
`research/dedup-copydays-region-grid-positive-full-audit.json` and
`research/dedup-regional-all-query-negative-full-audit.json`.

Managed gradient-pyramid reservations now pass12 all-feature macOS tests,
including equality with unmanaged output, last-owner retention/release,
exactpeak and one-byte-short admission, work refusal before allocation and
first/middle/final cancellation/retry. Whole-process RSS and decoder allocation
completeness are not established. The real three-level gradient measurement
recovers201501 previously omitted by the complementary portfolio. Both control
200001 and gain201501 reject all156 different publisher-origin groups without
pixel/process/time refusals; the all229 positive gradient measurement now finishes with27 candidates and no refusals. Five candidates are new to the prior complementary portfolio, giving35 in their diagnostic union. All five new gains plus the control each reject156 different publisher-origin groups (936 total), with ordered labels, reports and input hashes audited. This is not yet an integrated fourth-lane file/index result. Evidence: `research/dedup-gradient-pyramid-full-audit.json` and `research/dedup-gradient-six-query-negative-audit.json`.

An explicit opaque-linear gradient histogram recipe and mutual squared-distance
matcher are now implemented as separate primitives. Supplied positions, scales
and orientations are not yet detected automatically. Four authored macOS tests
pass affine-light normalization, independently constructed quarter-turn texture,
opaque-alpha refusal, uniform/rotated-flat rejection, nearest-neighbor ties in
both directions, invalid descriptors, work admission and cancellation/retry.
A rotated-flat adversary first exposed weighted-interpolation roundoff creating
false gradients; difference-form interpolation fixes it without an energy
threshold change. Initial failure and corrected logs remain preserved.
These primitives are not SIFT-equivalent and establish no real-copy recovery,
native Linux, managed allocation or file/index coverage. Evidence:
`research/dedup-gradient-primitive-state.json`.

Gradient pyramids now reuse the same generic native area reduction and original
pixel-center mapping as existing binary pyramids. Eleven no-default-feature
macOS tests pass, including all existing pyramid tests and a separate96x96
random texture enlarged by independently replicating every pixel twice in each
axis. At least ten recovered gradient correspondences exactly satisfy the
pixel-center scale2 relation. One-level extraction equals the original gradient
extractor; cumulative work admission and cancellation remain explicit. This
authored scale fixture does not qualify continuous scales or real edited copies.
All-feature managed regression and native Linux/current real-copy gates remain
pending in the gradient state artifact.

Local dominant-gradient orientation now has a separate primitive: an opaque
integer-centered16x16 patch,36 circular bins,six smoothing passes and peak
interpolation. Distant tied peaks and uniform patches return no orientation.
Five gradient tests pass, including analytical0/45/90-degree slopes, work and
boundary admission and cancellation. This supplies one local angle; it does
not implement scale detection, multiple orientation hypotheses or automatic
file/collection feature extraction. The full real-copy gate remains pending.

The gradient feature extractor now combines native strongest-corner selection,
single dominant orientation and unit-scale descriptor windows inside an explicit
13-pixel boundary margin. Admission requires512 grid sites per maximum detector
feature before extraction; pixel/corner budgets stay separately bounded.
Six macOS gradient tests pass deterministic extraction, exact self-matching,
unrelated texture below ten correspondences, coordinate bounds, one-site-short
work admission, cancellation/retry and the earlier descriptor/orientation gates.
This primitive is unmanaged and single-scale; these authored gates do not prove
held-out copy recall, file lifecycle, indexed retrieval or allocation completeness.

All six gradient primitive tests also pass on native Linux with the three
selected source/test files unchanged. The first fixed real-image diagnostic
executes the three prior omissions plus recovered pyramid control200001 through
unit-scale gradients,4096 seeded projective trials and unchanged radius3
constrained pixel policy. Correspondence counts are5/0/3/0; none obtains
geometry, so pixel verification is unattempted rather than reported as passed.
The loss of the control prevents default promotion. Multiscale descriptor and
detector robustness remain unresolved. Evidence:
`research/dedup-copydays-gradient-four-cases.json` and
`research/dedup-gradient-primitive-state.json`.

The fixed native pyramid probe also diagnoses three strong-subset omissions
(`200101`, `200201`, `200301`) at the original corner threshold and zero threshold,
with one/two/three pyramid levels. All six executions finish successfully;
none obtains geometry. At three levels, `200301` source features increase from
146 to 834 but mutual correspondences only from one to two. Lower detector
admission alone does not recover these three cases. This finite diagnosis does
not identify the complete cause or qualify arbitrary edits; descriptor and
correspondence robustness remain work. Evidence:
`research/dedup-copydays-low-contrast-three-omissions.json`.

A separate fixed diagnostic replays all four descriptor variants and records
distance, forward-ratio, mutual and reverse-ratio admission on these three
omissions plus recovered `200001`. All twelve level-specific final counts equal
native correspondences. At three levels the omitted cases have respectively
664/772/146 features inside distance64, but only 7/9/6 pass forward uniqueness
and 1/6/1 survive both directions. The recovered control yields 18 matches.
This finite evidence localizes admission loss to ambiguity rather than the
absolute-distance threshold alone; it does not prove which correspondences
are physically correct. No production thresholds were relaxed. Evidence:
`research/dedup-copydays-match-admission-four-cases.json`.

An explicit spatial-oriented pyramid primitive now applies normalized cell
quotas separately at every level, retaining original-image coordinates.
Four no-default-feature pyramid tests pass, including one-level equality to the
existing spatial extractor, multilevel coordinates, pixel/feature budgets,
overflow, cancellation and retry. A separate managed variant now reserves
grid scratch and retained features through the shared ordinary/spatial admission
path. Five all-feature macOS pyramid tests pass, including output equality,
last-owner release, one-byte-short peak admission and first/middle/final
cancellation/retry for both variants. This is managed allocation evidence,
not whole-process RSS or native Linux qualification. The fixed4x4/quota32 diagnostic
finishes eight runs on the same four cases; baseline fields remain exactly
unchanged. Spatial selection improves control inliers15->17, but obtains no
geometry on any of the three omissions (final matches2/9/1). Thus this grid
alone does not close these gaps and is not promoted as a default. Evidence:
`research/dedup-spatial-pyramid-state.json` and
`research/dedup-copydays-spatial-pyramid-four-cases.json`.

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

### Explicit shared-geometry filter portfolio (2026-10-07)

`compare_local_files_projective_pyramid_filter_portfolio` retains the primary
radius-1 photometric evidence and separately records an explicit secondary
filter's residual counts, fit refusal and acceptance. It extracts/matches once
and estimates geometry once. Both filters run inside the existing shared
source/dependency and sticky cancellation lifecycle. Checked cumulative window
work admission precedes decoding; runtime refusal of the secondary phase
invalidates the whole result even if the primary accepted.

The native macOS identity/unrelated gate passes, including cumulative cap
one-short rejection before managed allocation, secondary zero-work refusal,
first/middle/final one-shot cancellation and reservation release. The independent
Copydays `203102.jpg` pair, lost by radius-3-only smoothing, is retained with
accepted filters `[true, false]`. These gates do not establish broad edited-image,
burst/semantic, native Windows, collection or whole-process memory coverage.
The immutable-probe full run completes all 229 pairs without errors: 28 are
accepted and 201 remain omitted. Both filters' reported native geometry, counts,
fit refusals and decisions equal the respective standalone measurements on
all 229 pairs; all 386 normalized images were rehashed. Two query-specific
different-origin runs complete all 312 comparisons with zero candidates. An
offline union with the previously measured registration API recovers 30 pairs,
but this is not an integrated three-policy API or a collection qualification.
Evidence:
`research/dedup-pyramid-filter-portfolio-state.json`,
`research/dedup-pyramid-filter-portfolio-loss-recovery.json` and
`research/dedup-pyramid-filter-portfolio-cancel-macos.log`.

### Integrated registration and both pyramid filters (2026-10-07)

`compare_local_files_projective_complementary_filter_portfolio` runs the fixed
registration portfolio and both explicit pyramid pixel filters on shared selected
views. Original two-search evidence/acceptance remains available unchanged;
secondary residuals/refusals and the three acceptance flags remain separate.
Feature matching and native geometry are not repeated for the second filter.
The checked outer cap admits the existing portfolio's work cap plus the secondary
filter cap. Decode, source/dependency checks and sticky cancellation still own
the entire result. All phases complete before acceptance is returned.

The authored native macOS gate passes: identity accepts all three, unrelated
images reject, primary geometry and decisions are retained, cap one-short refuses
before managed allocation, and final-phase zero work invalidates earlier success.
All existing complementary cancellation/source-mutation adversaries also pass.
An independent Copydays `201101.jpg` gain is observed with acceptance flags
`[false, false, true]`. Full229 recovery and negative qualification of this new
integrated API remain pending. This does not resolve the original acceptance
matrix's remaining edited-image, burst/semantic, RAW-rendering, whole-process
memory, platform or collection requirements. Evidence:
`research/dedup-complementary-filter-portfolio-state.json`,
`research/dedup-complementary-filter-portfolio-first-gain.json`.

The corresponding explicit collection/root APIs now use the existing shared
union-descriptor proposal extractor with the integrated three-phase verifier.
No matching or geometry is applied to merged ambiguous descriptors to reject
pairs. The native authored three-file gate passes against exhaustive direct
comparisons, including identical proposal feature counts, nested overlapping
roots, first/middle/final cancellation, preallocation cumulative admission and
attributed final-filter work refusal. Production Copydays indexed retrieval,
native Linux for these integrated APIs and broad semantic/burst negatives remain
pending. Evidence: `research/dedup-complementary-filter-collection-macos.log`.
The integrated file API also completes two finite query-specific different-origin
negative gates (312 comparisons, no accepted pair):
`research/dedup-complementary-filter-portfolio-negative-state.json`.

The fixed production collection probe now admits both feature families for two
files (4000 total features, at most 16 million distinct-ID retrieval hits including
same-file hits). Three independent strong Copydays pairs pass indexed retrieval
and fresh integrated verification: `201101.jpg` (secondary-filter gain),
`203102.jpg` (primary-filter preservation), and `207502.jpg` (prior hit-budget
adversary). All reported constituent native geometry, counts, fit refusals and
decisions match their fixed standalone measurements. This is a three-pair gate;
full229 collection measurement is running and has not yet been qualified.
Evidence: `research/dedup-complementary-filter-collection-three-cases.json`.

The integrated complementary file and collection gates now both pass native
Linux (two tests, terminal exit0); selected implementation/test hashes are
checked before/after separately from a whole-tree snapshot. Evidence:
`research/dedup-complementary-filter-portfolio-linux.log` and
`research/dedup-complementary-filter-linux-source-tracking.json`.
Remaining fixed-policy Copydays omissions are separated explicitly: 76 have no
geometry from either feature family and 123 have geometry but fail pixel
confirmation (30 accepted in the previously measured constituent union). These
are not resource/decode failures. Diagnostic encoded-sRGB radius3 verification
rejects the first eight geometry-bearing omissions with unchanged native
geometry; changing the declared color space alone does not recover those eight.
No default is promoted and no acceptance threshold is relaxed. Evidence:
`research/dedup-copydays-remaining-case-diagnosis.json` and
`research/dedup-pyramid-encoded-blur-eight-omissions.json`.

### Explicit projective regional color evidence (2026-10-07)

`verify_projective_regions_photometric_filtered` confirms two caller-declared
pixel-center rectangles under a fixed projective transform, retaining whole-frame
unfitted evidence separately. Fit/residual centers must map inside both domains;
filter windows still use original pixels and may extend beyond those domains.
Regional source-pixel counts describe the explicit rectangles. No whole-image
candidate or identity flag is produced. Cumulative work admits a whole-frame
unfitted pass plus two regional fit/residual passes before work; model/horizon
and cancellation guards still apply. Invalid/empty/overflow/outside rectangles
are explicit errors, and disjoint mapped domains refuse insufficient fitting.

Authored edited-half evidence passes: the remaining region confirms while the
whole image remains discrepant. Full-domain fitted and unfitted evidence equals
the existing API. First/middle/final cancellation and work cap one-short tests
pass. All four native macOS photometric/filtered/projective/warp groups pass.
Initial fixture radius0 was invalid under existing filtering admission; after
using radius1, a new domain/window test exposed incorrect guarding of window
samples rather than centers. That implementation was corrected; failed logs are
retained separately. Real edited/cropped Copydays regional admission, native
file/index/source integration and broad negative qualification remain pending.
Evidence: `research/dedup-projective-regions-state.json` and
`research/dedup-projective-regions-macos.log`.

Regional primitive regressions pass all30 tests across four groups on native
Linux with no default features and unchanged selected source/test hashes,
matching the all-feature macOS gate. This validates the primitive, not automatic
edited-region selection or whole-library qualification. Color-fit parameter
diagnostics on eight omissions retain exact original geometry, pixel counts and
acceptance. `200801`, `200901` and `201501` have no constrained channel in either
direction, so changing parameter bounds alone cannot change those least-squares
optima. This does not prove correct geometry or identify the full cause of their
rejection. Evidence:
`research/dedup-projective-regions-linux-source-tracking.json` and
`research/dedup-pyramid-fit-parameters-eight-omissions.json`.

### Regional selected-file lifecycle integration (2026-10-07)

`compare_local_files_projective_pyramid_regions` now verifies caller-declared
region pairs under one native pyramid model inside shared decoded views and the
existing source/dependency/cancellation lifecycle. Whole-image candidate evidence
is retained unchanged. Empty, excessive, overflowing or out-of-frame domains
refuse explicitly; rectangle validation is independent of geometry availability.
Cumulative admission counts the primary verification and every region's complete
verification cap before decoding. Per-region fitting refusals remain explicit;
work, memory, source or cancellation failures discard the whole result.

Returned region records use managed shared storage, with the reservation held
until the last owner drops. Evidence records contain only scalar/array fields and
are Copy, satisfying managed buffer admission without untracked nested heap
allocations. The native macOS gate passes primary geometry/acceptance preservation,
an informative region and a tiny-region fit refusal, shared-owner memory release,
first/middle/final cancellation, cap one-short refusal before allocation, region
count, checked overflow and domain validation. Native Linux is running. This
is not automatic region selection or real edited-copy recall/precision evidence.
Evidence: `research/dedup-pyramid-regions-files-state.json` and
`research/dedup-pyramid-regions-files-macos.log`.

The integrated registration/two-filter file API now completes all229 Copydays
strong pairs without errors: 30 accepted, 199 omitted. All reported constituent
geometry, pixel counts, fit refusals and decisions equal fixed independent
measurements, and all386 normalized images are verified. This resolves the
previous offline-union limitation for the file API; it does not complete copy
coverage or the independently running indexed collection measurement. Evidence:
`research/dedup-complementary-filter-portfolio-full-state.json`.

### Real edited-copy regional diagnostic (2026-10-07)

A diagnostic uniform4x4 source grid derives target rectangles solely from the
fixed native pyramid homography and image dimensions, without using origin
labels or choosing regions from pixel scores. Independent Copydays overlay
query `200102.jpg` confirms six of16 regions under the existing radius3,
1000-pixel/.3-coverage/.9-agreement/.03-residual gates and declared color-fit
bounds. Native geometry and whole-image rejection remain exactly equal to the
prior fixed radius3 report. Region storage is released after the final owner.
This proves finite local support for one edited copy, not whole-image equality,
a full-copy candidate or negative precision. All156 different-origin regional
comparisons are running; real corpus/general selection qualification remains.
Evidence: `research/dedup-copydays-region-grid-overlay.json`.
The selected-file regional lifecycle gate also passes native Linux (four pyramid
primitive/file/collection tests, selected tracked files unchanged):
`research/dedup-pyramid-regions-files-linux-source-tracking.json`.

The overlay query completes all156 different-origin regional comparisons without
errors: zero whole candidates and zero supported regions; ordered origin labels,
manifest/probe hashes, input hashes and release counters are verified. This
qualifies one query only. A bounded two-worker all229-query negative suite is
now running (35,724 different-origin pairs), preserving each query report and
explicit error/timeout/process status. Regional support remains separate from
whole-copy admission, and a single-query gate is not promoted to general
semantic/burst precision. Evidence:
`research/dedup-copydays-region-grid-overlay-state.json` and
`research/dedup-regional-all-query-negatives/summary.json`.

The integrated three-phase indexed collection probe now completes all229 strong
pairs without errors: 30 accepted, 199 omitted. All229 candidate decisions equal
the integrated direct file API; every retrieved pair's reported constituent
native evidence matches direct comparison (managed peak can differ). All386
normalized inputs are verified. This is the complete finite strong-subset index
measurement, not broad edited-copy, semantic/burst or scale qualification.
Evidence: `research/dedup-complementary-filter-collection-full-state.json`.

### Shared full-frame evidence for regional verification (2026-10-07)

Regional selected-file confirmation now reuses whole-frame unfitted evidence
from the same immutable views, model and policy. When whole color fitting
refuses without retaining that evidence, it is obtained once and shared across
regions. Individual regional fitting, domain/work validation and final source
and cancellation checks remain separate. Existing conservative cumulative
admission is unchanged. Only trusted internal reuse accepts the shared evidence;
public standalone regional verification still performs its complete admission
and whole-frame guards. Filter and source-count mismatches are refused internally.

All30 macOS primitive regressions pass. A direct two-region equivalence test
retains identical full regional evidence with fewer observed callback invocations,
including mismatched evidence/work refusals. The selected-file lifecycle test
passes after reuse, including explicit whole-fit-refusal fallback. Real overlay
`200102.jpg` retains exactly the previous geometry, every domain, pixel count,
fit refusal, six supported regions and ownership/release evidence. Single-run
time is recorded but no throughput or RSS improvement is claimed. Current Linux
regressions are running. The all-query negative suite continues on the original
immutable regional probe, rather than changing its executable mid-measurement.
Evidence: `research/dedup-regional-reuse-state.json` and
`research/dedup-regional-reuse-overlay-equivalence.json`.


### Spatial gradient regional collection checkpoint (2026-10-07)

Explicit spatial quotas now apply to both gradient recipes, managed pyramids,
source-guarded file comparisons and regional collection confirmation. In the
fixed 31-query low-correspondence subset, 124 native executions preserve the
legacy fields and produce two new interpolated models. Each retrieves and
confirms one local region in the atomic collection path; neither becomes a
whole-image candidate. Both queries complete all 156 different-origin controls
with independently audited zero whole or regional positives (312 comparisons).
Eight corrupted negative reports are rejected by the audit. Evidence:
`research/dedup-spatial-regional-collection-two-audit.json`,
`research/dedup-spatial-region-gain-negatives/state.json` and
`research/dedup-spatial-negative-audit-adversaries.json`.

Gradient retrieval source-ID and insufficient-ID scratch now reserve memory
before allocation; descriptor/owner credits reconcile actual capacity. A macOS
unit verifies an exact 32-byte limit for two full-width IDs, 31-byte refusal,
every observed cancellation checkpoint, last-owner release and empty input with
zero budget. The compound collection regression also passes with tracked
sources unchanged. This does not account
for all allocations or establish a process RSS ceiling. Evidence:
`research/dedup-gradient-metadata-exact-limit-state.json`.

The first immutable Linux snapshot exposed test-fixture permission propagation:
`fs::copy` copied read-only fixture permissions onto temporary mutation targets.
Tests now create writable temporary files from fixture bytes. A new independently
hashed immutable snapshot is under native Linux validation. Its sources predate
the latest ID-scratch helper extraction; neither failed earlier gates nor a
running retry qualifies the current complete library. Evidence:
`research/dedup-spatial-gradient-linux-retry-state.json`.
Combined all-family retrieval, current-source complete corpus qualification,
full collection scaling and the remaining acceptance rows are still required.

### Repeated gradient proposal allocation checkpoint 2026-10-07

The frozen six-source64MiB reproduction now identifies the exact refusal:
`gradient_file_pair_report` requested16 additional bytes while67108864 were
already reserved. The stack points to appending a file pair for every descriptor
hit, before final sorting/deduplication. Evidence:
`docs/research/dedup-six-refusal-trace.json`. Temporary tracing was removed from
working memory sources after freezing the diagnostic executable.

Gradient retrieval now keeps sorted unique file proposals as hits arrive while
preserving the independent complete descriptor-hit counter. Seven macOS index
tests pass, including32 identical descriptors across owners0/u64MAX:1024 hits,
one pair under2048bytes; measured exact/one-short memory admission, cancellation
and zero unique-pair cap. Evidence:
`docs/research/dedup-gradient-unique-proposals-regression.log`.
The fixed six-source collection, all15 direct-pair parity and new Linux snapshot
remain pending. No RSS reduction, throughput or complete resource accounting
is established by these finite tests.

The separate frozen five-family bidirectional229-pair run has completed with
independent grid/count/legacy parity checks:37 whole-image positives,151 whole
or local-region positives,78 omitted queries. Local support is not whole-image
identity. This predates sixth-family and proposal-allocation changes. Evidence:
`docs/research/dedup-bidirectional-collection-229-coverage.json` and
`docs/research/dedup-bidirectional-collection-229-terminal-recheck.json`.

The corrected frozen six-source collection now completes under the unchanged
64MiB managed limit:21590 indexed features,12666196 descriptor hits,15 unique
proposals,38189120-byte managed peak and zero retained bytes after return.
All15 returned pairs exactly match their independent fresh two-source whole and
four-regional-phase confirmations, including geometry/domains/pixel counts and
JSON types. Evidence: `research/dedup-six-unique-proposals-six-file.json` and
`research/dedup-six-unique-proposals-six-file-audit.json`.
The measured process RSS is71745536bytes, separately from managed reservations;
this does not establish a process-wide64MiB ceiling, throughput advantage,
386-source scaling or broad recall/precision. The expanded regional mutation
fixture and current Linux source qualification remain pending.

The current unique-gradient/incremental collection now passes the expanded
macOS compound gate in473.97seconds: all independent whole/four-regional fields,
late source mutation/affected-edge removal, restored reversed retry, cancellation
and cumulative cap refusals. Selected collection/index/fixture hashes remain
unchanged during this run. Evidence:
`research/dedup-six-unique-proposals-regional-lifecycle.log` and
`research/dedup-six-searches-state.json`. Current fixed-source Linux remains pending.

A separate JPEG orientation gate authors EXIF IFDs directly while retaining the
same compressed scan and ICC bytes. All eight orientations pass on each of two
rectangular64x8CMYK/YCCK profiled fixtures:16cases, exact dimensions and every
normalized pixel under independent coordinate mappings, with managed ownership
released. Evidence: `research/dedup-jpeg-profiled-orientation.log` and
`crates/rrrah-dedup/tests/jpeg_orientation.rs`. This is a relative orientation
oracle against native orientation1 pixels, not an independent JPEG color-decoder
oracle; broad orientation/container/ICC and Linux qualification remain pending.

The frozen configured-limit12-source collection completes with44373 indexed
features,58115321 descriptor hits and66 proposals under an explicit128MiB
managed cap. Its68538456-byte managed peak and zero retained bytes are distinct
from123305984-byte process RSS. All66 pairs now match independent fresh pair
confirmations in whole/four-region fields, including exact native geometry,
domains, counts and JSON types. Typed feature/hit/work/pair caps are checked.
Evidence: `research/dedup-six-twelve-file.json`,
`research/dedup-six-twelve-file-pair-oracle.json` and
`research/dedup-six-twelve-file-audit.json`. This remains a fixed12-source case,
not386-source scale, independent region-mask truth or a throughput comparison.

The immutable JPEG orientation snapshot also passes on native Linux: one
compound fixture covers16relative normalized orientation cases; all copied
source hashes recheck after termination. Evidence:
`research/dedup-jpeg-orientation-linux-state.json`. This preserves the same
relative orientation/color-oracle limitation as its macOS qualification.

The immutable current six-family/unique-gradient/incremental/regional-lifecycle
Linux snapshot completes60tests (51library,1collection,1admission,7index), with
allcopied source hashes rechecked after native termination. Evidence:
`research/dedup-six-unique-regional-lifecycle-linux-state.json`. The later expanded
all-callback index fixture is separately qualified on macOS only. A fresh current
frozen binary is now running the full229pair control and312two-query negative
gates; older frozen corpus results remain source-version evidence, not substitutes.

Four additional older five-family regional gains complete624 different-origin
comparisons with zero whole/local-region positives, preserving all report/image/
audit hashes. Evidence: `research/dedup-bidirectional-four-gain-negative-terminal-audit.json`.
This is finite selected-query precision, notsemantic/burst orall-query coverage.


The fixed 24-source six-family collection now completes and matches all 276
independent pair confirmations exactly. Of these, 66 observations reuse a
pinned 12-source prefix and 210 are fresh. Publisher labels identify 12 copy
pairs: eight pass whole-image confirmation, nine pass whole or local support,
and three remain omitted. All 264 different-origin pairs reject. Evidence:
`research/dedup-six-twentyfour-file-audit.json` and
`research/dedup-six-twentyfour-pair-label-audit.json`.

The frozen configured-limit executable peaks at 137,387,144 managed bytes with
zero retained credit after completion, under a declared 256 MiB cap. Process
RSS peaks at 215,220,224 bytes separately; wall time is 2,986.82 seconds and CPU
user/system time is 1,536.46/12.40 seconds under concurrent load. These establish
this fixed 24-source measurement, not full 386-source scaling or useful general
latency. Explicit intermediate-scale spatial extraction also leaves the three
hard crop copies omitted; it is not promoted as a coverage solution.

Same-source build-profile measurement completes 72 alternating native calls
(three fixed pairs, direct and two-source collection, one warmup and five measured
repetitions per profile). All exact JSON outputs, including managed peaks, agree;
all 2798 immutable snapshot source files and executable/image pins reverify.
Release median child CPU ratios versus debug range from 1.49 to 2.26 on these
cases under concurrent load, not full-collection throughput. Evidence:
`research/dedup-scale-profile-measurements.json`. The global intermediate-scale
229-pair corpus completes with 34 whole candidates and no new finds beyond the
prior six-search whole/local union; its independent typed/pixel/domain audit
passes. One spatial-scale crop registration diagnostic still yields no whole
or local support. These omissions remain requirements, not qualified coverage.

The intermediate-scale, spatial-scale, distinct-location ratio and domain-qualified
geometry native gate passes all 49 selected tests on both macOS and ARM64 Linux
from the same immutable 2804-file snapshot. Snapshot hashes reverify after both
runs. File tests include cancellation, appended input mutation, edge removal,
restoration/reverse retry and exact regional pair parity. Evidence:
`research/dedup-scales-distinct-cross-platform-state.json`. These finite fixtures
do not qualify Windows, real hard-crop recall or the remaining matrix rows.

## Managed distinct-location source-resolution collection checkpoint

The explicit managed distinct-location gradient-scale regional collection API
now preserves the known Copydays `200200.jpg` / `200201.jpg` crop at original
source resolution. Its217 correspondences,125 projective inliers, every regional
domain/pixel count/photometric coefficient, one accepted local region and false
whole-image identity decision exactly equal the separately measured managed
file API. The retained payload is240744 bytes; measured managed peak is330712464
bytes and retained credits return to zero after dropping the result. This is
managed accounting, not a decoder/geometry complete-allocation or RSS bound.
Independent raw-output, source-pin, residual and region-acceptance arithmetic
checks pass in `research/dedup-original-managed-collection-200201-audit.json`.
Ten deliberately corrupted audit records are rejected; that parser gate alone
is not native collection evidence.

All14 authored macOS gradient-scale file/collection tests pass, including
fresh all-pair parity, three-copy cancellation, removal of both changed-source
edges while preserving the unchanged edge, restored-source retry and
pre-iteration cumulative admission. See
`research/dedup-distinct-managed-region-collection-state.json`.
The fixed current Linux snapshot passes all20 managed-result, distinct-matching
and scale-file/collection tests; all2810 source files rehash after completion.
See `research/dedup-managed-regions-collection-linux-state.json`. The original-resolution
one-query negative diagnostic has16 independently audited different-origin
comparisons out of156 required; remaining comparisons are running. This finite
positive and prefix do not establish general real-crop recall, semantic/burst
precision, full386-source collection performance, Windows behavior or all rows.

The two remaining original-resolution crops are now independently measured
through the same frozen managed collection: `200101.jpg` has26 matches and16
inliers but zero accepted regions; `200301.jpg` has five matches and no model.
Every geometric/regional field equals the preceding diagnostic and source pins
verify. See `research/dedup-original-managed-remaining-crops-audit.json`.
A radius7 photometric diagnostic preserves the first crop's geometry and region
domains but raises its best bidirectional matched fraction only from0.644668 to
0.701815, still below0.9; it recovers no region. This is not promoted as a default
or counted as recovered recall. See
`research/dedup-original-blur7-200101-audit.json`.

An explicit area-resampled gradient candidate recipe passes its independent
integer/fractional footprint oracle and all17 authored scale-file/collection
regressions on macOS. The source-guarded managed whole/regional file API now
measures all three original-resolution hard crops: `200101.jpg` grows from26/16
to77/33 matches/inliers but still has no accepted region; `200301.jpg` grows
from five to seven matches without a model; `200201.jpg` retains one accepted
region with249/127 matches/inliers. Full producer pins, typed output, residuals
and region-acceptance arithmetic verify in
`research/dedup-original-area-regions-audit.json`. One of three remains recovered;
this is no demonstrated recall improvement or default promotion. The original
point-sampling one-query negative diagnostic has32 audited pairs out of156;
those negatives do not qualify the area recipe. The fixed area snapshot now passes all58 native ARM64 Linux gradient/scales/
file/collection/domain/projective tests; all2813 source/fixture files rehash
after completion. Evidence: `research/dedup-area-regional-linux-state.json`.
This finite gate does not qualify the8x8 real diagnostic or Windows.

The explicit area/radius7 diagnostic on8x8 regions recovers the previously
omitted original-resolution `200101.jpg`: seven local regions pass unchanged
minimum samples/coverage and0.9 matched-fraction thresholds; the best
bidirectional fraction is0.999469. Correspondences and geometry remain77/33,
whole-image acceptance stays false and managed retained credits return to zero.
The independent audit also checks both domain bounds and rejects10 corrupted
records. Evidence: `research/dedup-original-area-grid8-blur7-200101-audit.json`.
Two different-origin controls for that same frozen recipe are running; this
positive is not promoted to a default or broad precision/recall qualification.

The area/grid8/radius7 recovered first crop now also passes the real two-source
collection gate:77 correspondences,33 inliers, all regional domains/counts/
coefficients and seven supports equal fresh independent managed file evidence.
Whole-image acceptance stays false;258888 retained bytes release completely
on last drop and managed peak is330712464 bytes. Native data and provenance
verify in `research/dedup-original-area-grid8-collection-200101-audit.json`.
All19 authored collection/file lifecycle tests pass on macOS; the corresponding
current Linux snapshot gate is still running. The same frozen recipe's full156
negative gate has17 independently audited comparisons without candidates or
refusals; this prefix does not prove the remaining139 comparisons or broad
precision and is not a full386-source performance qualification.

Area collection Linux checkpoint: the fixed 2816-file snapshot passed all 25 selected native tests (4 distinct-location, 19 scale-file/collection, 2 managed-evidence). Every snapshot file was rehashed after completion. Evidence: `research/dedup-area-collection-linux-state.json`. This qualifies the finite lifecycle/parity cases, not full collection recall or complete allocation/RSS bounds.

Mixed-format indexed pixel search now passes the independent PNG/TIFF/BMP/PPM/TGA/lossless-WebP/QOI fixture collection: all 21 equal pairs are retrieved in both input orders, all seven one-pixel negative pairs are excluded, and managed reservations return to zero. All six raster-equality tests pass on macOS. Evidence: `research/dedup-cross-encoding-indexed-state.json`. This finite collection is not a broad cross-format or native Linux qualification.

Mixed-format alpha indexed search passes 15 independently encoded PNG/TIFF/TGA/lossless-WebP/QOI fixtures: base and hidden-RGB variants form one ten-file group, visible one-pixel edits form a separate five-file group. Exactly 55 equal pairs are retrieved and all 50 cross-group pairs are excluded in both request orders, with managed reservations released. Seven raster-equality tests pass on macOS. Evidence: `research/dedup-alpha-cross-encoding-indexed-state.json`. Broad HDR/ICC and Linux qualification remain open.

Native Linux now passes all seven raster-equality tests on the immutable 2818-file snapshot, including opaque seven-format and alpha five-format indexed collections in both orders. All 2818 file hashes were verified after the successful exit. Evidence: `research/dedup-alpha-indexed-linux-state.json`. This qualifies the same finite fixtures on macOS and Linux; broader cross-format/HDR/ICC corpus remains open.

The explicit low-contrast detector experiment on 200301.jpg changes minimum corner score from 0.0001 to 0.000001 while retaining source-resolution area/grid8/radius7 and all final acceptance thresholds. Native output increases correspondences from seven to ten but still has no projective model, zero accepted regions and no whole candidate. Independent pins/raw-output/model/resource audit passes, and ten corrupted report controls refuse. Evidence: `research/dedup-original-lowcontrast-200301-audit.json`. This remains a miss and is not promoted as a default.

Symmetric candidate-only 5x5 box smoothing was measured on 200301.jpg with unchanged detector/matcher/geometric acceptance. It extracts 859/1707 features and eight correspondences but no model or accepted local region. Final supplied-model regional confirmation remains tied to original pixels. Independent native-output/resource/model audit passes: `research/dedup-original-smooth-candidates-200301-audit.json`. The miss remains; this diagnostic does not provide a whole decision or managed matcher/geometry allocation bound and is not promoted.

The deterministic union of the independently measured low-contrast and smoothed-candidate recipes now recovers the known 200301.jpg crop locally: 17 distinct-location correspondences produce 10 projective inliers and 47 accepted regions under the unchanged source-pixel grid8/radius7 criteria. The union is independently reconstructed from pinned native reports; raw native parity, geometric residuals and regional arithmetic pass. Evidence: `research/dedup-original-correspondence-union-200301-audit.json`. This is a diagnostic known-positive recovery, not a whole decision, generalized integrated file/collection API, negative precision or complete allocation bound. These remain required before promotion.

Candidate-only smoothing now has a public managed primitive with checked tap/memory admission, opaque-source refusal and cancellation without retained partial output. Two native macOS integration tests pass independent full-footprint oracles at radii0/1/2/4, HDR/borders, all callback cancellations and last-owner credit. The example uses the primitive; its oracle passes, and all native output fields for the real 200301.jpg smoothed-candidate diagnostic exactly match the earlier prototype. Evidence: `research/dedup-gradient-candidate-smoothing-state.json`. Generalized union file/collection integration and precision remain required.

Native managed correspondence union now reproduces the diagnostic recovery from all 18 raw recipe correspondences: 17 distinct-location points, 10 inliers and 47 accepted original-pixel regions. Exact typed correspondence/model/regional parity is independently audited in `research/dedup-managed-correspondence-union-200301-audit.json`. This qualifies managed union storage and known-positive parity, not fresh combined extraction, collection, negative precision or complete geometry/decoder allocations.

The fresh managed candidate-union file API now independently extracts both recipes from the original JPEGs and reproduces all correspondence/model/regional fields of the known 200301.jpg recovery: 17 correspondences, 10 inliers, 47 accepted regions. Actual retained memory is positive before dropping evidence and zero afterward. The authored source-change/cancellation/admission gate passes two tests. Evidence: `research/dedup-fresh-candidate-union-200301-audit.json`. This is local known-positive file evidence; collection retrieval, same-recipe negative precision, Linux and full decoder/geometry allocation qualification remain pending.

The fixed 2822-file native Linux gate passes six managed candidate smoothing/union/file tests, with every snapshot file rehashed afterward. Two other publisher-origin sources through the same frozen fresh union recipe reject with zero accepted local regions; typed native/model/location/region audit passes. Evidence: `research/dedup-candidate-union-linux-state.json` and `research/dedup-fresh-union-negative-controls-audit.json`. These finite gates predate the new collection integration and do not establish broad or semantic precision.

The new two-recipe managed collection entrypoint now passes four authored file/collection tests and all19 existing scale file/collection regressions on macOS (23 total). Every fresh accepted pair is retrieved, retrieved evidence equals separate file confirmation, changed-source incident edges are removed while the healthy edge survives, reversed restored retry and cancellation/admission pass. Evidence: `research/dedup-candidate-union-collection-state.json`. The real known crop collection measurement and native Linux collection qualification are running/pending; this finite gate does not establish broad recall/precision or allocation completeness.

The source-resolution six-file area/grid8/radius7 collection now completes all15 fresh pair comparisons with exact correspondence/model/region/whole-decision parity. All12 different publisher-origin pairs have zero accepted local regions and false whole decisions. The first and second crop pairs support7 and15 regions; the third supports zero and remains a miss for this single recipe. All15 pairs were proposed; 51,350 features and187,681,598 descriptor hits were indexed, with451,245,072 managed peak bytes and zero retained usage after release. Evidence: `research/dedup-original-area-six-collection-terminal-audit.json`. This finite old-recipe collection gate does not qualify the newer compound candidate-union collection or full-corpus recall.

The fresh compound candidate-union two-source collection now recovers the real original-resolution200301.jpg crop with17 correspondences,10 inliers and47 accepted local regions. Its complete canonical evidence, including retained credit, equals separate fresh file confirmation; managed peak is378,759,952 bytes and usage returns to zero after release. Evidence: `research/dedup-fresh-candidate-union-collection-200301-audit.json`. This is a known positive with two sources, not whole-image identity, a six-source compound collection result, full-corpus recall or broad precision. Native Linux27-test qualification and the frozen recipe156-negative gate remain running.

The fixed2823-file candidate-union collection snapshot completes native Linux qualification: four compound file/collection tests, two managed correspondence-union tests, two candidate-smoothing tests and19 prior gradient-scale file/collection regressions (27 total). All snapshot source hashes recheck after terminal exit0. Evidence: `research/dedup-candidate-union-collection-linux-state.json`. This snapshot predates the later six-source example and does not establish full corpus collection, semantic precision or complete decoder/geometry allocation bounds.

The frozen fresh compound local file recipe now starts all229 original-resolution strong pairs, preserving explicit native errors in the denominator. An independent strict prefix audit verifies the first two:200001.jpg has288 correspondences/102 inliers/80 supported regions;200101.jpg has122/50/8. No whole-image identity or all229 recall follows from these two positives. Same-recipe200301.jpg negative audit verifies20 of156 distinct publisher-origin originals with zero local support. Evidence: `research/dedup-fresh-union-original-full-strict-checkpoint-2-audit.json` and `research/dedup-fresh-union-original-negatives-200301-strict-checkpoint-20-audit.json`. All386 original JPEG hashes and independent EXIF IFD0 domain metadata check, including18 swapped dimension cases; source-space bounds use the oriented dimensions. Terminal reports require every row even when requested with checkpoint mode. Full229, full156, real six-source compound collection and release parity remain running/pending.

The frozen source-resolution area/grid8/radius7 one-query gate completes all156 different publisher-origin originals with zero whole candidates, zero local accepted regions and no native errors. Independent terminal audit verifies all157 original source hashes, exact positive-control parity, typed native/model/region resource evidence and ordering. Evidence: `research/dedup-area-grid8-original-negatives-200101-terminal-audit.json`. This older single recipe does not qualify compound-union precision or semantic/burst discrimination.

The fresh compound recipe completes four native release comparisons with exact debug evidence parity: two real positives (200301:47 regions;200001:80), and two different-origin negatives with zero accepted regions. Retained credit and all canonical correspondence/model/regional fields match, managed usage returns to zero, peak stays within512MiB. Evidence: `research/dedup-fresh-union-release-parity-terminal-audit.json`. Finite release parity does not establish all229 release recall or controlled speed improvement.

The fresh compound original-resolution prefix independently verifies seven of229 pairs: six have local pixel support, while photographed wrinkled copy200302.jpg remains a miss (11 correspondences, no projective model). Increasing candidate-only smoothing radius from2 to4 also misses that same pinned pair: seven correspondences, zero inliers and zero accepted regions. This failed diagnostic is not promoted and no geometry or pixel threshold is relaxed. Evidence: `research/dedup-fresh-union-original-full-strict-checkpoint-7-audit.json` and `research/dedup-wrinkled-smooth4-audit.json`. Same-recipe one-query negative prefix verifies44 of156 different-origin originals with zero accepted local regions: `research/dedup-fresh-union-original-negatives-200301-strict-checkpoint-44-audit.json`. Full runs remain active; nonrigid-copy coverage is unresolved.

A seven-level quarter-octave candidate diagnostic (scales1..2.828, unchanged radius2 smoothing and acceptance) also fails the pinned wrinkled200302.jpg pair: six distinct correspondences, zero model/inliers/regions. Independent native/source/resource audit: `research/dedup-wrinkled-fine-scales-audit.json`. This changes both spacing and upper scale extent relative to seven half-octave levels1..8; it does not isolate scale spacing as a cause. No default promotion or acceptance relaxation follows.

Early release of original decoded candidate images, after both smoothed buffers exist and before descriptor extraction, passes all23 macOS compound/scale file-collection tests. Independently audited200301.jpg fresh repeat preserves all canonical evidence (17 correspondences,10 inliers,47 regions and retained credit) while managed peak decreases from354,962,656 to300,766,880 bytes. Evidence: `research/dedup-early-decode-release-checkpoint-1-audit.json`. This is managed reservation accounting, not process RSS; recovery of the two budget-refusal files and Linux qualification remain running.

Explicit EncodedSrgb regional filtering does not recover the pinned screen-copy200501.jpg: all240 correspondences and128 inliers exactly match the LinearSrgb baseline, but zero regions satisfy unchanged original-pixel criteria. Independent audit: `research/dedup-screen-copy-encoded-audit.json`. Color-space selection alone is insufficient for this case; no default promotion or threshold relaxation follows.

The early candidate decode-release change also passes native Linux27 tests (4 compound file/collection,2 smoothing,2 correspondence union,19 scale file/collection); all2826 fixed snapshot files rehash after terminal exit0. Evidence: `research/dedup-early-decode-release-linux-state.json`. This qualifies lifecycle/parity tests for the library change, not successful completion of the two real resource-refusal cases, later12.8M domain-policy example or full corpus/RSS.

The early decode-release repeat terminates with independently audited recovery of200801.jpg, formerly Features(Budget):478 correspondences,114 inliers and2 original-pixel supported regions within426,586,064 managed peak bytes (same512MiB budget/criteria). The control200301.jpg remains exactly17/10/47.200701.jpg instead advances to Pixels(Budget); the explicit combined-domain repeat remains running. Evidence: `research/dedup-early-decode-release-terminal-audit.json`. Results are separate from the frozen full229 baseline; no mixed-version recall claim.

The explicit12.8M combined-domain admission repeat independently recovers200701.jpg after early original decode release:2029 correspondences,1123 inliers,96 original-pixel supported regions within429,618,400 managed peak bytes. The control remains exact17/10/47. Evidence: `research/dedup-combined-domain-budget-checkpoint-2-audit.json`. The8M policy could not admit this8,773,392-pixel pair;12.8M matches two allowed6.4M inputs. Geometric/pixel thresholds and512MiB memory budget remain unchanged. All229 dimension arithmetic identifies22 pairs over8M and none over12.8M (`research/dedup-original-domain-admission.json`), but does not establish execution, recall or full-memory bounds. Third parity repeat remains active; full frozen corpus is a different version.

The updated early-decode-release and12.8M domain recipe completes all five native release controls with independently audited exact debug parity:200301 has47 supported regions, recovered200701 has96, recovered200801 has2, and two different-origin controls have zero regions. Evidence: `research/dedup-combined-domain-release-terminal-audit.json`. The strict full229 release auditor rejects10 reconstructed corrupt reports and keeps native errors separate from misses (`research/dedup-combined-domain-release-full-auditor-controls.json`); these are parser controls, not recall. A new frozen full229 source-resolution release run is active separately from the older baseline.

The updated full release run independently verifies14 of229 pairs:12 have local support, two miss, and none have native errors. The two misses are wrinkled200302 and screen-copy200501; this prefix includes recovered200701 and200801. Evidence: `research/dedup-combined-domain-release-original-full-checkpoint-14-exhaustive-diagnostic-audit.json`. A separate updated-version repeat recovers201401 (6275 correspondences,6007 inliers,62 regions), previously Features(Budget), while201303 still misses (42 correspondences, no model). Evidence: `research/dedup-updated-additional-failures-terminal-audit.json`. Separate diagnostic rows are not added to the full-run recall denominator.

Visual inspection identifies201303 as a blurred enlarged crop of a sea rock. Every111930 four-point subset of its42 proposals is evaluated by the native exhaustive solver at tolerance2/minimum10; no qualifying model is returned, even without whole-domain eligibility filtering. Evidence: `research/dedup-201303-geometry-exhaustive.json`. This is exhaustive over this solver's four-point hypotheses, not a mathematical impossibility proof for all homographies. Radius4 candidate smoothing also misses (48 proposals, no geometry): `research/dedup-201303-smooth4-audit.json`. Descriptor quality and blur/crop coverage remain unresolved.

Screen-copy200501 remains a miss with linear-color radius8 confirmation:240 correspondences,128 inliers,zero supported regions. Its best bidirectional matched fraction increases from.61613 to.73229, below unchanged.9 acceptance. Evidence: `research/dedup-screen-filter8-audit.json` and `research/dedup-screen-filter-radius-comparison.json`. Radius8 is diagnostic only. Analytic periodic-band controls pass30 positive variants and30 broad-edit negatives on macOS and Linux, using supplied identity geometry and a fixed radius7 filter. Each positive direction compares2601 pixels with full match and.61562 coverage. Linux snapshot2828 files all rehash. Evidence: `research/dedup-periodic-capture-production-counts-controls.json` and `research/dedup-periodic-capture-linux-state.json`. These finite pixel-stage controls do not establish unknown camera-artifact or recovered-geometry coverage.

The older six-source compound collection terminates101 after reporting Features(Budget) on pairs(1,3),(1,5),(3,5). It does not complete15-pair fresh parity. Evidence: `research/dedup-fresh-union-six-collection-retry.json`. An updated release collection repeat and a separate156-negative updated release gate are active; neither is yet qualified by a terminal audit. Older negative prefixes (135/156 and144/156) stay version-specific. Full collection coverage, real screen-copy/nonrigid/blurred-crop recovery, remaining acceptance-matrix rows and complete memory bounds remain open.

An explicit per-input candidate smoothing entrypoint now recovers blurred crop201303 using source radius4 and target radius0:54 correspondences,11 inliers,46 original-pixel supported regions. A fresh positive prerequisite repeats the same canonical evidence. Evidence: `research/dedup-201303-asymmetric4-audit.json` and `research/dedup-asymmetric4-original-negatives-201303-checkpoint-0-initial-audit.json`. The symmetric entrypoint delegates with equal radii; asymmetric original-pixel direct parity, unrelated rejection, cancellation and work-refusal release pass on macOS. Native Linux28 tests complete (5compound,2union,2smoothing,19scale), with2828 snapshot file hashes verified after exit0: `research/dedup-asymmetric-smoothing-linux-state.json`. Wrinkled200302 remains a miss under4/0:7 correspondences,no model or supported regions (`research/dedup-wrinkled-asymmetric4-audit.json`). A separate229-pair asymmetric corpus run and156-negative query gate are active; four deliberate corrupt full reports are rejected by its auditor. One recovered pair is not substituted into symmetric-run recall, and no default or collection promotion follows yet.

The frozen older compound recipe's one-query156 different-origin negative gate now completes with independent terminal source/archive/control/model/region audit and zero accepted regions or native errors: `research/dedup-fresh-union-original-negatives-200301-terminal-audit.json`. This result qualifies that frozen recipe on this finite query only; updated release and asymmetric variants retain their separate negative gates.


### Asymmetric smoothing: completed Linux lifecycle extension

The frozen late-lifecycle Linux snapshot completes all 29 native tests
(6 candidate-union file/collection, 2 correspondence union, 2 smoothing,
19 scale file/collection), with zero failures and zero ignored tests.
All 2,828 snapshot files rehash after terminal return code 0; the library
implementation and six-test candidate-union source match the current checkout.
Evidence: `research/dedup-asymmetric-late-linux-state.json` and its pinned log
and snapshot manifest. Deferred cancellation, source replacement and restored
reverse-order retry are now exercised on both macOS and Linux. These finite
fixtures do not establish corpus recall, all-query precision or complete RSS bounds.

The actual blurred crop 201303 also passes the separate reverse-order native
experiment with candidate radii [0,4]: 54 correspondences, 11 geometric inliers
and 46 original-pixel supported regions. Evidence:
`research/dedup-201303-asymmetric4-reverse-audit.json`.
The current collection entrypoint still extracts the symmetric configured
smoothing radius and confirms with the symmetric pair API. Therefore the new
asymmetric direct-pair recovery is not yet an indexed-collection coverage claim.


### Symmetric-first fallback and indexed proposal integration

The opt-in `compare_local_files_projective_candidate_union_fallback_regions_managed`
keeps an accepted symmetric result and only tries [4,0], then [0,4], on misses.
Three-attempt gradient/matching/union work is admitted before file access;
resource refusals stop rather than becoming misses. Common content snapshots and
sticky cancellation span attempts; missed evidence is dropped before retry.
The frozen real two-case repeat is independently verified:
200301 keeps its exact canonical 47-region symmetric evidence after one attempt;
201303 recovers its exact canonical 46-region asymmetric evidence after two.
Evidence: `research/dedup-fallback-known-pairs-retry-audit.json`.
The initial run was stopped by a source-pin change and remains an incomplete
report, not a second-pair result. The repeat pins an immutable source snapshot.

`scan_projective_local_collection_candidate_union_fallback_regions_managed`
now indexes the low-contrast original, symmetric smoothed, asymmetric smoothed
and zero-radius default-corner descriptor domains, then freshly confirms using
the fallback API. Cross-domain hits only propose pairs. The three-source macOS
fixture retrieves every direct positive with exact fallback evidence and rejects
unrelated pixels; cumulative admission refuses before iterator access.
Evidence: `research/dedup-candidate-union-fallback-collection-macos-state.json`.
Its source-mutation/cancellation test, 32-test Linux integration snapshot and
real indexed two-case experiment are still running. The earlier completed
30-test Linux snapshot qualifies direct fallback and predates this collection
integration. Full-corpus fallback recall and negative precision remain unproven.


### Updated symmetric union: completed six-source retrieval parity

The frozen updated release compound symmetric union completes six-source
collection and all 15 fresh pair comparisons. Independent terminal audit confirms
exact geometry/pixel evidence parity for every retrieved pair, all three known
origin-copy pairs supported (8,13,47 regions), and all twelve different-origin
pairs rejected. Index counters:101,081features,574,190,937descriptor hits,15pairs;
managed peak467,231,904bytes and terminal used0. Native collection and pair
refusals are absent. This closes the earlier frozen version's three Features(Budget)
collection refusals on this finite set, without replacing that historical report.
Evidence: `research/dedup-updated-release-six-collection-terminal-audit.json`.
The recipe uses the original8M combined pixel domain, unlike explicit12.8M full
and fallback diagnostics. It is not the new four-domain fallback collection or
full229-source collection proof.


### Screen-copy bounded translation diagnostic

All nine target translations in[-1,0,1]pixels per axis complete with zero
accepted original-pixel regions. The zero-offset regional output exactly matches
the prior linear radius8 experiment. Best bidirectional matched fraction improves
from0.7322916667 atzerooffset to0.7639583333 at[-1,0], below unchanged0.9acceptance.
Evidence: `research/dedup-screen-translation-grid-retry-audit.json`.
These are supplied-model pixel diagnostics, not refit or full-domain geometric
qualification, and do not rule out larger/local/nonprojective alignment or
photometric capture correction. No threshold/default change is promoted.
The initial4argument dispatch refusal is retained separately.


### Fallback index: completed real complementary two-case retrieval

The frozen four-domain indexed fallback diagnostic finishes both real positive
pairs and passes independent terminal verification.200301 retains exact47-region
symmetric evidence after one attempt;201303 is retrieved and recovers exact46-region
asymmetric evidence after two attempts. Canonical geometry, original-pixel fields,
retained evidence and ordered recipe metadata match the independently qualified
direct comparisons. Evidence:
`research/dedup-fallback-collection-known-pairs-terminal-audit.json`.
Together with32native Linux tests and macOS lifecycle/parity controls, this
qualifies finite indexed recovery, not full229retrieval or broad negative precision.

The screen-copy diagnostic now separates local geometric support from original-
pixel evidence. Four unconstrained local homographies fitted only to the 128
global inliers pass residual checks but accept no original-pixel regions. Nine
fitting-domain transfer samples differ from the global model by up to 12.24,
4.41, 1.27 and 9.73 target pixels respectively; this finite comparison does not
establish the true geometry or the cause of the pixel misses.

A native least-squares local-translation diagnostic retains the global
projective shape and denominator. Independent recomputation verifies all four
region memberships, translation means, composed matrices and tolerance-2
inliers (13/14/62/35). Its composition test includes a non-affine denominator;
four deliberately altered mean/composition/membership/inlier records are
rejected by the auditor. Original-pixel qualification is still running, and no
production default or acceptance threshold changes. Evidence:
`research/dedup-screen-regional-translations-audit.json`,
`research/dedup-screen-regional-translations-auditor-controls.json`,
`research/dedup-screen-inlier-regional-model-pixels-terminal-audit.json`.

The source-resolution asymmetric original-only radius4 candidate diagnostic
now completes all156 different publisher-origin comparisons for the recovered
201303 query, without accepted regions or native errors. Its known positive
retains46 original-pixel supported regions, and the independent terminal audit
verifies all labels, source/archive pins, geometry, regional arithmetic and
managed release evidence. This qualifies one query under that explicit recipe;
it does not qualify the separate symmetric-first fallback's negative gate,
all-query precision, semantic similarity or burst discrimination. Evidence:
`research/dedup-asymmetric4-original-negatives-201303-terminal-audit.json`.

The photographed wrinkled mountain print remains unresolved. A dense native
stride32/16 grid at scales1/2/4 generates30 ratio-qualified proposals, reduced
deterministically to21 distinct source-and-target pairs. Native all-image,2x2
and4x4 partitions produce no minimum10/tolerance2 projective models; independent
reconstruction verifies pair admission, all partitions and typed outcomes.
This finite experiment does not establish geometric nonexistence or pixel
equality. The initial scaled-boundary and duplicate-target refusals are retained
separately, and none of these diagnostic examples alters production acceptance.
Evidence: `research/dedup-wrinkled-dense-distinct-partitions-audit.json`.

Original-resolution radius8 pixel-based projective refinement now completes
for the screen-copy baseline and first independent SIFT model. The initial
8M work gate returns two explicit Budget refusals; its conservative per-model
full-work bound is178563600 sampled/filter pairs. The separate180M repeat
completes and retains110/240 and48/91 original candidate residuals within2px.
Independent model auditing normalizes arbitrary homography scalar magnitude;
two equivalent scale controls retain identical evidence and four malformed
initial/model/memory/stdout controls reject. Neither refined model accepts an
original-pixel region on grid4x4; smaller grid8x8 confirmation is running with
the same residual/coverage/count/fraction thresholds. Evidence:
`research/dedup-screen-original-refinement-work-audit.json`,
`research/dedup-screen-refinement-auditor-controls.json`,
`research/dedup-screen-refined-models-pixels-terminal-audit.json`.

For the separate folded-print query201702, the verified289distinct proposal
set produces4 local models on2x2 and8 on4x4. Independent partition and residual
auditing confirms minimum10/tolerance2 geometry. All eight4x4 models are now
under original-pixel verification restricted to their fitting target domains;
local geometric agreement is not yet a recovered-copy claim. Evidence:
`research/dedup-folded-201702-regional-geometry-audit.json`.


A separate native screen-ROI diagnostic fits bounded full RGB affine color on
8x8 checker training samples and evaluates the other checker cells. Forward
radius8 and reverse radius14 are selected from the baseline projective Jacobian
at the source ROI center, before pixel evaluation. Both fixed known-ROI heldout
fractions exceed0.9 (2383/2400 and6191/6524), with center clipping to the opposite
ROI and whole-image filter support. This does not establish a recovered copy in
the integrated region/file API: the ROI is supplied, sample storage is outside
managed decode credit, and negative/burst/low-information qualification remains
incomplete.

All156 other original images were tested against the screen query under the
same normalized ROI geometry. Zero pairs passed both directions, with zero
native execution errors. Thirteen reverse-only fractions exceed0.9; a single
direction therefore cannot support acceptance. The terminal auditor independently
checks membership, source/model pins and typed decisions. Six intentionally
corrupted report controls (false acceptance, sample count, ROI, source digest,
missing case, duplicate case) reject; the unmodified report passes. These are
finite fixed-geometry controls, not all-query retrieval or semantic precision.
Evidence: `research/dedup-native-screen-affine-color-footprint-audit.json`,
`research/dedup-native-screen-affine-color-negative-gate-terminal-audit.json`,
`research/dedup-native-screen-affine-color-negative-auditor-controls.json`.

The affine-color path now has explicit library APIs for known-region views,
source-guarded bidirectional selected files, automatic target-grid views/files,
and fresh native candidate geometry followed by automatic color regions. Outer
source/dependency snapshots cover both search and confirmation; cancellation is
sticky across callback and request generation tokens. Sample vector payloads and
retained grid evidence use the shared memory budget; allocator overhead, decoder
allocation completeness and whole-process RSS remain separate. These APIs expose
local evidence and fitting refusals, without promoting exact identity or a default
copy decision. Authored file tests reject content append and equal-content inode
replacement, discard cancelled evidence and release memory; the integrated native
geometry fixture exactly matches separate guarded grid confirmation.

The automatic8x8 screen grid under supplied baseline geometry produces64 regions
and two bidirectional fraction supports. An independent auditor reconstructs all
domains, radius choices and successful sample counts. A saturated-white fixture
then reproduced a floating-point accumulation error (Invalid rather than an
uninformative fit refusal). Summing taps before one division fixes this without
clipping or changing thresholds. A fresh frozen repeat retains both supports and
passes independent auditing. The initial negative-grid run is preserved and
explicitly stopped for this reproduced error; its errors are not negative proof.
The corrected156-original negative grid and native fresh-geometry screen search
are still running. Evidence: `research/dedup-native-screen-affine-color-grid-meanfix-audit.json`,
`research/dedup-native-screen-affine-color-grid-negatives-stopped.json`,
`research/dedup-native-screen-affine-color-grid-negatives-meanfix-checkpoint-audit.json`.

The native affine-region Linux qualification now terminates successfully: all
2858 files of its immutable source snapshot rehash and seven focused suites pass
16 tests. Later input-contract tests are outside that Linux snapshot. Evidence:
`research/dedup-affine-region-linux-state.json`.

The observed unrelated-original color-grid support on `201900.jpg` is rejected
by fresh native candidate geometry: 11 points, zero inliers, no model and no
color regions. This qualifies that counterexample only; color agreement alone
still must not establish a copy. Evidence:
`research/dedup-affine-grid-201900-native-search-audit.json`.

On an immutable76-pair original-resolution slice, spatial distinct-point and
exhaustive all-pairs auditors agree on every pair summary and decision:
68 supports, eight misses and zero native errors. Auditor-specific source pins
differ by design; common input pins agree. This is partial corpus evidence,
not229-pair recall or complete coverage. Evidence:
`research/dedup-fallback-76-spatial-exhaustive-equivalence.json`.

Current source/target footprint selection, guarded file lifecycle and inlier-domain
bounding now pass25 tests across eight focused suites on native Linux aarch64.
All bytes of the isolated source snapshot rehash; current five modules and eight
test files match it. Added measurement examples are outside that Cargo snapshot.
Evidence: `research/dedup-common-footprint-linux-state.json`.

The folded-print control remains unresolved: six8x8 native local models retain
strict geometry and yield zero pixel supports under target-axis, common-axis
and inlier-bounded ROI sampling. Raw color-fit diagnostics explain three bounds
refusals through offsets above0.1 but still yield insufficient heldout fractions.
No color/pixel threshold was promoted. Evidence:
`research/dedup-folded-grid8-common-footprint-regions-audit.json`,
`research/dedup-folded-witness-common-footprint-regions-audit.json`,
`research/dedup-folded-grid8-color-bounds-flags-audit.json`.

A93-case supplied-geometry negative prefix has four color-only region supports
and zero native errors. Fresh native geometry rejects observed counterexample
`207300.jpg` with15 points, zero inliers and no model; this is one finite control
at the preserved earlier binary revision. Broad or current-version precision
is not inferred. Evidence: `research/dedup-affine-grid-meanfix-checkpoint-93-audit.json`
and `research/dedup-affine-grid-207300-native-search-audit.json`.

### Fresh geometry rejection for 203100 and witness-auditor controls

The preserved native search binary rejects `203100.jpg` against `200501.jpg`: 21 distinct proposals, no model/inliers/regions/supports, no retained managed memory. `docs/research/dedup-affine-grid-203100-native-search-audit.json` qualifies the exported point bounds, distinctness, terminal report and preserved-source hashes. The two observed color-only region supports for this unrelated pair are not copy evidence. This is one historical-revision pair, not all-query precision.

`docs/research/dedup-folded-witness-auditor-controls.json` records one accepted valid six-region report and nine rejected mutations: shifted domain, changed model, radius, sample count, matched count, false support, duplicate case, footprint axes and input digest. This qualifies auditor refusals, not independent pixel resampling or recovery of the folded print. The six witness regions still yield zero copy supports.

### Training/heldout residual diagnosis

`diagnose_affine_region_with_footprint` reports training residuals separately from heldout evidence on the same fitted model and cached samples. It does not add pixel reads, sample allocations or relax acceptance. The original verifier skips the extra training pass. Native macOS focused qualification passes 16 tests (8 region, 4 guarded-file, 2 grid, 2 unsupported-input), including damage restricted to heldout checker interiors: all 128 training samples match while heldout errors remain visible, and the original evidence is exactly preserved. Cancellation inside the diagnostic training pass returns no partial result and releases credits. Evidence: `research/dedup-affine-training-diagnostic-tests-state.json`. This new revision has not yet been qualified on real folded images or Linux; prior snapshots remain evidence for their pinned revisions.

### Guarded file training/heldout diagnosis on the folded print

`diagnose_affine_region_files_with_footprints` uses the same guarded decode/source-snapshot lifecycle, pixel work admission and directional footprints as verification. Its optional extra residual pass uses cached training pairs. Current macOS focused qualification passes 17 tests, including append, equal-content inode replacement and sticky cancellation with no partial result. `research/dedup-affine-training-diagnostic-file-tests-state.json` pins source and test logs.

The terminal seven-case diagnostic preserves original sample/read/memory evidence exactly: one screen control and six unchanged native witness geometries. The screen still passes both directions. Four fold fits have similar training/heldout fractions (approximately .814/.811 and .698/.700; .732/.720 and .934/.932; .604/.599 and .806/.801; .782/.771 and .597/.599). Two retain Color(Bounds) refusals. There are zero fold supports. Training error itself remains substantial; these measurements do not identify whether residual geometry, lighting, print texture or color nonlinearity causes the gap. No threshold was relaxed. `research/dedup-folded-training-diagnostic-audit.json` qualifies terminal output/count parity and diagnostic arithmetic, not independent pixel resampling or fold recovery.

### Negative-grid checkpoint at 111 cases

The immutable meanfix checkpoint `research/dedup-affine-grid-meanfix-checkpoint-111-audit.json` verifies 111 ordered different-origin sources and their supplied-model grid domains/radii/typed decisions. It contains five color-only region supports: one for 201900, two for 203100, one for 207300 and one for 210600. There are no native errors or grid refusals in this prefix. These are counterexamples to color-only admission, not copy decisions; the 156-case run and fresh native geometry for 210600 are still running. Prior targeted native geometry rejects the first three cases at their preserved binary revisions; no rejection claim is made yet for 210600.

The subsequent terminal fresh-geometry run on 210600 emits 22 distinct proposals and no model/inliers/regions/supports, with zero retained managed memory. `research/dedup-affine-grid-210600-native-search-audit.json` verifies exported point bounds/distinctness, absence of a model and the preserved-source provenance. Thus all four currently observed different-origin color-support cases in the 111-case prefix have individual native no-model rejections at their historical binary revisions. This does not qualify all-query precision or the unfinished 156-case grid.

### One-pixel alignment study on the folded print

The finite translation study `research/dedup-folded-training-shifts-audit.json` audits 54 candidates: nine target-axis shifts per native witness model, fixed target witness rectangles and fixed expanded source rectangles. Candidates retain at least ten original native correspondences within two pixels; 15 shifts fail that gate, 9 retain Color(Bounds) refusals and 30 yield fits. Selection uses forward training mean squared error with equal forward training/heldout counts; heldout color scores never select a shift. Equal counts do not independently establish identical pixel membership, so this is a diagnostic rather than a qualified refinement API.

Four models select small shifts, but their heldout bidirectional fractions remain approximately [.802,.578], [.724,.939], [.655,.816], [.778,.595], giving zero supports at the unchanged .9 threshold. Small translation alone does not recover this print in the tested domains; causation of the remaining error is still unproven. No production classifier, color bounds or thresholds changed. Independent audit verifies model composition, exact native-point residual membership, fixed rectangles and training-only selection arithmetic, not pixel resampling or general copy recall.

### Local ordinal region evidence and Linux diagnostic qualification

`rank_region::compare_rank_region` adds allocation-free eight-neighbor/center luminance-order evidence under supplied geometry. It prepays at most 45 pixel reads per target center, clips source centers, rejects low-information comparisons and explicitly refuses alpha or out-of-display-range samples. It returns counts, not a copy decision. Exact mapped neutral monotone transfer invariance does not imply invariance under interpolation, colored light, quantization, cropping or all illumination changes. The four focused macOS tests cover a nonlinear neutral transfer, inversion, independent textures, solid-color refusal, unsupported pixels/domains, work refusal and cancellation. Evidence: `research/dedup-rank-region-tests-state.json`. Real folded-print, broad negative and Linux rank qualification remain pending. The real-image diagnostic example is being built; its initial private-snapshot import refusal is preserved and corrected using the public content snapshot API.

Separately, `research/dedup-training-diagnostic-linux-state.json` qualifies all 28 tests in eight focused suites of the immutable 2859-file training-diagnostic snapshot. Five existing color/region modules and eight test sources match that revision. This predates the newly added ordinal-region module/export/example; it provides no Linux qualification of that new code. Source snapshots and historical reports remain pinned to their own revisions.

### Real local-rank counts and limitations

`research/dedup-rank-region-real-audit.json` qualifies twelve supplied-model controls (one screen, six folded witnesses, five color-only negative regions), all source/model/domain memberships and 24 independently enumerated geometric tap-count checks. Contrast/agreement counts are arithmetically bounded, not independently resampled. The weakest positive bidirectional fraction is .645633, while the strongest negative is .669211. Their overlap contradicts a single rank-fraction threshold that would admit every positive and reject every negative in these finite controls. No rank-based copy classifier or lower threshold has been introduced.

`research/dedup-rank-real-auditor-controls.json` accepts the valid report and rejects eight mutations of site counts, valid counts, read counts, agreement overflow, missing case, case membership, retained memory and source hashes. `research/dedup-rank-region-linux-state.json` separately qualifies all four no-default-features rank tests on native Linux aarch64 and checks every byte of the 2861-file snapshot; current rank/lib/test source hashes match. It does not cover the real-image decoder probe, broad precision or Windows. The newly observed color-only 213500 case is outside the qualified 111-case prefix and is undergoing fresh native geometry; no conclusion is asserted yet.

### Explicit filtered ordinal evidence

`rank_region::compare_filtered_rank_region` adds a constant-storage target-axis box filter around every center and ordinal neighbor. Its caller supplies the filter radius and full checked read budget; no copy admission is added. Filter zero preserves the existing API, evidence and cancellation callback counts. The seven focused macOS tests include independent grayscale box/contrast counts, exact 564480-read accounting for a 7x7 filter on 256 centers, one-read-short preflight refusal, checked radius/work overflow and mid-window cancellation, plus the previous four ordinal gates. Evidence: `research/dedup-filtered-rank-region-tests-state.json`. Averaging does not commute with every monotone pixel transfer; no such filtered invariance is claimed. The previous rank source revision is preserved in `research/dedup-rank-region-before-filter.rs`. Real filter0/3/8 measurements on the same twelve controls are running and have no promotion claim.

The new unrelated color-only 213500 control has terminal native rejection: 23 distinct proposals and no model/inliers/regions/supports at the preserved search binary revision. `research/dedup-affine-grid-213500-native-search-audit.json` verifies report/point bounds/distinctness and source provenance. This targeted check is not full156/all-query precision.

The subsequent terminal `research/dedup-filtered-rank-real.json` records all36 measurements. Filter0 exactly reproduces all twelve original real evidence objects, including both directional counters and managed peaks. At filter8, five of six folded witness pairs exceed .9 ordinal agreement in both directions; folded3 remains around [.860,.855]. Three unrelated controls also exceed .9 (201900 and both203100 regions). Thus high filtered ordinal agreement alone remains a false-positive route, and no copy classifier/default is promoted. The report pins current filtered source and preserved original rank source; independent filtered tap/pixel auditing is still pending. These are reported directional ordinal fractions, not confirmed copy decisions.

### Terminal negative color-grid gate and filtered ordinal count audit

`research/dedup-affine-grid-meanfix-terminal-audit.json` qualifies all156 ordered different-origin sources: six color-only region supports across201900,203100(two),207300,210600,213500, zero native errors and zero grid refusals. `research/dedup-affine-grid-terminal-false-controls.json` cross-checks the six supports against five separately qualified historical native no-model rejections. This closes that supplied-model gate, not an integrated all156 retrieval/all-query precision gate. The optimized counterpart still runs.

`research/dedup-filtered-rank-real-audit.json` checks all36 fixed controls/72 directions for exact original case/model/domain membership, full filter0 evidence parity and independent geometric valid-site/read counts. The convex window fast path uses a numerical interior margin and enumerates boundary windows; `research/dedup-filtered-rank-window-oracle-controls.json` matches its validity and read counts against exhaustive taps for1800 authored affine/projective/edge/filter-size cases. Contrast and ordinal-agreement counts are bounded but not independently resampled, and neither audit qualifies a copy classifier or broad precision.

### Independent ordinal pixel mathematics and complete optimized-grid parity

`research/dedup-rank-pixel-oracle-audit.json` qualifies independent NumPy luminance, projective warp, bilinear interpolation, box-window reduction and ordinal comparison counts for all36 fixed controls/72 directions. Every valid-site, informative-pair and agreeing-pair count equals native output exactly. The eight source/query pixel dumps in `research/dedup-rank-normalized-pixels.json` come from the same native selected-frame decode/view API, with source snapshot and exact RGBA32 byte hashes. Thus this is independent rank mathematics on shared normalized pixels, not an independent decoder/color-normalization oracle. The false filtered ordinal supports persist with independently confirmed arithmetic; no copy classifier/default promotion follows.

The optimized156-case negative color-grid run is now terminal. `research/dedup-affine-grid-release-terminal-audit.json` verifies complete ordered membership, grid/source bounds and typed decisions. `research/dedup-affine-grid-release-full-parity.json` confirms exact full evidence equality with the complete meanfix debug run on all156 inputs, including directional counts, support counts and managed memory. Both retain six color-only region supports in five unrelated sources. No paired throughput speed claim or integrated all-query retrieval precision is made.

### Terminal fallback negative gate for201303

`research/dedup-fallback-release-original-negatives-201303-terminal-audit.json` qualifies all156 different publisher-origin sources with zero region supports and zero native errors on the frozen symmetric-first fallback recipe. The known201303 positive control retains46 region supports. Source/archive pins, independent locations, geometric residuals, regional domains/counts and managed-memory lifecycle are checked. This is one-query finite precision evidence, not semantic/burst or all-query precision. The broader positive229 gates remain running.

The new full-region ordinal diagnostic uses the same eight previously qualified4x4 native models, over their full fitting cells instead of smaller8x8 witness rectangles. The measurement example raises only explicit maximum work admission to2m sites per direction; neighbor/filter8, contrast.005 and minimum1000 comparisons remain unchanged. No runtime/default classifier is promoted, and independent full-cell pixel/coverage qualification is pending.

### Full-cell geometry exposes the need for local deformation

`research/dedup-folded-regional-rank-pixel-audit.json` qualifies all eight unchanged native4x4 models on their full fitting-cell domains. The disjoint target cells total240000 pixels out of480000 query pixels; none passes both ordinal directions at.9. Independent NumPy luminance, warp, interpolation, window reduction and ordinal counts equal all16 native outputs exactly. These full cells are substantially less consistent than their small witness subsets; extending each projective model does not recover the folded print. Source bbox/read accounting is bounded here, not fully independently reconstructed.

`research/dedup-folded-qualified-landmarks.json` prepares156 distinct native inlier correspondences from those eight models, each with at least10 inliers at the unchanged two-pixel tolerance. Every selected point belongs to its original fitting cell and model inlier set, and source/target locations remain more than two pixels apart. This prepares a locally varying warp investigation; it does not prove all156 matches correct, implement a warp or relax copy admission.

### Native conforming piecewise geometry foundation

`piecewise_warp::PiecewiseWarp::from_triangles` constructs a retained, memory-budgeted piecewise-affine mapping from caller-supplied landmarks and triangle indices. It verifies finite bounded coordinates, distinct source/target locations, nondegenerate orientation-preserving faces and conforming nonoverlap in both planes. Duplicate faces, target/source overlap and unshared edge contacts/T-junctions refuse the entire construction. Target/source mapping is bounded by a caller triangle-test cap, checks cancellation and returns no extrapolated point outside the mesh; shared-edge agreement uses explicit floating-point tolerances. Landmarks/triangulation remain caller-qualified candidates, never copy proof.

`research/dedup-piecewise-warp-tests-state.json` qualifies four focused native macOS mesh tests plus seven rank regressions. The authored nonlinear row-shear grid maps continuously in both directions and round-trips known locations; exact face payload credits remain shared until the last clone drops. Memory/work one-short controls and every observed construction/map callback checkpoint return no partial output. Rejected construction releases all retained credits; cancelled queries preserve the existing caller-owned mesh until dropped. These are authored floating-point controls, not an independent exact-predicate certificate, automatic native triangulation or recovery of the folded photograph. The pre-export library source is preserved in `research/dedup-lib-before-piecewise-warp.rs`; previous measurement reports retain their earlier source revisions.

## Target triangulation foundation (2026-10-08)

The raster API now proposes a bounded deterministic Bowyer-Watson target topology. Source orientation and overlap still require PiecewiseWarp validation; ordinary floating-point predicates do not provide exact geometric certificates. Native authored grid and cancellation/resource refusal tests passed together with four piecewise warp tests (six total). Evidence: `research/dedup-triangulation-tests-state.json`. Real folded-image triangulation, field sampling and copy admission remain unqualified.

The real 156-landmark folded-print proposal produced 296 target faces and used every landmark. Strict native source validation refused the mesh (Invalid); independent orientation checks identify 1 inverted source faces. Evidence: `research/dedup-folded-triangulation-audit.json`. This correspondence union cannot be admitted as a whole deformation field; no faces were silently removed and no thresholds were relaxed.

Geometry-only diagonal alternatives for the real inverted face were enumerated without deleting landmarks or faces. Exactly one of three alternatives preserves the target quadrilateral and positive source orientations. Changing faces 213/214 passes the complete native PiecewiseWarp source/target overlap validation (156 landmarks, 296 faces). Evidence: `research/dedup-folded-topology-state.json`. This is a supplied-topology diagnostic, not automatic topology repair or pixel/copy qualification.

Whole-domain offline mesh ordinal diagnostic (2026-10-08): the accepted supplied topology covers 210968 of480000 query pixels (43.95%). Complete-window filter8 ordinal agreement is0.78625 forward (177216 valid sites) and0.72796 reverse (894840 sites), below0.9. A single unrelated-source forward control gives0.44553, with a different valid domain due to image dimensions. Nine measurements at filter radii0/3/8 are preserved in `research/dedup-folded-piecewise-pixel-diagnostic.json`; integral window means/masks agree with direct windows on12 authored cases. No native mesh pixel parity, all-negative precision or copy-admission qualification is inferred. Sparse accepted landmark geometry still does not align the full physical deformation accurately enough.

## Live full original-resolution prefix audits (2026-10-08)

Both original229 producers were confirmed live via their existing process handles. Immutable snapshots were independently audited: fallback123 and symmetric204 rows, with source/archive identity, distinct controls, geometry and regional arithmetic checks. fallback: {'local_supported': 112, 'miss': 11, 'native_error': 0}. symmetric: {'local_supported': 185, 'miss': 19, 'native_error': 0}. These are prefixes, not terminal full229 qualifications. Evidence paths are recorded in `research/dedup-six-searches-state.json`; original running reports and processes are preserved.

Triangulation hull regression: independent circumcenter and monotone-hull tests exposed lost convex-hull area on seeded cloud8 with the original finite supertriangle. Enlarging its normalized extent and adding budgeted supporting-boundary validation fixes the24 seeded clouds; seven native tests pass including every observed cancellation checkpoint. Current real156-landmark topology is unchanged and still refuses source orientation before the separately supplied diagonal alternative. Ordinary f64 remains approximate; no exact-predicate or all-input triangulation guarantee. Evidence: `research/dedup-triangulation-hull-fix-state.json`.

Native bounded source-diagonal proposal API now preserves all landmarks and face count, checks local target convexity/area and source orientation, limits flips/predicate work, retains output memory credit and cancels without partial output. Nine focused native tests pass, including every observed proposal cancellation checkpoint. The real156-landmark automated proposal produces296 faces matching the offline alternative and passes full native PiecewiseWarp conformity. This proposal is not a complete topology search; previously measured whole-domain pixel disagreement remains unresolved. Evidence: `research/dedup-diagonal-proposals-state.json`.

Native MeshGrid rasterizes accepted mesh faces at integer centers and retains explicit absent samples outside the mesh. Bounds cap sites, triangles and face-bounding-box visits before managed output allocation. Shared coordinate credit survives clones; cancellation returns no partial atlas. Eleven native focused tests pass across four suites, including two atlas tests with both-direction point-query parity and resource/cancellation guards. Real atlas/pixel parity is pending. Evidence: `research/dedup-mesh-grid-tests-state.json`.

Real native MeshGrid qualification: both integer-coordinate atlases from the automatic156-landmark topology match an independent affine-system/half-plane oracle at every site (480000 forward and3145728 reverse). Covered sites:210968 forward,968526 reverse; hole masks match exactly and maximum coordinate error is below1e-7. The first16MiB probe successfully wrote the forward atlas then correctly refused the larger reverse allocation; a separate128MiB probe completed both, preserving the initial refusal evidence. Native pixel interpolation/rank confirmation remains pending. Evidence: `research/dedup-folded-native-mesh-grid-audit.json`, raw coordinate dumps pinned there.

Native compare_mesh_rank computes whole-atlas ordinal evidence using managed source/target luminance prefix sums and exact missing-sample count windows. It pre-admits sites, conservative five image reads/site and scratch payload; cancelled computations retain no partial evidence. Eleven focused native tests pass: identity/inversion parity with direct projective windows at0/1/2, exact scratch/read budget boundary, overflow and every observed cancellation checkpoint, plus existing grid/rank suites. Prefix sums alter floating point order; real pixel/count parity and copy admission remain pending. Evidence: `research/dedup-mesh-rank-tests-state.json`.

Real native mesh rank qualification: guarded source/query decode, automatic156-landmark geometry, both native atlases and prefix-window rank evidence completed. All six comparisons at radii0/3/8 have exact site, coverage, valid, informative and agreement count parity with independent offline mesh math. Source bytes and frozen probe binary are pinned; content snapshots verify after both comparisons and managed credits release to zero. Filter8 fractions remain0.786249 forward and0.727964 reverse: this confirms implementation parity, not sufficient physical-deformation alignment or copy admission. Evidence: `research/dedup-folded-native-mesh-rank-audit.json`.

On the same independently qualified123 known-positive pairs, fallback supports112 versus symmetric110, with gains201303.jpg and207002.jpg and no losses. Eleven shared misses are retained in `research/dedup-fallback-symmetric-prefix-123-comparison.json`. The complete all-feature native crate test suite is running with346 pinned source/manifest/test/example files; no terminal success is inferred from the live process. State: `research/dedup-all-features-current-20261008-state.json`.

Fixed-mesh residual localization partitions all forward filter8 native counts across296 faces exactly, with deterministic shared-edge ownership. Sorted by maximum target edge, quartile agreement fractions are0.968287,0.940553,0.876718,0.764328. The largest-edge quartile contains148948 of177216 valid sites (84.05%); its edges span74.615..382.998 pixels. This supports investigating denser local geometric proposals but does not prove density causally fixes alignment. No geometry, thresholds or copy decisions were changed. Evidence: `research/dedup-folded-mesh-face-rank.json`.

Dense seed translation diagnostic: fixed32-pixel seed lattice and169 bounded source offsets assessed175 positive and113 unrelated candidate windows. Offsets selected by target-defined training support, with weak source contrast counted as disagreement; disjoint centers assessed the heldout score. No proposals reached both fixed0.9 gates. Filter3 source-axis and target-axis boxes have different physical footprints, unlike the final whole-mesh target-axis gate; this run therefore does not prove common-footprint translation refinement fails. Windows/filter taps overlap between center partitions, so this is not independent spatial holdout. Evidence: `research/dedup-folded-dense-seed-diagnostic.json`.

Common target-axis proposal filtering finds1 eligible positive of168 seeds and0 of106 unrelated seeds at unchanged0.9 training/heldout center gates. Original156 landmarks plus that single proposal pass the same frozen native mesh topology/conformity/grid/rank pipeline. Every domain/coverage/valid/read count remains unchanged across six comparisons; filter8 whole-domain agreement improves from0.786249 to0.7891640479846248 forward and0.727964 to0.7316943244443579 reverse. Improvement remains far below0.9; local proposal success is not copy admission. New pixel counts lack independent resampling audit. Evidence: `research/dedup-folded-augmented-native-mesh-rank-state.json`.

Broader fixed625-offset common-footprint search over±24 source pixels yields3 positive proposals from168 seeds and0 from104 unrelated seeds. Direct49-tap mean/bilinear oracle exactly reproduces all selected scores for both narrow274 and wide272 seed cases. Original156 plus3 distinct correspondences pass whole native geometry, retaining identical coverage and valid counts. Filter8 fractions are0.7984468717301685 forward and0.7433824049160532 reverse, still below0.9. New full-field pixel counts remain without independent audit; one accepted proposal lies on the search boundary, so optimization completeness is not claimed. Evidence: `research/dedup-folded-wide-augmented-state.json`.

The159-landmark augmentation now has independent full-domain pixel qualification: all six native valid/informative/agreement count sets equal separate NumPy mesh resampling, box sums and ordinal math. All original156 points remain unchanged, every point is used, and independently computed source/target face orientations are positive. This closes the previous independent-count gap for the wide augmentation; fractions still miss the unchanged0.9 gate. Shared decoder and fixed supplied proposals limit scope. Evidence: `research/dedup-folded-wide-augmented-pixel-audit.json`.

Spatial outer test for the three eligible local shifts uses disjoint target taps: training taps lie within21 pixels on both axes, each outer tap has at least one axis33+ away. All outer informative supports remain identical for zero/selected shift. Fractions baseline→shifted:264pairs0.56818→0.48106,436pairs0.59862→0.52064,221pairs0.43891→0.64706. None reaches0.9; two deteriorate. Local source translations therefore cannot be admitted as constant corrections over these outer regions. Source taps can still overlap geometrically. Evidence: `research/dedup-folded-wide-proposal-outer-holdout.json`. The measured159-landmark mesh gain remains finite and is not a broad deformation/copy guarantee.

Dense smaller-window proposal diagnostic uses31-pixel footprints and16-pixel lattice, retaining original contrast/min-pair/0.9 proposal gates.702 positive seeds yield40 candidates;454 unrelated seeds yield0. Direct-window oracle verifies all1156 selected scores. Two proposals violate unchanged source/target distance>2 uniqueness; their refusal provenance is explicit, and the all196-point diagnostic is preserved. The194-point distinct proposal set preserves original156 and passes whole native geometry. Coverage/valid counts remain unchanged; filter8 fractions become0.8052580488286741 forward and0.757413878275499 reverse. New full-field independent audit and native bounded proposal API remain pending; no copy admission. Evidence: `research/dedup-folded-dense-distinct-state.json`.

Dense194-landmark qualification now independently reproduces all six whole-domain pixel count sets and every native coordinate/hole in both generated atlases.372 faces use all194 landmarks and preserve original156. This closes the prior independent dense-field pixel/count gap. A second fixed-policy proposal pass is running on the qualified updated field, with already represented target locations excluded before scoring; no result is inferred yet. Evidence: `research/dedup-folded-dense-distinct-pixel-audit.json` and `research/dedup-folded-native-dense-mesh-grid-audit.json`.

Second fixed-policy small-window pass on the independently qualified194-point field evaluates660 positive seeds and433 unrelated seeds;13 positive proposals and0 unrelated pass. Direct selected-score oracle validates1093 cases. Every prior194 landmark is retained; all13 new points pass source/target distance>2 uniqueness and whole native geometry.207-point whole-field filter8 fractions become0.8110524003670028 forward and0.7708421441725248 reverse with unchanged domain/coverage/valid counts. Iterative reused pixels are not independent spatial holdout; new full-field independent pixel audit remains pending and0.9 admission still fails. Evidence: `research/dedup-folded-second-pass-state.json`.

The207-point second-pass field now has exact independent full-domain count parity for all six comparisons; original156 and preceding194 points remain unchanged. Latest independently audited live prefixes: symmetric219 rows199 supported/20 misses/0 errors; fallback142 rows130 supported/12 misses/0 errors. Both229 producers were confirmed live. A separate current-source Linux raster gate for six rank/mesh suites is running on a refreshed immutable snapshot; no Linux pass is inferred until terminal qualification. Evidence: `research/dedup-folded-second-pass-pixel-audit.json` and current state references.

Current Linux aarch64 raster-only gate passes20 tests across six suites (rank7, triangulation3, diagonal2, piecewise4, grid2, mesh-rank2). All2896 snapshot files retain exact bytes; current exercised core/dedup sources match. Concurrent PDF/decode changes are explicitly excluded by the locked raster dependency tree (no hayro or rrrah-decode package). Test-output buffering interleaves stderr suite headings; qualification therefore checks all20 exact declared test names and six successful result counts rather than naive adjacent heading/result pairing. No Linux decode/corpus/Windows claim. The separate all-feature gate remains running and cannot qualify later concurrent PDF changes. Evidence: `research/dedup-mesh-linux-state.json`.

## Terminal native all-feature crate gate (2026-10-08)

The previously running all-feature rrrah-dedup gate exits0 and passes491 tests across81 result groups (including empty doc-test group), with0 failures,ignored,measured or filtered tests. All346 pinned current core/decode/dedup source,manifest,test/example files match. This qualifies the compiled native crate suite, including new mesh/rank/triangulation modules; concurrent unpinned PDF dependency/test changes after compilation are explicitly outside its scope. It does not certify the full workspace,current PDF edits,Windows,Linux decoder/corpus or complete duplicate recall/precision. Evidence: `research/dedup-all-features-current-20261008-state.json`, verifier checks exact result counts and every pinned byte.

Native mesh_refine proposal API now performs bounded source translation search in fixed31-pixel windows using cached mesh taps and managed prefix sums. All candidates share worst-offset geometry support; target-defined information is constant, weak source contrast is disagreement, stable ties choose first(y,x), and only training counts select the offset. Seven focused native tests pass including3 new tests for known translation, output shared credit, every observed cancellation, prepaid budget refusal, flat and unsupported-alpha inputs. No real proposal parity/Linux or copy-admission proof yet. Historical491-test all-feature gate predates this new module/export; old lib bytes are preserved. Evidence: `research/dedup-mesh-refine-tests-state.json`.

Native release mesh-refine parity: guarded normalized pixel input streams, native156-landmark triangulation/diagonal/conformity/atlas and625-offset proposal search completed on all1156 archived seeds. Every selected offset, target-defined training/validation pair count, agreeing count and eligibility matches independently direct-window-verified offline data exactly (702 positive seeds40 proposals,454 unrelated seeds0). All managed payloads drop to zero and input content snapshots verify after search. Initial probe header-length compilation error is preserved separately; corrected release build/probe hashes are pinned. Shared native decoder pixel dumps are inputs; this does not qualify raw decode wrapper, spatially independent validation, full negative precision, native whole-field proposal adoption or copy admission. Evidence: `research/dedup-native-mesh-refine-parity-audit.json`.

Native mesh landmark admission preserves all original points and appends eligible proposals in stable order only if both source and target locations remain strictly more than the configured minimum separation from retained points. Original aliases/malformed scored evidence refuse the operation; proposal proximity is explicitly counted. Capacity and conservative pair work are prepaid; cancellation returns no partial managed payload. Eight focused tests pass (3 admission,3 refinement,2 mesh rank), including every observed cancellation checkpoint and retained shared-credit lifetime. Historical pre-admission mesh_refine source is preserved byte-for-byte. Real whole-field adoption probe is running; Linux and complete copy admission remain unqualified. Evidence: `research/dedup-mesh-landmark-admission-tests-state.json`.

Terminal native whole-field refinement/admission parity: release probe exits0, all normalized input snapshots verify, managed payload drops to zero. All156 original landmarks preserved;40 eligible proposals yield38 new distinct locations and2 explicit proximity refusals, total194. Complete re-triangulation/source-diagonal proposal/whole-mesh conformity passes. Six forward/reverse rank reports (filter radii0,3,8) exactly match independently pixel-qualified frozen194-landmark results; all194 coordinates match within1e-8 and every source/target pair remains strictly separated by>2. This closes native adoption parity for this supplied real field, not automatic retrieval/file API, independent spatial holdout, broad negatives, Linux or final copy admission. Strong-filter agreement remains .805258 forward/.757414 reverse, below unchanged .9 gate. Evidence: `research/dedup-native-mesh-refine-adoption-audit.json`.

Native Linux frozen raster snapshot passes26 tests across8 suites, including translation refinement and landmark admission. All2901 snapshot files match and current dedup sources match. Concurrent live `rrrah-core/src/develop/ahd.rs` change after capture is reported separately and not qualified by this frozen run; first current-source drift rejection log is retained. Linux raw decoder/real corpus/Windows remain outside scope. Evidence: `research/dedup-mesh-refine-linux-state.json`.

Terminal native second-pass adoption repeats the supplied194-point field:13 eligible distinct proposals,0 proximity refusals,total207, all194 original points preserved exactly. Complete whole-mesh conformity passes and all6 bidirectional rank reports match independently pixel-qualified offline207-point results exactly. Strong-filter agreement .811052/.770842 remains below unchanged .9 gate. This is real iterative adoption parity, not complete copy admission or independent spatial holdout. Evidence: `research/dedup-native-mesh-refine-second-adoption-audit.json`.

Symmetric12.8M source-domain union full229 producer exits0 and independent terminal audit verifies all229 rows, source/archive hashes, oriented dimensions, distinct locations, geometry/region math and managed cap. Counts208 local_supported,21 miss,0 native_error. Miss inventory:5 rows without accepted global geometry;16 rows with accepted geometry and region attempts but0 supported local regions. Region math agreement is not full resampling oracle or integrated copy/collection/negative precision. Evidence: `research/dedup-combined-domain-release-original-full-terminal-audit.json` and `research/dedup-combined-domain-terminal-miss-inventory.json`.

Main-problem refusal analysis verifies all21 terminal union misses:5 lack accepted global geometry; all16 geometric positives have at least one region satisfying min1000 compared pixels and coverage>=.3, but fail unchanged bidirectional matched fraction>=.9. Closest supported minimum fractions:207002 .8975;205901 .891882;207401 .872238. These predicate failures do not isolate blur, warp or color causally. Native exhaustive 4-point tests on all5 sparse supplied sets find no min10/tolerance2 model for200302,201303,209401,211902; best unconstrained210202 model has10 inliers but forward whole-source corner denominator crosses zero, so this witness is ineligible. Lower-ranked domain-eligible models have not been excluded. Native geometry probe release build and all5 runs exit0. Fixed-geometry radius8 confirmation campaign is running for all16 photometric misses (205901 separate, other15 sequential); thresholds unchanged and no default promotion. Initial wrong frozen binary rejected the unsupported mode before measurement; failed logs/pins retained separately. Evidence: `research/dedup-main-problem-refusal-analysis.json`, `research/dedup-main-geometry-miss-exhaustive.json`, `research/dedup-main-geometry-miss-210202-domain-check.json`.

Main-problem radius8 campaign terminal: all16 prior photometric misses completed with0 native errors; every correspondence/model/inlier and every region domain equals radius2 baseline exactly. Four gain local supported regions with unchanged .9/min1000/coverage .3:205901(3 regions),206301(1),207002(1),207401(2);12 remain unsupported. Provenance, managed bounds and predicates audited, not independent resampling/whole copy/default promotion. Full same-recipe624 different-origin comparisons (156 for each recovered query) now running; current prefix is not a precision proof. Evidence: `research/dedup-main-photometric-misses-radius8-audit.json`.

Exhaustive whole-domain projective selection now exposes `verify_projective_for_domains`: rejects ineligible hypotheses before ranking, also constrains refinement, and retains work/cancellation refusal without partial results. Shared corner-sign/inverse eligibility arithmetic also used by existing sampled API.21 focused native tests pass (3 new,4 sampled-domain,14 projective), including stronger horizon8-point consensus versus weaker valid6-point consensus, every observed cancellation and one-short work refusal. Historical pre-change geometry source bytes preserved exactly. All5 sparse real misses re-run with exhaustive eligibility:4 no model,210202 obtains domain-valid11-inlier model (independent residual/adjugate/corner checks pass). This corrects the earlier unresolved possibility of lower-ranked eligible models; new model is undergoing native pixel confirmation. No automatic file fallback/default promotion or new Linux API proof yet. Evidence: `research/dedup-projective-exhaustive-domains-tests-state.json`, `research/dedup-main-geometry-domain-miss-audit.json`.

New exhaustive-domain API Linux qualification: frozen native Linuxaarch64 no-default-feature snapshot passes21 tests across3 suites; all2904 frozen files intact and all current dedup Rust/manifest files match. Minimal dependency graph excludes core/decode/PDF, so concurrent RAW changes outside this graph are not claimed. Initial test-name verifier omitted a clippy attribute between #[test] and fn; failed log preserved, corrected attribute-aware source declaration check confirms all21 exact names and counts. Evidence: `research/dedup-projective-domains-linux-state.json`.

Same-recipe radius8 negatives first frozen4-row prefix independently audited:4 rejections,0 false local supports,0 native errors;624 required, incomplete. Origin/source hashes, exact order, oriented domains, point separation, geometry/region predicates and managed payload caps verified; no independent resampling or semantic/burst precision claim. Evidence: `research/dedup-main-radius8-full-negatives-snapshot-4-audit.json`. Direct visual inspection of remaining214402 and212602 against originals observes print rephotography with glasses/lens and grille occlusions respectively. These are hypotheses for local unoccluded/nonrigid evidence, not an isolated causal explanation. Evidence: `research/dedup-main-remaining-occlusion-inspection.json`.

Managed projective atlas API now reuses supplied full-domain projective geometry as a prepaid target-to-source pixel-coordinate atlas, with outside-source holes, shared managed output credit, no geometric extrapolation across a horizon and no copy decision.35 focused native tests pass (3 new atlas tests plus mesh/grid/rank/projective suites); independent analytical inverse, identity/perspective direct-window parity, every observed cancellation and one-short work/memory limits. Initial test callback type inference compile error retained; corrected test rerun terminal. Before-change geometry/grid and Cargo sources archived. Release raster-only guarded-pixel probe completes6 real whole-frame forward/reverse filter0/3/8 counts on200500/200501, drops all managed bytes to0 and validates normalized input snapshots. Independent linear-system mapping, bilinear luminance and complete-window rank math matches all6 counts exactly. This establishes whole-frame math, not arbitrary cropped-center windows, performance superiority, Linux new atlas or copy admission. Evidence: `research/dedup-projective-grid-tests-state.json`, `research/dedup-projective-rank-screen-pixel-audit.json`.

210202 supplied exhaustive-domain11-inlier model native pixel probe exits0 with0 supported regions and best support-eligible bidirectional fraction .537967, below unchanged .9; input hashes, region math and managed cap independently audited. Geometry recovery does not recover a confirmed copy and is not promoted into a file fallback. Native same-recipe radius8 negatives latest frozen10-row audit:10 rejections,0 false local supports,0 native errors of624 required, still incomplete. Evidence: `research/dedup-main-geometry-domain-210202-pixels-audit.json`, `research/dedup-main-radius8-full-negatives-snapshot-10-audit.json`.

### Managed projective center regions (2026-10-08)

`compare_mesh_rank_region` selects target centers in full-frame coordinates and restricts mapped source centers while retaining surrounding atlas context for rank neighbors and filter windows. All supplied context is charged to site/read/memory limits. Unsupported participating context pixels can refuse even outside the selected centers; this is an explicit conservative limitation. Missing context is excluded, never extrapolated.

The terminal focused gate passed 15 tests, including three new region tests covering direct perspective parity, source-center clipping, every observed cancellation, prepaid one-short budgets, invalid rectangles and unsupported context. A real rephotographed screen pair matches the independently pixel-audited direct reference exactly in both directions at filter radii 0, 3 and 8. Evidence: `docs/research/dedup-mesh-rank-regions-tests-state.json` and `docs/research/dedup-projective-rank-screen-regions-audit.json`. These checks establish local math and resource behavior; full copy admission, occlusion recovery, broad false-positive precision, Linux and comparative performance remain unqualified. Historical pre-change mesh-rank source is retained for earlier source pins.

### All existing occlusion rank regions (2026-10-08)

The fixed existing geometry and all 108 previously proposed domains were measured for each of two occluded Copydays pairs (216 region trials, both directions, filters 0/3/8). Four eligible rank-only cases reach the unchanged 0.9 agreement threshold for the eyeglasses image; the best bidirectional fraction is 0.950752. The grille image remains below threshold, best 0.818560. Thirteen native refusals are preserved in the report rather than discarded. Evidence: `docs/research/dedup-main-occlusion-all-rank-regions-audit.json`. This replaces the earlier single-selected-region observation with an exhaustive check of the existing region set, not arbitrary-region coverage. Rank alone previously accepted unrelated fixed-model controls, so these results do not authorize copy admission or a default policy change. Independent pixel verification for these newly measured pairs and same-recipe precision remain pending.

### Explicit combined local geometric/rank evidence (2026-10-08)

`local_rank::compare_local_rank_region` combines original supplied one-to-one correspondences with bidirectional direct filtered rank evidence. It requires regional witnesses inside both rectangles, separate source/target residual tolerances, full-domain eligible geometry, and rejects near-duplicate source or target points. Geometric pair checks and the aggregate site/read work of both pixel directions are admitted before work; cancellation never returns partial support. The result describes local support, not whole-image/file identity or automatic copy admission. Authored gate: 10 passed, including three new tests (`docs/research/dedup-local-rank-tests-state.json`). Real integrated pair qualification, unrelated precision and Linux remain pending.

### Integrated local-rank resolution diagnostic (2026-10-08)

The actual local-rank API was measured on five selected real occlusion regions with all original correspondences. Fixed 2-pixel source/target residual tolerances reject all five before pixels: the best eyeglasses region retains only5 of14 forward witnesses under the inverse source-pixel test. A separately frozen diagnostic declares target tolerance2 and source tolerance `2 * source_diagonal / target_diagonal`, without outcome tuning. This yields source6.4 for eyeglasses and4.0 for grille. Three eyeglasses regions retain14/10/12 bidirectional witnesses and pass unchanged0.9/.3/min1000 rank at filter8. The grille best-rank region has one witness; its populated region has12 witnesses but fails rank. Independent residual counts and exact direct-versus-cached pixel counts are recorded in `docs/research/dedup-main-occlusion-local-rank-resolution-audit.json`. The policy remains explicit experimental evidence, not default copy admission or calibrated unrelated precision. Equal image-diagonal units do not guarantee correspondence noise equivalence under cropping/nonuniform perspective.

### Local-rank boundary refusals (2026-10-08)

Two additional authored boundary tests explicitly refuse flat/uninformative and translucent participating pixels, huge-filter arithmetic overflow, overflowing rectangles, projective horizons, nonfinite policy values, insufficient in-region witnesses and duplicate points outside the chosen region. The terminal combined local-rank/region/atlas-region gate passes15 tests (`docs/research/dedup-local-rank-boundary-tests-state.json`). The previous three-test local-rank source is preserved for historical pins. Native complete positive-plus156-negative eyeglasses controls remain running; these boundary fixtures do not prove broad precision.

### Native Linux combined geometry/local-rank gate (2026-10-08)

A fresh2912-file frozen snapshot passes43 native aarch64 Linux authored tests in nine suites: local_rank5, mesh_rank_regions3, projective_grid3, mesh_grid2, mesh_rank2, rank_region7, projective_exhaustive_domains3, projective_domains4, projective_geometry14. The independent gate verifier checks exact test declarations/names, all terminal counts and all current dedup files against the snapshot. Evidence: `docs/research/dedup-local-rank-linux-state.json`. This qualifies the updated native local-rank and projective/center-region implementation on Linux; real decoder/corpus, Windows, full crate tests and broad precision are outside this gate. The preexisting historical validation metadata is explicitly listed in the snapshot; current source files are copied independently.

### Screen all-region and combined API qualification (2026-10-08)

All78 existing screen200501 regions were checked with fixed global geometry and predeclared normalized-diagonal residual tolerances. Eight regions contain at least10 bidirectional geometric witnesses; seven satisfy unchanged0.9/.3/min1000 rank predicates at filter8. Selected region58 is then tested through the actual combined `compare_local_rank_region` API with all original correspondences:22 witnesses, minimum bidirectional agreement0.953515. Filters0/3 refuse support;8 supports locally. Six independent NumPy inverse/bilinear/sliding-window/ordinal count checks exactly match the actual API. Evidence: `docs/research/dedup-main-screen-all-local-rank-audit.json` and `docs/research/dedup-main-screen-local-rank-api-audit.json`. Earlier single control had nine witnesses and remains an honest refusal. Selected positive/local evidence does not establish calibrated broad precision, automatic file/collection admission or complete coverage.

### Managed cached combined local-rank API (2026-10-08)

`compare_local_rank_region_cached` reuses the exact direct API geometry checks and decision predicates while building managed projective contexts expanded by rank radius plus filter radius. Both complete contexts are charged to aggregate site/read budgets before geometry/pixels. Atlases and summed-window scratch are released before return, including cancellation/refusal; unsupported participating context may conservatively refuse outside selected centers. The previous direct module is archived for historical source pins.

The terminal focused gate passes17 tests, including two new tests covering bidirectional identity/perspective ordinary/inverted direct parity at filters0/1/3, exact aggregate limits, tiny memory refusal and every observed cancellation. The actual cached API also matches all fields of the independently pixel-audited direct screen-region output at filters0/3/8:22 witnesses, support only at8, six pixel directions. Evidence: `docs/research/dedup-local-rank-cached-tests-state.json` and `docs/research/dedup-main-screen-local-rank-cached-api-audit.json`. Accounting is reduced by preparing context once; runtime improvement has not been benchmarked. The previous43-test Linux gate predates this cached combined API. Live origin controls retain the old frozen direct executable and are not restarted or reinterpreted as cached qualification.

### Selected real ROI process timing and origin checkpoint (2026-10-08)

Six alternating frozen release-process measurements per backend on the same screen-region58 inputs yield direct median2.898938s (range2.313606..3.208607) and cached median0.228683s (range0.204520..0.468493), observed ratio12.6767. Every process output equals the independently audited reference exactly. The measurement includes normalized-pixel loading and snapshot hashing plus both directions at filters0/3/8, under concurrent live workloads; it is not isolated kernel, decoder, retrieval or whole-collection speed. Evidence: `docs/research/dedup-local-rank-cached-process-timing-audit.json`.

The separate frozen direct full-candidate eyeglasses procedure has a verified four-row checkpoint: the positive origin pair supports locally and the first three different-origin originals reject. All nine dispatched positive geometric regions and their actual API outputs are preserved. A snapshot3 lookup raced against report progress and its failed log is retained; the actual frozen snapshot4 was subsequently audited. The radius8 photometric negative gate has a separate verified43/624 checkpoint with zero false local support or native errors. Both negative campaigns remain incomplete; no default promotion or broad precision claim follows from either prefix.

### All16 photometric misses through cached combined API (2026-10-08)

A terminal frozen supplied-geometry campaign measures every original16 photometric miss. The actual cached combined API finds filter8 local support in11 queries:200501,201702,202201,202401,203101,206301,207002,211702,211901,214402,214902. All proposed original regions are accounted for; independent NumPy inverse/residual calculations confirm every dispatched or excluded regional witness count under predeclared target2/source2*diagonal ratio/min10. No native process errors occur. Forty-two explicit filter refusals are retained rather than counted as support. Evidence: `docs/research/dedup-main-all-photometric-local-rank-audit.json`.

This qualifies finite supplied-model local evidence, not automatic file/collection classification: independent pixel oracles have only selected prior cases, broad unrelated precision is pending, and candidate retrieval is not rerun here. Native errors and refusals remain separate. The existing radius8 color method and ordinal method are complementary; no union/default promotion or full-corpus new success count is claimed. An initial NumPy matmul audit emitted floating warnings despite finite matching counts; it is retained separately. Elementwise independent coordinate arithmetic subsequently verifies all16 cases without those warnings.

### Complete original point-budget recheck (2026-10-08)

All42 previous budget refusals belong to207401 (1016 original correspondences vs diagnostic maximum1000). A separate frozen large-point diagnostic admits the canonical28000-point ceiling and its prepaid392014000 checks, preserving all1016 points and every earlier geometric/pixel threshold. All14 original eligible regions and42 filter cases complete successfully;11 regions support locally at filter8. Independent prior geometric witnesses and all actual native predicates are verified in `docs/research/dedup-main-local-rank-large-points-audit.json`. Together with the unchanged16-case campaign,12/16 photometric misses now have measured local ordinal support. No whole-file success total, automatic union/default or broad precision is inferred.

An authored regression with1016 distinct points proves max1000 refusal, one-short point-check refusal, exact sufficient-budget admission,36 regional witnesses and a duplicate at the end outside the region refusing explicitly. Eight direct/cached tests pass (`docs/research/dedup-local-rank-complete-point-tests-state.json`). The original smaller diagnostic and its42 refusals remain preserved.

### Remaining crumpled-paper/grille mesh diagnostic (2026-10-08)

Actual original/query pixels were visually inspected for202502 (printed architectural scene behind circular metal grille) and204702 (seaside scene on heavily crumpled print). The qualitative observations are pinned in `docs/research/dedup-main-grille-paper-visual-inspection.json` and are not causal proof.

A generic native normalized-pixel mesh probe measures three remaining paper/grille pairs with two explicit inputs each: every original correspondence or every previously verified global inlier. No point is removed after mesh refusal. Five constructions explicitly refuse invalid geometry;204702 with its complete52 global inliers admits a conforming piecewise mesh. Whole-frame filter8 rank is0.930720 forward and0.888551 reverse; neither low whole-frame support coverage nor reverse agreement meets existing acceptance. This provides measured nonrigid evidence, not recovery. Evidence: `docs/research/dedup-main-remaining-mesh-audit.json`. Independent real mesh/pixel math, selected local regions and broad unrelated precision remain pending.

### Complete crumpled-paper local piecewise regions (2026-10-08)

All125 originally proposed204702 region pairs are retained in the diagnostic. Three pairs contain at least10 of the unchanged52 global-inlier mesh landmarks inside both rectangles. No mesh point is removed or refitted. Native selected-center piecewise rank measures both directions at filters0/3/8. Region88 has16 witnesses and satisfies unchanged filter8 predicates: minimum agreement0.905168, valid coverage0.4272/0.536896. Region26 has higher agreement0.921775 but coverage0.151815/0.294223 and explicitly does not support; region27 remains below agreement. Evidence: `docs/research/dedup-main-crumpled-mesh-regions-audit.json`.

This is actual native local nonrigid support, not automatic duplicate admission. Independent real mesh coordinates/pixels, held-out/negative calibration and library-level combined piecewise witness admission remain pending. The prior whole-frame reverse0.888551 and low coverage remain unchanged historical evidence; selected-region accounting is reported separately.

### Independent crumpled-region piecewise pixel math (2026-10-08)

The native trace for204702 region88 exports the unchanged mesh face topology and exactly reproduces the prior six native rank rows. An independent NumPy linear-system barycentric evaluator maps each participating coordinate using those face vertices; separate bilinear luminance and sliding-window/ordinal calculations match all six forward/reverse count sets exactly at filters0/3/8. Evidence: `docs/research/dedup-crumpled-mesh-region88-pixel-audit.json`. The finite local support0.905168 is independently pixel-qualified. Face topology and native decoded inputs remain shared, so independent topology/conformity, decoder, negative precision and whole-file admission are not implied.

### Combined piecewise local-rank library API (2026-10-08)

`piecewise_local_rank::compare_piecewise_local_rank_region` now combines borrowed fully admitted mesh geometry with original one-to-one correspondences and both selected-region pixel directions. Image coordinate frames must match mesh frames; every regional geometric witness must agree with mesh source/target queries within separate declared tolerances. It preserves all supplied points and checks aliases outside regions. Aggregate pairwise point checks, worst-case witness triangle queries, expanded-context sites/reads and both-context worst-case face sites are prepaid. Temporary atlases/prefixes release credit before every return; borrowed mesh retains its owner's credit.

The terminal authored gate passes12 tests including two new combined-API tests: geometry consistency, image-frame mismatch, duplicates, exact one-short aggregate limits, every observed cancellation and tiny memory refusal. Evidence: `docs/research/dedup-piecewise-local-rank-tests-state.json`. The pre-dimensions-accessor mesh module is archived and matches historical real trace pins. Prior real selected-piecewise evidence is not yet actual combined API qualification; that integration, Linux, broad precision and file/collection admission remain pending.

### Actual combined piecewise real-image API (2026-10-08)

The frozen `piecewise_local_rank_pixels_probe` invokes the actual combined library API on204702 region88 using all52 prior mesh inlier points. Six pixel count sets exactly reproduce the independently barycentric/bilinear/window-audited reference;16 regional witnesses agree with the same fully admitted mesh in both directions within0.01 pixels. Filter0/3 refuse local support, filter8 supports. Managed temporary memory releases before the probe exits. Evidence: `docs/research/dedup-crumpled-combined-piecewise-api-audit.json`. This is finite actual-API local evidence; native topology/decoded inputs remain shared, no independent descriptor/held-out geometry or broad unrelated/default/file admission claim follows.

Fresh frozen negative checkpoints are separately verified: the full direct eyeglasses procedure includes the supported positive and19 different-origin negatives, zero false local support; the radius8 color procedure reaches60/624 verified negatives with zero false support/native errors. Both processes remain active. These prefixes are incomplete and remain tied to their original frozen direct/color binaries, not the newly added piecewise API.

### Geometric-anchor ordinal diagnostic for occlusion (2026-10-08)

A separate independent Python diagnostic selects disjoint5x5 center windows around original global inlier positions in deterministic order before pixel scoring. Candidate anchors additionally satisfy predeclared target2/source2*diagonal-ratio geometry residuals; no pixel-score-based window pruning occurs, and selections remain identical across filters0/3/8. Neighbor/filter contexts may overlap; only center regions are disjoint, so this is not independent spatial holdout.

At filter8,202502 grille has forward/reverse agreement0.949699/0.956594 over77/78 anchors with14135/14491 informative pairs;212602 grille0.954120/0.971168 over66/76 anchors with11639/13804 pairs. Both have100% valid selected-center coverage. The same rule measures204702 crumpled paper0.939398/0.943301 and200501 screen0.982753/0.976710. Evidence: `docs/research/dedup-geometric-anchor-rank-bidirectional-audit.json`. This explains a measurable difference between existing coarse rectangles and preselected geometric support windows, not a native implementation or completed recovery. Actual native anchor API, unrelated precision, selection calibration and automatic file/collection admission remain pending; existing coarse-region refusals are retained.

### Native geometric-anchor rank API (2026-10-08)

`anchor_rank::compare_anchor_rank` implements the geometry-first window selection in Rust. It validates all original supplied points, one-to-one spatial separation, both model residuals and full-image projective domains before pixel scoring. Greedy integer center windows use caller order and must be disjoint separately in both coordinate frames; at least the declared number of geometric anchors must remain in each direction. Window/point selection storage is managed. Worst-case pair/selection work, both expanded contexts and pixel reads are prepaid with checked arithmetic.

A private count-only mesh-rank helper allows zero-information windows to contribute their actual valid/site counts to the aggregate; public single-region minimum-information semantics remain unchanged. Aggregate support requires minimum informative pairs, coverage and agreement in both directions. Neighbor/filter contexts may overlap and selection is not held-out geometry or whole-file identity.

The terminal focused gate passes15 tests including three new anchor tests: identity/inversion/flat aggregation, pre-scoring overlap/boundary/mismatch/alias refusal, exact one-short limits and every observed cancellation/tiny memory release. Evidence: `docs/research/dedup-anchor-rank-tests-state.json`. The previous mesh-rank module is archived and matches historical native trace pins. Actual native real-image oracle parity, Linux and broad unrelated precision are still pending; the preceding independent Python positive study is not substituted for those gates.

### Native geometric anchor real-pixel qualification

`compare_anchor_rank` now reproduces all 24 independent NumPy direction/filter count sets on four real pairs (two grille photographs, crumpled paper, screen). All four have bidirectional local support at filter radius8 under unchanged .9 agreement/.3 coverage/1000 informative-pair thresholds. Original prior global inliers are supplied in original order; no point removal after refusal. Evidence: `research/dedup-anchor-rank-native-real.json`. This is finite local support; unrelated precision, Linux and automatic file/collection admission remain unqualified.

### Current anchor/cached/piecewise Linux gate

Native Linux aarch64 passed 55 tests across 13 suites including anchor rank, cached local rank, piecewise local rank and existing geometry/grid/rank contracts. Exact declared test names, terminal result counts, all2924 snapshot files and current dedup source/test bytes are verified by `../scripts/verify-dedup-anchor-local-linux.py`; evidence `research/dedup-anchor-local-linux-state.json`. This qualifies authored Linux behavior, not real decoder corpus, Windows, broad precision or integrated collection admission.

### Exact-geometry unrelated texture negatives

Eight deterministic independent texture pairs, each filter0 and8, retain20 exact bidirectional anchors, full500-center coverage and >=1000 informative pairs per direction, but reject local support. This ensures16 negatives reach pixels rather than reject only for missing geometry. Four anchor tests pass on macOS; the refreshed Linux gate passes56 tests across13 suites and verifies exact frozen/current dedup bytes. Previous55-test snapshot remains historical; its original anchor test is archived as `research/dedup-anchor-rank-tests-before-geometric-negatives.rs`. Evidence `research/dedup-anchor-geometric-negatives-tests-state.json` and `research/dedup-anchor-geometric-linux-state.json`. Real descriptor-qualified hard negatives, semantic/burst precision and file/collection admission remain incomplete.

### Real unrelated pixels with borrowed positive anchor geometry

Twenty-four different-origin real target substitutions preserve exact positive target frame dimensions, prior positive projective models and all original global inlier points. All72 native filter cases reach pixels; all48 filter8 directions exceed1000 informative comparisons. Zero filter8 supports; maximum direction agreement0.5929799426934097 versus unchanged0.9 threshold. Input hashes/groups/model/point preservation/anchor counts and support predicates verified in `research/dedup-anchor-real-pixel-negatives-audit.json`. This is an adversarial supplied-geometry test, not descriptor-qualified hard-negative retrieval, semantic/burst precision or automatic file/collection admission.

### Guarded selected-frame anchor file API

`anchor_rank_file::compare_anchor_rank_files` encloses selected-frame decodes and native anchor counts in source/external-dependency snapshots, propagates sticky callback/request-token cancellation, preserves all supplied correspondence points and prepays anchor work before IO. Six focused macOS tests pass: exact view/file evidence parity, immediate/mid/final cancellation, late file mutation, before-IO work refusal and managed release. Evidence `research/dedup-anchor-file-tests-state.json`. Supplied geometry support only; automatic candidate discovery/collection, real file corpus, dependency mutation fixtures, Linux/Windows remain unqualified. Previous Linux snapshot is historical to this new module/lib edit.

### Automatic candidate geometry to guarded anchor files

`anchor_rank_file::search_anchor_rank_files` obtains fresh native candidate-union geometry, then passes the complete original union correspondence buffer unchanged to guarded original-pixel anchor comparison. Outer source/dependency snapshots enclose both phases; sticky request/callback cancellation is shared. Three focused tests pass including identity and1.17-scale fixture positives with exact direct-file evidence parity and managed release, plus file mutation/cancellation/pre-IO refusal regression. Evidence `research/dedup-anchor-candidate-search-tests-state.json`. Prior supplied-file module preserved byte-for-byte as `research/dedup-anchor-file-before-automatic-search.rs`. Broad corpus/precision, collection and Linux/Windows remain incomplete; local support is never exact identity.

### Automatic anchor request-token cancellation

Both request owners now have six authored token-cancellation checks across initial/mid/final observed callbacks of automatic search. Every case returns `Cancelled` with zero retained managed credits. Identity/scaled exact direct parity and supplied-file lifecycle tests also pass. Evidence `research/dedup-anchor-candidate-token-tests-state.json`; initial missing-documentation dependency build failure preserved separately, fixed with three variant descriptions only. Original pre-token test archived byte-for-byte. Full229 real automatic-file campaign remains live; first grille202502 supports natively with all original union points, independent prefix audit pending. No collection/Linux/Windows/full precision qualification.

### Automatic grille-pair independent pixel qualification

Both real grille pairs202502 and212602 are now supported by fresh automatic candidate union and guarded original-file anchor comparison. All original correspondence points are retained. Independent finite/bounds/alias/bidirectional residual and coordinate-only disjoint-window selection reproduces native anchor counts; all four filter8 direction counts exactly match independent NumPy warp/box/rank calculations. Evidence `research/dedup-anchor-automatic-pixels-prefix-2-audit.json`. Shared prior normalized decoding is explicit. Full229 campaign remains live; this finite prefix does not establish corpus/precision/collection coverage.

### Automatic crumpled-paper and report accounting qualification

Fresh guarded automatic anchor files support204702 crumpled paper in addition to202502/212602 grille positives. All three complete original correspondence selections and six filter8 direction pixel counts exactly match independent NumPy calculations (`research/dedup-anchor-automatic-pixels-prefix-3-audit.json`). New `../scripts/verify-dedup-anchor-automatic-report.py` checks the full229 required pair order, input/source-snapshot pins, explicit resolution tolerance, native count predicates and explicit errors/refusals; frozen first3 results audit has3supports and0errors/refusals. Full campaign and native Linux file gate remain live, so neither is terminal qualification yet.

### Initial indexed anchor collection integration

`local_collection::scan_anchor_rank_collection` reuses descriptor-indexed proposal retrieval and shared batch source invalidation, then freshly runs guarded automatic anchors for each proposed pair. Complete configured union-point selection/sites/read work is admitted before consuming file input. `LocalFileError::Rank` keeps geometric/rank refusals typed as pair issues. One native macOS test retrieves all3identical edges across3IDs, verifies support, refuses insufficient work before a panicking iterator is consumed, cancels and releases retained credits. Evidence `research/dedup-anchor-collection-tests-state.json`. Prior scan/collection modules preserved byte-for-byte; new collection mutation/mid-cancel/scale recall/precision/Linux remain unqualified.

All four prioritized real automatic positives (two grilles, crumpled paper, screen) now have exact independent original-point selection and8direction pixel count parity in `research/dedup-anchor-automatic-pixels-prefix-4-audit.json`; full229 campaign remains incomplete.

### Anchor collection policy preiteration admission

The new indexed anchor collection rejects malformed local rank fractions, tolerances, radius/minimum pairs/witness counts before input iteration, and gives immediate callback cancellation priority. Twelve authored cases use a deliberately panicking iterator and verify it is never consumed with memory peak0; native test passed. Evidence `research/dedup-anchor-collection-policy-tests-state.json`. Previous lifecycle implementation archived as `research/dedup-anchor-collection-before-policy-validation.rs`; its still-live lifecycle test is not a postvalidation-code proof. New collection lifecycle rerun/scale recall/precision/Linux remain pending.

### Anchor collection mutation/cancellation/restoration qualification

The archived pre-policy-validation collection implementation passed its full native lifecycle regression:3independent sources yield3supported edges; initial/mid/final cancellation releases all managed credits; mid/late ID2 append removes both incident edges while supported1-3 survives; restoringID2 and reversing input returns all3supports. Evidence `research/dedup-anchor-collection-lifecycle-tests-state.json` pins the exact archived implementation rather than claiming the later policy guard was tested. Current-code lifecycle/preflight and new scaled indexed-versus-all-direct pair parity suites are now running, so remain unqualified until terminal.

### Automatic and guarded anchor file Linux terminal gate

Native Linux aarch64 passed59 tests across15 exact declared suites, including automatic identity/scale candidate discovery, full point direct-file parity, both request-owner token cancellation, selected-file mutation/callback cancellation/resource refusal, existing geometry/grid/rank contracts. All2937 frozen snapshot files and exact test names/counts are verified in `research/dedup-anchor-files-linux-state.json`. Current newer local_scan/collection changes match preserved historical archives; indexed anchor collection and later policy validation are outside this snapshot/gate. No real corpus Linux, Windows, broad precision or full-goal completion claim.

### Current indexed anchor collection lifecycle and scale parity

Current post-policy-validation code passed3native macOS suites: full mutation/cancel/restoration lifecycle;12before-input policy/work/cancel cases; scaled3-file indexed retrieval with exactly the same anchor evidence as all3direct pair calls, with memory released. Evidence `research/dedup-anchor-collection-current-tests-state.json`. Linux18-suite current dedup qualification launched with immutable snapshot and prior frozen dependencies; not terminal yet. Finite authored collection fixture coverage does not establish broad corpus recall/precision.

### Terminal fallback corpus and low-texture crumpled miss

Original frozen fallback process terminal0 confirmed; all229 strong publisher-origin pairs independently audited for ordered inputs/archive pins, distinct original correspondences, geometric residuals and regional arithmetic/domains. Result211local supports/18miss/0native errors; comparison to prior208-support frozen single recipe records gained/lost cases in `research/dedup-fallback-release-original-full-terminal-comparison.json`. This remains finite local support, not unrelated precision/identity/full indexed collection.

Viewed original200300 low-contrast mountain landscape and heavily crumpled rephotograph200302. Original native11points and fallback7points do not form a model. Independent SIFT grayscale/CLAHE with two feature contrast settings produces4/15/4/9distinct proposals but only4forward+inverse consistent witnesses in every variant. Diagnostic `research/dedup-low-texture-crumpled-sift-diagnostic.json` is a feature investigation, not native acceptance or recovery. Minimum10 and original-pixel thresholds remain unchanged.

### Guarded fallback-geometry anchor file API

`anchor_rank_file::search_anchor_rank_fallback_files` wraps the existing color-driven symmetric/asymmetric fallback selection, retains recipe metadata/color evidence and feeds its entire selected original point buffer into guarded original-pixel anchors. Outer source/dependency snapshots enclose every attempt and anchor comparison. Three authored tests pass for first-recipe metadata, complete points/direct-file equality, cancellation and supplied-file lifecycle. Evidence `research/dedup-anchor-fallback-file-tests-state.json`. Pre-fallback module preserved byte-for-byte as `research/dedup-anchor-file-before-fallback.rs`. This does not change attempt selection to rank-driven search; real3newly supported pairs, precision, collection and Linux qualification remain pending.

### Indexed anchor collection Linux terminal gate

Native Linux aarch64 passed62tests across18exact declared suites: current indexed collection full mutation/cancel/restoration lifecycle,12preiteration admission cases and scaled indexed-vs-all-direct rank evidence parity, plus59prior file/geometry/rank regressions. Immutable2940-file snapshot verified. New later fallback wrapper is excluded; its exact pre-fallback module archive matches frozen bytes. Evidence `research/dedup-anchor-collection-linux-state.json`. Finite authored cases, not broad real collection precision/recall, Windows or complete objective.

Optimized fallback-anchor actual-file probe build terminated0; executable and API/example source bytes frozen separately. Actual201303/207002/210202 recovered-pair checks now live in `research/dedup-anchor-fallback-recovered-real.json`; terminal outcomes pending.

### Actual recovered fallback pair201303 independent pixels

Fresh guarded fallback-anchor files support201303. Recipe2[4,0], all54original correspondences and the model equal the prior complete fallback run exactly;11anchors each direction and native support predicate verified. Independent all-point alias/bounds/residual/disjoint-window selection and normalized original-pixel warp/box/rank calculations reproduce both direction counts exactly. Evidence `research/dedup-anchor-fallback-recovered-real-prefix-1-audit.json` and `research/dedup-anchor-fallback-recovered-pixels-prefix-1-audit.json`. Original normalized decoder shared via frozen dump; no independent decoder, remaining2-pair terminal, precision, collection or Linux proof.

### Recovered fallback pairs terminal pixels and indexed portfolio integration

Actual guarded fallback-anchor API finished3/3supports for201303/207002/210202. Complete original points/models/recipes equal the prior229fallback corpus exactly; all6direction anchor/count sets independently reproduce from normalized original pixels. Evidence `research/dedup-anchor-fallback-recovered-real-terminal-audit.json` and `research/dedup-anchor-fallback-recovered-pixels-prefix-3-audit.json`. No broad precision or whole-image identity claim.

`local_collection::scan_anchor_rank_fallback_collection` now indexes symmetric/asymmetric proposal domains and confirms fresh guarded fallback anchors under shared batch source invalidation. Policy/work preiteration admission is shared with the original anchor collector.3native authored suites pass: all3identity/scaled fixture edges exactly match direct original points/recipe/rank evidence;12preiteration cases preserved; fallback file parity. Initial test-type compile failure retained in original log, corrected coordinate-list comparison. Exact prior collection module archived as `research/dedup-anchor-collection-before-fallback-portfolio.rs`. New portfolio lifecycle/precision/real collection/Linux remain pending.

### Geometry-only proposals and guarded anchor-only search

`local_scan::find_projective_candidate_union_geometry_files_managed` returns complete descriptor union and domain-eligible native model without regional pixel/color proof. Common extraction/fitting/source guards are shared with legacy region-confirmed API, which still runs its original confirmation. `anchor_rank_file::search_anchor_rank_geometry_files` independently confirms original pixels using anchors under outer guards; no regional color fitting is run/reported.4authored suites pass complete points/model/inliers/error/hypotheses parity, identity/scaled anchor parity, cancellation/release and fallback/preiteration regressions. Evidence `research/dedup-anchor-geometry-search-tests-state.json`. No speed claim; real parity/runtime/precision/Linux pending. Pre-change scan/file modules archived byte-for-byte.

Pre-geometry-only indexed fallback lifecycle completed:3independent files/3supports;3cancel phases;2ID2mutations preserve supported1-3; restored reversed input yields3supports; all retained credits released. Evidence `research/dedup-anchor-fallback-collection-lifecycle-tests-state.json` explicitly pins archived implementations. Current geometry-only changes remain outside that gate.

### Geometry-only anchor pipeline: real parity and remaining performance work

The frozen geometry-only executable reproduced the complete legacy native anchor evidence on all four prioritized real pairs (202502, 212602, 204702, 200501), including every correspondence, projective matrix, selected-anchor counts and both directional pixel counts. `dedup-anchor-geometry-real-parity-audit.json` verifies report, input, executable and archived-source hashes. Whole-process observed times were 50.33, 35.26, 59.50 and 31.17 seconds under concurrent workloads; these establish no comparative speed claim.

A separate alternating benchmark is running with six executions of each frozen backend on the same 202502 pair, checking exact anchor evidence every time. The legacy backend additionally performs regional color diagnostics. Source inspection also confirms the geometry and anchor phases currently decode the selected frames independently; a shared guarded decoded-frame lifecycle remains an optimization opportunity and requires cancellation, mutation and memory-release regression coverage. Corpus precision, remaining difficult misses and the broader contract matrix remain open.

The geometry-only pipeline now passes its expanded macOS lifecycle regression: callback and both request-token cancellation at start/mid/final, file mutation at mid/late processing, restoration, and zero retained managed memory on all paths. Identity/scale parity remains in the same test. Source/test/log hashes are recorded in `research/dedup-anchor-geometry-lifecycle-tests-state.json`; other platforms and broad real precision remain unqualified.

The explicit `scan_anchor_rank_geometry_collection` API now connects descriptor-indexed proposal retrieval to fresh geometry-only original-pixel anchors, using shared batch source invalidation and upfront anchor work admission. Its three-file identity/1.17-scale fixture produces all three pairs with exact fresh-direct correspondence, transform, inlier, hypothesis, error and anchor evidence parity. Source/log pins: `research/dedup-anchor-geometry-collection-tests-state.json`. Dedicated collection preflight/lifecycle tests are running; the currently running Linux snapshot predates this indexed API and does not qualify it. No real corpus collection recall/precision or full-contract completion is claimed.

Dedicated macOS geometry-only indexed lifecycle and preflight regressions now pass: cancellation at start/mid/final, mid/late mutation invalidating incident pairs, restored reversed-order scan, zero retained managed memory, and malformed/work admission before consuming inputs. Gate: `research/dedup-anchor-geometry-collection-lifecycle-tests-state.json`. The fresh original-file eyeglasses origin-control campaign now runs this geometry-only pair backend against all157 originals (one positive and156foreign); incomplete results do not yet prove its precision.

Anchor policy semantics are now validated before file IO in supplied-geometry comparison and all three automatic search variants. Previously invalid anchor parameters could remain unchecked when candidate geometry was absent. The shared pure validator preserves existing native thresholds; cancellation remains first. Ten malformed variants, missing-source preflight, zero peak managed memory, and existing rank/file/geometry/fallback regressions pass (10 tests across5suites); `research/dedup-anchor-file-policy-preflight-state.json` pins evidence. Live real campaigns and the ongoing Linux snapshot use earlier frozen code and do not prove this added preflight. Archived prechange modules retain the exact historical bytes.

The semantic anchor-policy validator is shared by pair and all three indexed collection variants, avoiding divergent validation. All four dedicated macOS preflight suites pass, including new fallback-collection malformed/work/cancellation checks before input iteration; `research/dedup-anchor-shared-policy-preflight-state.json` pins current source/tests/logs. This refactor predates neither the running Linux snapshot nor frozen real executables, so those results remain historical for this change.

The frozen alternating benchmark completed all12runs (six per backend, reversed order each round) on real202502. Whole-process median was103.8624s for legacy proposal/color/anchor and48.7999s for geometry-only proposal/anchor, observed ratio2.1283. Every run reproduced the exact full baseline anchor JSON, with input/executable hashes checked each run. `research/dedup-anchor-geometry-alternating-audit.json` verifies order/counts/pins/medians. Concurrent workloads and one-pair scope limit this to an observed pipeline comparison; no collection/corpus/kernel speedup or equal color-diagnostics claim. Frozen executables predate the new semantic-policy preflight, whose separate tests pass.

The geometry-only anchor file API now exposes explicit ordered smoothing via `search_anchor_rank_geometry_files_with_smoothing`, retaining the symmetric wrapper unchanged. Authored2/2,4/0,0/4 fixture tests match complete legacy point/model and fresh supplied-file anchor evidence; cancellation/release and policy preflight pass (`research/dedup-anchor-geometry-smoothing-tests-state.json`). This enables asymmetric proposals without redundant regional color work; real recovery and a rank-driven automatic fallback are not yet qualified. Initial test syntax failure remains archived in the first log; the separate retry log passes.

Fresh geometry-only origin positive214402 now has independently recomputed complete-point aliases/bounds/bidirectional residuals, selected disjoint windows, and exact NumPy count parity in both directions (139/147 anchors); `research/dedup-anchor-geometry-origin-pixels-prefix-1-audit.json`. Normalized JPEG decoder pixels are shared and pinned, not independently decoded. The explicit asymmetric release probe has completed its build and is frozen with source hashes; fresh3pair real parity is running using radii chosen by prior legacy fallback. Automatic rank-driven fallback and broad false-positive precision remain open.

Native aarch64 Linux geometry/fallback snapshot qualification completed:67tests across23suites, all declared tests and suite terminal counts verified, all2948frozen files hashed (`research/dedup-anchor-geometry-fallback-linux-state.json`). It includes geometry-only pair parity/lifecycle and fallback file/indexed collection parity/lifecycle, existing ordinary indexed and geometry/rank regressions. The snapshot predates new geometry-only indexed API, shared semantic-policy preflight and explicit-smoothing anchor wrapper; current drift is listed and these changes are excluded. Prior frozen decoder/dependency workspace is retained; no current unrelated-dependency, Windows, real precision or full-contract qualification claim.

The first explicit asymmetric real parity attempt exposed an experiment budget mismatch: source201300 is2560×1920, radius4 costs88,473,600 smoothing taps, exceeding the copied64M cap. Native code correctly returned Search(Features(Budget)); the failure and independent diagnostic are retained. A separately named128M recipe matching historical fallback work admission is being built, without changing anchor acceptance thresholds. No real asymmetric parity is claimed yet.

The corrected explicit128M asymmetric real experiment completed3/3support (201303,207002,210202), each with complete fresh native point/model/anchor/count evidence exactly matching prior frozen color-driven fallback after excluding its attempt metadata. `research/dedup-anchor-geometry-smoothing-real-128m-terminal-audit.json` verifies order, all input/executable/source hashes and full evidence equality. Prior-selected radii remain explicit inputs; no automatic rank-driven fallback or relative timing claim. The earlier64M refusal remains preserved.

The opt-in `search_anchor_rank_geometry_fallback_files` now chooses symmetric/asymmetric attempts by original-pixel anchor support, without regional color diagnostics. It prepays explicit cumulative point/selection/site/read caps for3per-phase maxima, spans all attempts with source/dependency/token guards, stops on typed refusals, and drops unsuccessful buffers. Authored identity-first, flat-three-misses, callback cancellation/release, four one-short cumulative admission and unchanged file preflight tests pass (`research/dedup-anchor-geometry-rank-fallback-tests-state.json`). Native real automatic recovery, token/mutation lifecycle, indexed integration and Linux are not yet qualified.

The rank-driven fallback authored lifecycle now also passes both generation-token owners at start/mid/final, mid/late source mutation invalidation across attempts, restored-source retry and zero retained managed memory (`research/dedup-anchor-rank-driven-fallback-lifecycle-state.json`). The new native release executable is frozen with source hashes and a3pair automatic-selection real campaign is running, separately corroborating each selected recipe against the prior frozen direct geometry-only executable. No prior radii are provided to automatic selection; real campaign completion, broad precision, indexed integration and Linux remain pending.

Automatic rank-driven fallback has its first real corroborated recovery:201303 selected attempt2[4,0] without prior recipe input, complete evidence equals separate fresh frozen direct geometry-only execution. Independent NumPy full-point alias/bounds/bidirectional residual, anchor-window selection and exact pixel counts match both directions (`research/dedup-anchor-rank-driven-fallback-real-checkpoint-1-audit.json`, `research/dedup-anchor-rank-driven-fallback-pixels-prefix-1-audit.json`). JPEG normalization decoder is shared via pinned prior dumps. The3pair automatic campaign remains incomplete; no automatic full229/broad negative/indexed/Linux promotion. The separate symmetric geometry-only origin series now audits29pairs (one positive,28foreign rejected, zeroerrors/refusals); this does not qualify automatic fallback precision.

Automatic rank-driven fallback real qualification completed3/3positive:201303 attempt2[4,0],207002 attempt1[2,2],210202 attempt2[4,0]. Full selected evidence equals separate fresh-direct execution; independent NumPy recomputation qualifies all six directions (all original points/aliases/bounds/bidirectional residuals, disjoint anchor selection and exact counts) using shared pinned JPEG normalization. `research/dedup-anchor-rank-driven-fallback-real-terminal-audit.json` and `research/dedup-anchor-rank-driven-fallback-pixels-prefix-3-audit.json`. Full229recall/broad false-positive qualification remain incomplete.

The opt-in `scan_anchor_rank_geometry_fallback_collection` now indexes the full symmetric/asymmetric proposal portfolio and confirms pairs through fresh rank-driven fallback. Shared validator admits cumulative caps before consuming inputs, common batch source invalidation is retained. Three-file identity/1.17-scale all3pair full-point/anchor/recipe fresh-direct parity passes, alongside rerun native fallback lifecycle and dedicated four one-short cumulative collection caps with cancellation priority and zero peak memory. Gates: `research/dedup-anchor-rank-driven-collection-tests-state.json`, `research/dedup-anchor-rank-driven-collection-preflight-state.json`. Real collection recall/precision, collection mutation lifecycle and native Linux for this API remain pending.

Rank-driven indexed collection mutation/cancellation/restore/reversed-order regression is running. A new immutable2960file Linux dependency snapshot includes current dedup source and33declared suites, covering latest indexed APIs, shared/automatic admission and lifecycle; source identity is preserved independently of future edits. The automatic rank-driven fallback now has its own157pair real origin-control campaign (one positive plus156foreign originals), with typed errors/refusals retained. These jobs are incomplete and provide no pass/precision claim yet.

Current rank-driven indexed collection lifecycle passes: all3identical edges, callback cancellation at start/mid/final, mid/late mutation eliminating every changed-source incident edge while retaining the unaffected edge, restored reversed-order all3supports, and zero retained managed memory (`research/dedup-anchor-rank-driven-collection-lifecycle-state.json`). Automatic fallback real origin positive is independently corroborated by complete evidence equality with previously independently pixel-qualified symmetric positive;157pair negative campaign remains incomplete. A separate all229strong real automatic rank-driven fallback campaign is running against the same prepared original JPEG corpus, with errors/refusals retained; selected3pair successes do not imply its full recall.

Low-texture crumpled200302 remains unresolved after another bounded independent proposal diagnostic: four weak SIFT model seeds × gray/CLAHE dense forward/backward Farneback, fixed16pixel lattice, cycle≤1,15pixel NCC≥.85/std≥2, aliases>2. Eight variants yield at most one surviving correspondence and no model. Source/script/result count conservation audit: `research/dedup-low-texture-crumpled-flow-diagnostic-audit.json`. No native admission, Rust dependency, weaker acceptance threshold or recovery claim; the initial two-channel sampling API failure is preserved and the corrected diagnostic uses explicit bilinear reverse-flow sampling.

### Low-texture crumpled print: bounded learned proposal diagnostic

The frozen automatic rank-driven corpus checkpoint at 20 pairs supports 19 and reports no geometry for `200302.jpg`, with no refusals or native errors. The independent origin checkpoint at nine pairs supports its positive and rejects eight foreign originals; both remain partial, not full recall or precision proof.

An isolated CPU Kornia LoFTR reference evaluated grayscale and CLAHE at maximum sides 320 and 640. Input, implementation and weight hashes are preserved; weights were loaded with `weights_only=True`. All distinct original-coordinate proposals were retained before a fixed 2-pixel target / diagonal-scaled source robust fit. Bidirectional inlier counts were 4, 5, 3 and 7, below ten native anchor witnesses. The verifier independently checks provenance, bounds, separation and all residual counts; it does not repeat learned inference, certify whole-domain geometry or confirm original pixels. This experiment does not recover the crumpled print and introduces no library dependency or acceptance relaxation. Evidence: `dedup-low-texture-crumpled-loftr-diagnostic.json`, its audit and measurement/verification scripts.

### Rank-driven collection cumulative limits: all classes and exact boundary

The current macOS preflight fixture covers all seven cumulative work classes: anchor point/selection/rank-site/pixel-read work and proposal gradient/matching/union work. Each one-unit-short cap returns Budget before consuming a panic-on-read iterator; cancellation wins in all seven cases and refused work has zero managed-memory peak. Exact cumulative caps admit empty input and return with no retained memory. The terminal current-source gate is `dedup-anchor-rank-driven-collection-caps-boundary-state.json`. The already-running Linux snapshot predates this test extension and remains separately scoped. This proves admission boundaries, not real collection recall/precision or decoding allocations.

### Crumpled print: fixed regional learned proposals

All four frozen LoFTR proposal sets were additionally partitioned into fixed target 2x2, 3x3 and 4x4 grids before fitting. Every original proposal belongs to exactly one cell; no residual-based repartitioning or post-refusal pruning occurs. The same 2-pixel target and diagonal-scaled source tolerances yield at most six bidirectional inliers in any cell across twelve variants. Disconnected union counts reach29 but cannot substitute for ten witnesses under one accepted geometry or an independently qualified conforming mesh. An independent verifier recomputes all cell memberships, bidirectional residuals, union counts and report/source hashes. Evidence: `dedup-low-texture-loftr-regions.json` and `dedup-low-texture-loftr-regions-audit.json`. No native pixel qualification or recovery is claimed.

### Profile-guided gradient descriptor borrowing

A two-second live profile of the original three-file collection (PID46189) sampled the gradient index during retrieval: owned128-f64 distance inputs involved repeated memcpy/memmove. Distance calculation now borrows both descriptor arrays while preserving arithmetic order. Seven current macOS decode-enabled gradient-index tests pass, including exact exhaustive distance/retrieval and file-pair oracles, boundary/collision behavior, cancellation and memory admission/release. `dedup-gradient-index-borrowed-distance-state.json` pins source/tests/log/profile. The initial raster-only command ran zero tests and is not validation. No timing speedup or whole-process memory bound is established; the profiler observed a process peak above the explicit managed-memory budget, which includes unqualified decoder/unmanaged/OS overhead and requires separate analysis. Existing frozen collection/Linux runs retain prior code.

### Borrowed descriptor distance: paired mixed retrieval timing

The frozen old and borrowed index modules were compiled into one native benchmark. Six rounds alternate their order on the same4096descriptors and1024queries (512exact positives,512foreign queries), checking exact retained IDs and f64 distances each round and releasing index memory. Median old retrieval is1.265415167s and borrowed retrieval0.60512825s, observed ratio2.09115202769. Source/log/frozen executable pins and count/order audit are in `dedup-gradient-index-borrow-bench-mixed-audit.json`. Concurrent jobs and the optimized dev profile limit timing interpretation; this is not a real collection or whole-library speedup, nor a full-process memory bound. The initial shorter64-query measurement is preserved separately.

### Rank-driven Linux terminal gate and current matching changes

The frozen native aarch64 Linux rank-driven snapshot completed all78tests across33suites with terminal exit0. The verifier checks declared names, every suite/result, frozen file hashes and current drift. It covers rank-driven file/indexed parity, cancellation/mutation/restore and cumulative admission plus rank/geometry regressions; it excludes current borrowed descriptor changes in gradient/index and expanded seven-class admission boundary test. Current macOS borrowed matching changes additionally pass25tests in five suites; distance arithmetic order and matching thresholds remain unchanged. State gates are `dedup-anchor-rank-driven-linux-state.json` and `dedup-gradient-borrowed-matching-state.json`. No current full-package, Windows, real collection or matching speedup claim follows.

### Rank-driven corpus first-four independent-pixel corroboration

The first four results of the frozen rank-driven full checkpoint41 match the prior independently NumPy-qualified prefix exactly after removing only new attempt/radii metadata. Input byte hashes, source tolerances and complete original point/model/anchor/directional count evidence match; each uses attempt1/radii2,2. All archived oracle pixel/script/report hashes revalidate. `dedup-anchor-rank-driven-pixels-prefix-4-parity-audit.json` corroborates four pairs/eight directions only. This is evidence transfer from the existing shared-decoder independent pixel-count oracle, not a new decoder, remainder-corpus, collection or current borrowed-gradient qualification.

### Borrowed descriptor changes: terminal real three-pair parity

Current borrowed index and matching changes reproduce complete frozen native evidence for201303,207002and210202: selected attempt/radii, original correspondences, transform and both anchor/pixel count directions agree exactly. All three fresh runs terminate successfully; input/binary/dedup snapshot pins and independent report equality audit pass in `dedup-borrowed-gradient-real-parity-audit.json`. This confirms three real positive regressions, not a full corpus/collection, fresh independent pixel recomputation, Windows or timing claim. A separate frozen native Linux five-suite current-gradient run is tracked in `dedup-borrowed-gradients-linux-snapshot.json`; result remains pending.

### Crumpled print: fixed crops condition learned inference

Unlike the earlier post-inference regional partition, a separate frozen LoFTR diagnostic conditions inference on each fixed nonoverlapping400x300target tile of the800x600print. Full source is limited to maxside640; each of four tiles is evaluated in gray and CLAHE, preserving all distinct original-coordinate proposals per tile before fitting. Fixed2pixel target/5.12source residual counts are3,2,6,3,3,4,3,6. An independent verifier checks pins, weight bytes, tile offsets/bounds, point separation and all bidirectional residual counts. The maximum6is below10native witnesses; no native full-domain/pixel qualification, conforming mesh or recovery is claimed. Evidence: `dedup-low-texture-crumpled-loftr-tiles.json` and its audit.

### Exact distance early rejection and borrowed-gradient Linux completion

Native Linux borrowed gradients complete all25tests in five suites with terminal exit0 and verified names/results/frozen source hashes. This snapshot excludes the subsequent early-exit change in gradient_index. Current macOS index retrieval now stops after a partial squared-distance sum exceeds the radius; valid descriptors are finite and all squared terms are nonnegative. Retained candidates accumulate all bins in the prior order; upfront comparison admission and cancellation checkpoints remain unchanged. Seven existing exact exhaustive/boundary/collision/cancellation/memory oracles pass. Six alternating mixed4096descriptor/1024query runs retain identical IDs/distances and512hits; median borrowed full-distance1.5397127085s versus early exit1.01059725s, observed1.52356708719ratio. Significant concurrent timing spread limits this to the microbenchmark; no real collection or current early-exit Linux claim. Gates: `dedup-borrowed-gradients-linux-state.json` and `dedup-gradient-index-early-exit-state.json`.

### Early-exit retrieval: every observed exact floating boundary

The existing exhaustive index oracle now tests all90candidate distances and their adjacent representable f64values (next_down clipped to0, exact inclusive cutoff, next_up), plus five fixed radii, for three queries and both descriptor recipes:1650searches. Retained IDs/distances exactly equal independent full128-bin accumulation, including collisions; all seven index/cancellation/budget/memory tests pass on current macOS. Gate: `dedup-gradient-index-all-distance-boundaries-state.json`. A fresh frozen Linux targeted index run is tracked in `dedup-gradient-index-early-exit-linux-snapshot.json` and remains pending. This is stronger floating boundary qualification, not real collection, broad semantic precision or whole-library completion.

### Native Linux current early-exit index terminal gate

The frozen current native aarch64 Linux early-exit index run terminates successfully with all seven tests, including1650exact distance cutoff/adjacent-float comparisons. Declared test names, terminal results and all2967snapshot file hashes verify; current dedup drift is empty at audit. `dedup-gradient-index-early-exit-linux-state.json` qualifies this targeted index suite over prior frozen dependencies, not the full package or Windows. A separately frozen current real three-file collection run (borrowed matching/index and early exit) is tracked in `dedup-anchor-rank-driven-real-collection-current.json`; older forward/reverse collection jobs continue on their original binaries. No current real collection outcome is claimed while running.

### Current automatic anchors: arbitrary-angle fixture diagnostic and mirror miss

A frozen current automatic rank-driven file probe evaluates all nine independent CC0/Pillow rotation fixture inputs. Identity,17degree,-37degree,63degree,1.17scale,2.16scale and2.16scale+17degree positives support; unrelated rejects. Mirrored positive returns no_geometry after all three recipes, exposing a reflection gap in this automatic local search path; it does not negate separate library D4/exact paths. The verifier checks every outcome, input/binary/source pins, complete original-point bounds/aliases, both whole-image projective denominator domains and native anchor count predicates. No independent pixel recomputation or real-world arbitrary-angle/reflection recall is established. Evidence: `dedup-anchor-rotation-current.json` and `dedup-anchor-rotation-current-audit.json`. Mirror proposal support remains required work; thresholds were not relaxed.

### Native canonical descriptor reflection primitive

The gradient API now exposes reflect_gradient_descriptor: canonical4x4cell rows reverse and8orientation bins negate, while caller landmark coordinates remain unchanged. Finite/nonnegative/unit descriptors are required; cancellation has priority, each fixed-size validation/permutation loop observes cancellation, and no heap or image allocation occurs. Six tests against independently physically mirrored64x64pixels (three orientations, fixed/interpolated cells) give squared descriptor distance below1e-24. Applying the permutation twice exactly restores every input bin. Start/mid/final cancellation and NaN refusal pass; all25tests across reflection/gradient/interpolated/distinct/index suites pass on current macOS. `dedup-gradient-reflection-state.json` pins evidence. This is a qualified proposal primitive; automatic file/indexed reflection recipes and native mirror pixel confirmation are still required. The initial test compile-failure log remains preserved.

### Explicit reflected candidate geometry and unchanged original-pixel support

Managed reflected source features now retain original landmark coordinates and charge their shared allocation. The explicit find_projective_candidate_union_reflected_geometry_files_managed entry point reuses low-contrast/smoothed native extraction, ratio matching, complete distinct point union and full-domain geometry. It prepays both maximum feature sets128-bin transformations before IO, observes common source/cancellation guards and preserves the ordinary path. The mirrored fixture now yields reflection geometry and passes a separate compare_anchor_rank_files call on original files with unchanged .005contrast/1000pairs/.3coverage/.9agreement/10witness thresholds. One-short reflection cap refuses before missing input IO; cancellation wins and refused work has zero peak, retained managed memory releases. `dedup-reflected-geometry-pixels-state.json` pins current sources and tests; pre-pixel three-suite gate/test source is archived. This proves one composed mirror fixture, not one combined-operation lifecycle, automatic fallback or indexed reflection retrieval, independent pixel reconstruction, negative reflection precision or native Linux. Those integrations remain required work.

### Common reflected file proposal/pixel operation lifecycle

search_anchor_rank_reflected_geometry_files now performs explicit automatic reflected-source proposals and original-pixel anchors inside common DecodeSourceSnapshot guards for both inputs/dependencies, with sticky global/both-generation-token cancellation and final cancellation priority. Reflection work is validated before outer source IO using the shared two-phase bin validator. Complete original point sets and anchor evidence equal the separately qualified calls, and the mirror fixture supports at unchanged thresholds. Callback and both token-owner cancellation start/mid/final return no result and release managed memory; mid/late source mutation invalidates, restoring bytes recovers support. One-short reflection admission refuses before missing IO with zero peak. The ordinary file geometry lifecycle/parity and reflected generator also pass:3tests/3suites in `dedup-anchor-reflected-geometry-state.json`. This is an explicit reflection recipe with a common operation lifecycle; reflection-aware symmetric/asymmetric portfolio selection, indexed retrieval, broad negative precision, independent pixel reconstruction and native Linux remain required.

### Six-phase ordinary/reflected anchor fallback

The explicit search_anchor_rank_reflection_fallback_files API prepays six ordinary/reflected symmetric/asymmetric phase maxima and all reflection bins before source IO. Common snapshots span every phase; typed errors stop rather than become misses. On macOS the ordinary fixture selects attempt1, the mirror selects reflected attempt4 with exact direct reflected points/anchors parity, and unrelated rejects after attempt6. Eight one-short work caps refuse before missing IO with zero peak allocation and cancellation priority. Start/mid/final callback cancellation and both source owners mid/late mutation produce no usable result and release managed memory; restoring bytes restores support. Initial three tests and expanded three-test suite terminate0. Gate: dedup-anchor-reflection-fallback-state.json. Reflection-aware indexed collection retrieval, broad real reflection precision/recall, independent reflected pixel recomputation and Linux/Windows remain open.

The original frozen real three-file collection session17386 terminates1: source3 returns Features(Budget) under512MiB, with observed peak526682688bytes. Pair1-2 supports, but input3 and exhaustive direct parity did not complete, so this is a budget refusal rather than a successful collection gate. Original log/report and other running current/reverse jobs are preserved. Reducing collection indexing memory remains required work.

### Collection portfolio pixel scratch release

extract_candidate_union_portfolio_proposals now scopes each additional smoothed image to feature extraction, releasing its pixel samples before append_portfolio_batch allocates the next merged descriptor buffer. Every feature batch, ordering and threshold remains unchanged. The removed overlap is78643200bytes for2560x1920RGBA32; this is a stage lifetime bound, not a measured whole-process peak gain. The indexed fixture all-pairs exact original-point/anchor/recipe parity and preflight caps suites both pass (2tests, terminal0); diff whitespace check passes. Gate: dedup-portfolio-scratch-release-state.json. A fresh frozen same-three-real-input/same512MiB native probe runs as session33786, report dedup-anchor-rank-driven-real-collection-scratch-release.json; successful real collection and exhaustive parity remain unproven pending completion. Previous failed and live probes remain preserved.

### Zero-radius smoothing single-buffer lifecycle

Native smooth_gradient_candidates_managed now uses one output allocation at radius0, preserving the existing2*pixels work admission and opaque alpha requirement. The +0f64 arithmetic retains prior one-tap signed-zero behavior; no neighbor or intermediate image is needed. Independent two-pass bit reconstruction covers negative zero, HDR extrema and subnormal samples; exact32byte peak for2RGBApixels demonstrates half the former64byte overlap. All cancellation checkpoints, one-short memory/work refusal and opacity errors release credit. Nonzero radius footprint/lifecycle tests and all indexed fixture/direct pair parity also pass:4tests, terminal0, dedup-zero-radius-single-buffer-state.json. Subsequent error-branch formatting is the only post-test source change. Real collection512MiB success and whole-process peak gain remain unproven; session33786 covers the preceding scratch-lifetime patch only, not this newer zero-radius branch.

Old reversed real collection session72433 terminates1 with source3 Features(Budget), matching the earlier old forward refusal. This does not prove traversal-order parity. Both logs/results and separately frozen running jobs are preserved.

### Current zero-radius file portfolio regression gate

Current macOS ordinary rank fallback, explicit reflected file operation and six-phase reflection fallback pass all5tests in3suites with terminal0 after the zero-radius single-buffer change. Direct original point/pixel evidence parity, ordinary/mirror/foreign selection, cumulative admission, cancellation and both-source mutation lifecycle retain prior behavior. Gate: dedup-zero-radius-file-portfolios-state.json. A separately frozen current native Linux8suite/12declared-test run is live as session53207; manifest dedup-reflection-memory-linux-snapshot.json and log dedup-reflection-memory-linux-retry.log. First direct runner launch exited126 because the script lacked executable permission; its log is preserved, explicit sh launch is live. Linux qualification, real collection512MiB success, reflection-aware indexed retrieval and broad real reflection precision/recall remain open.

### Frozen ordinary rank fallback real checkpoints75/27

New immutable full-corpus prefix75 verifies74supported,1no_geometry (200302),0pixel rejections/refusals/native errors;229total remain required. The origin-control prefix27 verifies1positive support and26foreign rejections with0false supports/refusals/native errors;156total negatives remain required. Audits pin input hashes, frozen source/binary, exact pair order/resolution policy and native support predicates. These frozen ordinary3phase prefixes do not qualify current reflection/memory code, independently reconstruct every pixel/geometry, establish exhaustive precision/recall or complete collection/contract coverage. Evidence: dedup-anchor-rank-driven-full-checkpoint-75-audit.json and dedup-anchor-rank-driven-origin-controls-checkpoint-27-audit.json.

### Reflected query in ordinary borrowed descriptor index

GradientDescriptorIndex::search_reflected explicitly admits128reflection bins, reflects a fixed-stack source query and delegates exact ordinary retrieval without allocating another stored descriptor bank. Caller coordinates remain unchanged; hits are proposals requiring original-pixel confirmation. Independent canonical permutation for both fixed/interpolated recipes yields exact direct-query hit parity. Every callback cancellation checkpoint and one-short reflection cap refuse; index ownership releases. All8tests across the new reflected query and existing exhaustive index suites pass on macOS (terminal0), dedup-gradient-index-reflected-query-state.json. Collection integration, reflected cumulative retrieval work/hit budgets and broad real precision/recall remain open. Live Linux session53207 uses its preceding frozen snapshot and does not qualify this later index API.

### Shared ordinary/reflected indexed pair proposals

gradient_file_pair_report_with_reflection queries the same borrowed descriptor index with both ordinary and reflected descriptors, retaining a single shared hit count and unique pair buffer. Admission checks2*n*n comparisons and128*n reflection bins before index allocation. Ordinary entry point delegates the same engine with one phase. Independent synthetic exact reflection retrieval adds only expected pair10-20 with5total descriptor hits; one-short comparison/reflection caps refuse with zero peak. All9macOS tests (7existing exhaustive index plus2reflection tests) pass, terminal0. Gate: dedup-gradient-reflected-pair-report-state.json. This is candidate retrieval, not original-pixel-confirmed reflection collection; shared collection policy, six-phase confirmation, mutation lifecycle, broad real precision and platform qualification remain required. Live Linux snapshot53207 predates both new index APIs.

### Native Linux reflection/memory terminal gate

Session53207 terminates0; all12tests across8declared suites pass. The verifier matches every declared test name/outcome and all2971frozen snapshot hashes. Qualified scope includes canonical reflection, explicit reflected geometry and common original-pixel file lifecycle, six-phase file selection/cumulative caps/source mutations, zero-radius single-buffer exact-bit/memory oracle, ordinary indexed fixture/direct parity and preflight. Gate: dedup-reflection-memory-linux-state.json. Current drift is gradient_index.rs and local_collection.rs because subsequent reflected query/pair/collection integrations were added after freezing. Those additions, real collection512MiB completion, broad reflection precision/recall, Windows and full contract coverage remain open. Initial launch permission failure is preserved separately; retry terminal result supplies this qualification.

### Reflected collection declared retrieval ceiling admission

The explicit reflected collection now admits2*max_features^2 comparisons and128*max_features reflection bins before iterator/source work, in addition to six-phase file maxima. Ten one-short phase/retrieval caps with a panic iterator refuse without iteration, cancellation wins and peak allocation remains0; targeted macOS test terminates0. Gate: dedup-reflection-collection-ten-caps-state.json. Mirror direct/index original-point and anchor parity has passed as an individual test in both live native sessions90040/65036; their foreign suites remain running and predate this additional admission check. No whole-suite/current collection precision/lifecycle qualification is claimed.

### Reflected indexed collection fixture and early cancellation

Sessions90040 and65036 terminate0. Actual outputs show the same two-test reflected fixture executable (overlapping builds replaced the executable before first invocation); first also passes ordinary indexed/direct collection regression. Mirror pair recovers at reflected attempt4 with exact fresh original points and anchor equality. Three-file original/mirror/foreign collection gives exactly one supported pair; both foreign pairs reject in fresh direct six-phase searches, with any retained negative anchors/recipe matching. These binaries predate the subsequently qualified ten-cap preflight. Current separate callback cancellation test at1/1000/10000 terminates0 and releases all managed memory. Gate: dedup-reflected-collection-fixture-state.json pins logs and states this scope. Late cancellation, source/dependency mutation batch invalidation, complete current suite, broad real precision/recall, real memory success and Linux/Windows remain required.

### Frozen accelerated real collection budget failure

Session4457 terminates1, native101, with third input Features(Budget) at512MiB. Frozen source/input/binary pins and failure were independently checked in dedup-anchor-rank-driven-real-collection-current-failure-audit.json. This snapshot includes borrowed matching and early-exit index, predates subsequent scratch-release/zero-radius/reflection integrations, and does not supply complete indexed/direct parity. Accelerated retrieval does not resolve its extraction memory refusal. Separate scratch-release session33786 remains live; no successful real memory claim follows from fixture tests or this failure.

### Frozen ordinary fallback checkpoints80/29

Immutable80pair prefix audit verifies79supported and1no_geometry (200302), with0pixel rejections/refusals/native errors. Origin29pair audit verifies1positive support and28foreign rejections,0false supports/refusals/native errors. Gates: dedup-anchor-rank-driven-full-checkpoint-80-audit.json and dedup-anchor-rank-driven-origin-controls-checkpoint-29-audit.json. Frozen ordinary3phase source/input/binary/order/resolution/native predicates qualify these prefixes, not all229positives/156negatives, independent all-pixel reconstruction, current reflection/memory code or full contract. Current reflected collection lifecycle baseline finds all3expected positive edges (mirror incident edges reflected) and begins measured mid cancellation; session83154 remains live, no terminal lifecycle result yet.

### Current six-phase rotation/scale/mirror and independent pixel gate

Frozen current six-phase file probe completes all9independent fixtures with terminal0: all8positive inputs support, unrelated rejects. Seven ordinary angle/scale positives select attempt1; mirrored selects reflected attempt4; unrelated exhausts6. Every input/source/binary pin, original-point bounds/aliases, both whole-image projective denominator domains and native predicates verify in dedup-anchor-rotation-reflected-audit.json. A separate NumPy oracle recomputes all original-point bidirectional residuals, greedy disjoint5x5anchor selection and original-normalized rank counts for all8supported pairs/16directions, exactly matching every anchor and sites/valid/informative/agreeing count; dedup-anchor-rotation-reflected-pixels-prefix-8-audit.json. Normalization is shared through separately dumped native PNG originals, so no independent decoder claim. This closes the prior mirror miss on this fixture set at unchanged thresholds, not broad real angular/reflection recall, semantic precision, collection lifecycle, Windows or whole-library coverage.

### Reflected collection Linux and macOS lifecycle terminal gates

Native Linux session18296 terminates0; all14tests/5declared suites and all2975snapshot hashes verify with empty current dedup drift. Scope: reflected query/pair retrieval, three-file positive/foreign direct parity, ten cumulative caps, early cancellation, ordinary collection and exhaustive index regressions; gate dedup-reflection-collection-linux-state.json.

MacOS lifecycle session83154 terminates0 after1004.16s. Three positive edges include reflected edges incident to mirror source2; baseline callback count352891925 calibrates start/mid/final cancellation. All cancellations return no report and release managed memory. Source2mutation atmid/final-minus10 removes both incident edges, attributes source2issue and preserves supported1-3. Restoring original bytes and reversing traversal restores all3supported edges; used0. Gate: dedup-reflection-collection-lifecycle-state.json. Only source2append mutations and global callback cancellation are covered here; dependency/other source/both token-owner breadth, native Linux late lifecycle, Windows, broad real reflection precision/recall and real collection memory remain open.

### Frozen reflected real origin controls first checkpoint

New six-phase file search prefix2 passes origin provenance/source/binary/order/resolution/native-predicate audit:1positive control supported,1foreign original rejected,0false supports/refusals/native errors. Audit: dedup-anchor-reflected-origin-controls-checkpoint-2-audit.json. This is only the first2of157required pairs, not156-negative precision completion, independent pixel qualification of this positive, collection scale or all-case proof. Session49650 remains live; scratch-release real collection session33786 also remains live with no terminal result.

### Frozen ordinary fallback checkpoints95/35

Immutable full-prefix95 verifies94supported,1no_geometry (200302),0pixel rejections/refusals/native errors. Origin-prefix35 verifies1positive support and34foreign rejections,0false supports/refusals/native errors. Audits dedup-anchor-rank-driven-full-checkpoint-95-audit.json and dedup-anchor-rank-driven-origin-controls-checkpoint-35-audit.json verify frozen source/input/binary/order/resolution/native count predicates; no whole229positive/156negative completion or current reflected/memory qualification follows. Independent all-pixel corpus reconstruction, broad semantic precision, collection scale and remaining contract matrix remain open. Separate reflected157pair and scratch-release collection native sessions remain confirmed live.

### Scratch release real collection terminal failure

Session33786 terminates1 after2952.514710125s; native101, third input Features(Budget), peak526682688bytes under512MiB. Every frozen source/input/binary pin verifies in dedup-real-collection-scratch-release-failure-audit.json. Early dropping of smoothed pixel scratch before descriptor merge alone does not resolve this real failure. Frozen run predates zero-radius single-buffer optimization; that optimization is not yet real-collection qualified. Same peak as prior versions indicates an earlier overlapping allocation stage remains to be isolated; it does not identify that stage causally. No all-pair parity or successful memory gate is claimed.

### Isolated source201400 extraction stage refusal

Fresh frozen diagnostic session31749 terminates0 with an explicitly printed extraction refusal (not successful extraction): original13567features and radius2smooth10301features succeed, radius4extraction returns Budget. Prior63484feature lower-bound credit66023360bytes is retained; source decode uses144666560bytes total. Radius2stage peak526682688bytes exactly matches failed collections; radius4begins at252429760livebytes, final release used0. Source/binary pins: dedup-extraction-memory-probe-source.json; output dedup-extraction-memory-probe.log. This isolates the stage on a lower-bound reproduction, not exact full collection allocation. LocalError::Budget still conflates memory, feature and work caps; causal separation via changed memory ceiling with unchanged work policy remains required before choosing a fix.

### Isolated memory ceiling causality comparison

Same frozen diagnostic binary/input/63484priorfeature lower-bound credit and work/feature policy run with only memory ceiling changed.512MiB refuses radius4;1GiB succeeds original13567,radius2=10301,radius4=10225,radius0=12272features. Radius4peak541242688bytes exceeds536870912ceiling; finalpeak555802688. Both release used0. Logs/source pins audited in dedup-extraction-memory-ceiling-audit.json. This distinguishes memory admission from work/feature limits in the isolated pipeline, not exact full collection peak or a solved production failure. Reducing overlapping/reserved buffers without changing evidence thresholds remains required.

### Per-level pixel scratch admission isolated512MiB recovery

Managed gradient scale admission now computes the largest actual per-level pixel scratch: factor1borrows callerRGBA and reserves8bytes/pixel grayscale; reduced levels reserve24bytes/pixel overlappingRGBA32/grayscale. All other detector/descriptor/grid reservations and extraction body/thresholds remain unchanged. All6scale tests including independent area sampling, full managed/unmanaged feature parity, exact peak/one-short limits, cancellation and ownership pass on macOS.

Frozen current dedup over prior dependency snapshot recovers source201400four-stage diagnostic at512MiB, terminal0: feature counts/capacities13567/10301/10225/12272exactly match prior1GiB run. Peak555802688becomes496785968, difference59016720bytes; finalused0. Gate: dedup-extraction-level-admission-audit.json verifies all2977snapshot hashes and binary/input/log pins. Current root example build had failed on unrelated concurrent vendor missing-doc lints; that log remains preserved and dependencies were frozen rather than modified. This is isolated lower-bound retained-state recovery, not full real collection memory/parity or Linux/Windows qualification.


### Per-level pixel admission: full collection and Linux retests pending

The frozen full three-file original/copy/foreign collection retest now runs
at the unchanged 512 MiB ceiling after correcting per-level pixel scratch
admission. It requires fresh direct parity for all three pairs, one supported
copy pair, two unrelated rejections, and zero retained managed memory. Its
producer validates all 2,977 files in the compiled dependency workspace and
pins inputs, binary, provenance, producer and successful build log.
Evidence: `research/dedup-real-collection-pixel-admission.json`; native process
session 43548 is pending. Earlier failed runs remain unchanged.

The same frozen workspace is separately undergoing native Linux
`gradient-scales-distinct` qualification (seven suites), session 42336.
Evidence: `research/dedup-pixel-admission-linux.log`. Neither launched run
is a completed gate, and neither establishes broad corpus recall/precision.

The full three-file per-level admission retest has now terminated with native
exit 101 after 55.697849 seconds: `collection refused: Budget`. The complete
2,977-file frozen workspace and binary/input/build/producer pins pass the
failure audit (`research/dedup-real-collection-pixel-admission-failure-audit.json`).
There is no full collection success. The native error does not attribute the
stage; aggregate retrieval comparison admission is being diagnosed separately
without changing the 512 MiB ceiling or pixel acceptance thresholds.

New immutable corpus checkpoints are audited: the original three-phase full
run has 119/229 cases, 118 supported, one no-geometry miss, zero pixel rejections,
refusals or native errors. The six-phase reflected original-control run has
7/157 pairs: one positive supported, six foreign negatives rejected, zero
false support/refusals/errors. Both frozen binaries predate per-level pixel
admission and neither checkpoint is terminal full-corpus qualification.


### Pixel admission Linux gate and precise collection refusal attribution

The native aarch64 Linux run is terminal exit 0: 60 tests in seven suites
(gradient scales/files, distinct locations, projective domains/geometry,
fixed and interpolated gradient). All declared test names, suite outcomes,
2,977 frozen source hashes and zero current dedup drift pass the verifier.
Evidence: `research/dedup-pixel-admission-linux-state.json`. This gate qualifies
per-level scratch admission and those regressions, not broad collection recall.

A separate frozen diagnostic library differs only by a stderr count line.
At the original full collection's limits it reports 109,849 indexed features,
requiring 12,066,802,801 comparisons under the existing n-squared preflight;
the configured ceiling is 12,000,000,000. All three feature banks extract and
the pre-retrieval peak is 488,562,688 bytes, below 512 MiB. The diagnostic
terminates with Budget and all source/binary/input pins pass the audit
`research/dedup-retrieval-count-diagnostic-audit.json`. This establishes the
comparison admission refusal and does not predict the full pipeline memory
peak. A dedicated next probe increases only the comparison ceiling to the
observed n-squared requirement; memory and acceptance policies are unchanged.


### Retrieval feature lifetime before fresh pair confirmation

The current typed collection lifecycle now explicitly distinguishes paths
requiring feature prechecks from fresh-file-only confirmation. Gradient and
five-search portfolio retrieval clear their managed feature banks after
pair retrieval and proposal-credit admission, before fresh decoding. The
binary feature-precheck path retains both banks as before. Requests, source
snapshots, cancellation generations, retrieved pair metadata and final
batch-wide source validation remain in the shared lifecycle. This changes
retention, not candidate or pixel acceptance policy.

A focused unit test admits two 256-byte banks plus a 16-byte pair at a
528-byte ceiling. Fresh confirmation must reuse both bank credits for a
512-byte scratch allocation; the feature-precheck variant must retain the
banks and return the same evidence without invoking fresh comparison.
Unit session 66882 is compiling. Four collection suites (ordinary, reflected,
early cancellation and late mutation/cancellation) are queued in session
57721 behind that existing build lock. Evidence logs: 
`research/dedup-feature-release-unit-tests.log` and
`research/dedup-feature-release-collection-tests.log`; compiled workspace
manifest: `research/dedup-feature-release-snapshot.json`. Qualification is
pending. The running admitted real collection session 92357 predates this
feature-lifetime change and must not qualify the new implementation.


The focused feature-credit regression is now terminal native macOS exit 0,
1 test passed, 52 unrelated lib tests filtered. Frozen complete source hashes
and exact test outcome pass `research/dedup-feature-release-unit-state.json`.
This verifies shared lifecycle credit reuse and the feature-precheck branch,
not real image or collection equivalence. Four macOS collection suites are
now running (session 57721). Native aarch64 Linux reflected/ordinary
collection and index suites are launched against the same frozen source
(session 49952, `research/dedup-feature-release-linux.log`). Both broader
qualifications are pending.

The feature-release implementation now passes the first three native macOS
collection suites: five tests covering ordinary all-direct parity, reflected
positive/foreign all-direct parity, all ten preiteration caps, and early
cancellation with final zero retained memory. Exact suite terminal counts
(1,3,1) and test names are audited in
`research/dedup-feature-release-collection-first-three-suites-audit.json`.
The late mutation/cancellation/reverse-order lifecycle suite is still running.

A separately frozen native real collection binary incorporating both memory
changes is built successfully and launched in session 81627. Its report is
`research/dedup-feature-release-real-collection.json`; all three indexed/direct
pairs, unchanged 512 MiB and unchanged pixel acceptance are required. The
comparison ceiling is the same admitted 12,066,802,801 as the previous probe.
The preceding session 92357 remains active on its own older immutable binary;
neither real collection run is a terminal gate yet.


### Feature release Linux gate and bounded index result growth

The feature-release native aarch64 Linux run is terminal exit 0 with 14
tests in five suites. All frozen hashes and declared outcomes pass the
verifier `research/dedup-feature-release-linux-state.json`. The sole current
dedup drift is the subsequent result-growth change in gradient_index.rs;
the gate qualifies the earlier frozen feature-release implementation.
No late lifecycle/full real collection or Windows claim is implied.

Exact radius retrieval now grows hit storage geometrically, bounded by the
smaller of result cap and candidate count, rather than requesting one slot
per hit. Distances, hit order, thresholds, work/result caps and cancellation
checks are unchanged. Nine macOS exhaustive/index/reflection tests pass with
all frozen source pins verified (`research/dedup-result-growth-index-state.json`).

A frozen paired native microbenchmark alternates both backends for six
rounds on 4,096 descriptors and 256 mixed queries at radius 0.5. Each backend
returns 545,163 hits per round; all IDs and exact f64 distances compare equal.
Old median is 1.2343394795s, new median 1.022389271s; median paired speedup
is 1.201058759x. Concurrent workloads were present, so this is a scoped
hit-heavy microbenchmark, not full collection latency/RSS proof. Audit:
`research/dedup-result-growth-bench-audit.json`. Native Linux current result
growth plus reflected/ordinary collection qualification is running in
session 26988 (`research/dedup-result-growth-linux.log`).

### Per-request collection generation cancellation

A new native collection regression checks both request generation owners
with a global cancellation callback that always returns false. For each
owner it covers an already-cancelled token and cancellation at callbacks
1, 1,000 and 10,000. All eight scenarios return ScanError::Cancelled and
release all managed credit; entry cancellation has zero budget peak.
Native macOS exit 0, one test passed; all frozen source hashes/log/test
outcomes are audited in `research/dedup-collection-token-state.json`.
This qualifies early/extraction request-token handling in the current
feature-release/result-growth implementation. Late request-token cancellation,
other sources/decoder dependencies, native Linux and real full collections
are not qualified by this test. Existing long collection processes remain
running on their own frozen earlier versions.


New immutable native corpus checkpoints pass their provenance/order/input
and native-count predicates: original three-phase strong copy run 139/229
(138 supported, one no-geometry miss, zero errors/refusals/pixel rejections);
six-phase reflected origin control 10/157 (one supported positive, nine
rejected foreign negatives, zero false support/errors/refusals). Evidence:
`research/dedup-anchor-rank-driven-full-checkpoint-139-audit.json` and
`research/dedup-anchor-reflected-origin-controls-checkpoint-10-audit.json`.
Both are nonterminal and use their older independently frozen binaries;
they do not qualify current feature release/result growth or full corpus
collection precision/recall. Native Linux request-token qualification is
queued behind the live result-growth build (session 69694); its compiled
source is the separately pinned collection-token snapshot.

### Current native Linux resource/index/collection gate

Both native aarch64 Linux runs are now terminal exit 0. The result-growth
run passes 14 tests in five index/ordinary/reflected collection suites;
the request-token run passes its single regression covering eight owner/
checkpoint scenarios. Frozen sources, declared names and terminal outcomes
pass their verifiers. All dedup library source files in both snapshots
match each other and current root source. The combined scoped gate is
`research/dedup-current-linux-resource-regressions-state.json` (15 tests).

This qualifies current per-level admission, feature release and bounded
result growth in the exercised index/collection/token paths. It does not
qualify late request tokens, decoder-dependency mutation, full real
collection, Windows or broad corpus recall/precision. The older macOS late
lifecycle and real collection processes remain live on separate frozen
implementations and are still pending.

The macOS feature-release collection lifecycle is now terminal exit 0:
all six tests in four suites pass, including the 1103.54-second late
lifecycle test. Mid/final global cancellation releases all credit; mid/late
source2 append mutation removes both incident edges, leaves the unaffected
edge, and restored reversed traversal recovers all three supported edges.
All frozen source hashes and exact outcomes pass
`research/dedup-feature-release-collection-state.json`. The sole current
source drift is the subsequent bounded-result-growth index. This therefore
qualifies the frozen feature-release implementation's late lifecycle, not
late request-token cancellation, other source/dependency mutations or the
newer index's late lifecycle. Real three-file processes remain live.

Current native macOS request-token regression is terminal exit 0: two tests,
including both owner-specific successful reflected baselines (154,187,550
callbacks each), midpoint/final generation changes with global callback
false, typed Cancelled and zero retained credit. Eight earlier scenarios
also pass. Frozen-source and terminal outcome audit:
`research/dedup-collection-late-token-state.json`. Native Linux session 69927
is still pending. No source/dependency mutation is inferred from these tests.

The old three-phase strong-copy run has a newly audited 143/229 prefix:
141 supported, two no-geometry misses (200302.jpg and 209401.jpg), no pixel
rejections, errors or refusals. The reflected origin controls have 11/157:
one positive supported, ten foreign negatives rejected, zero false support,
errors or refusals. These older frozen binaries do not qualify current
library recall, and the newly exposed 209401 miss requires investigation.


Native Linux late request-token qualification is now terminal exit 0, two
tests passed. Both successful mirrored baselines have 154,187,550 callback
checkpoints; each owner returns Cancelled with zero retained credit at its
measured midpoint/final checkpoint. Eight early cases also pass. The native
Mac and Linux qualified callback counts agree, without implying complete
cross-platform geometry/decoder equality. Audit:
`research/dedup-collection-late-token-linux-state.json`. Combined current
Linux index/collection/early+late token scope is 16 tests, with identical
current library source verified across both compiled snapshots:
`research/dedup-current-linux-resource-and-late-token-state.json`.

Original209400 and copy209401 were directly visually inspected: a repeating
stone mosaic and a perspective central crop of its principal scene.
Repeating texture is a correspondence-ambiguity hypothesis only. A current
six-phase diagnostic preserving original acceptance and 512 MiB is live in
session 38283 (`research/dedup-209401-current-six-phase.json`). Its source
is the same current late-token snapshot; no successful recovery is claimed.

Independent full-resolution SIFT diagnostics on 209400/209401 are terminal
exit 0. Four variants (grayscale/CLAHE, .04/.001 contrast threshold) each
have only four bidirectional inliers under target2/source-scaled tolerance,
with original-coordinate separation>2. Input and script pins pass
`research/dedup-209401-sift-audit.json`; no pair recovery, native pixel
confirmation or causal explanation is established. The live current native
six-phase run remains pending. Coarser candidate scales are a next diagnostic
for the repeating-texture hypothesis; acceptance is unchanged.
