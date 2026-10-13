# Selection after inspecting 500 repositories

Every entry in `REPOSITORIES.md` has an individual review record in `reviews/`.
The pass inspected pinned file trees and selected source excerpts, including
algorithm and test inventory where present. It is **not** a line-by-line audit of
every source file, an executed upstream test run, or a benchmark of 500 projects.
Four pinned trees have no recognized algorithm source: the EO project scaffold,
the KiCad board, a README-only tool catalog, and PerceHash's README-only scaffold.
These were reviewed as repository contents, not misreported as reviewed code.
Eight acquired files are explicitly capped prefixes; their records mark this.

Repository discovery was broad: many hits use "duplicate" for tabs, records,
game items or dependency graphs. The individual findings reject these domains.
The 500 count is the size of the inspected candidate pool, not 500 independent
relevant algorithms. Mirrors and wrappers are also not independent validation.

## Concrete directions selected for rrrah

| Requirement | References selected from the source pass | What to carry into rrrah | Verification before claiming coverage |
| --- | --- | --- | --- |
| Exact files | [fclones](https://github.com/pkolaczk/fclones), [dupe-krill](https://github.com/kornelski/dupe-krill), [zfs-dedup](https://github.com/Mic92/zfs-dedup), [dupehunter](https://github.com/amasen02/dupehunter) | Physical identity, size/sample/full-hash funnel, byte confirmation, error isolation | Same prefix/suffix but changed middle, injected digest collisions, hardlinks, file replacement, unreadable files, cancellation |
| Global visual candidates | [modhash](https://github.com/n24q02m/modhash), [blockhash](https://github.com/jaehl/blockhash), [goimagehash](https://github.com/corona10/goimagehash), [libphash](https://github.com/gudoshnikovn/libphash) | Versioned complementary hashes and independently specified pixel input; candidate scores remain separate from equality | Independent vectors, alpha/rotation/DC behavior, resizing/compression/exposure positives and solid-color/unrelated negatives |
| Crop/local edits | [perception](https://github.com/thorn-oss/perception), [cbird](https://github.com/scrubbbbs/cbird) | Local descriptors and geometric overlap verification rather than a single whole-image hash | Crop, border, watermark, unrelated textured images, insufficient overlap, transform degeneracy |
| Candidate index | [imagededup](https://github.com/idealo/imagededup), [IntraArchiveDeduplicator](https://github.com/fake-name/IntraArchiveDeduplicator) | Exact metric pruning, collision buckets, deterministic pairwise retrieval | Seeded exhaustive-search equivalence across radii and adversarial distributions |
| False-positive controls | [advhash](https://github.com/mattpodolak/advhash), [Learning-to-Break-Deep-Perceptual-Hashing](https://github.com/ml-research/Learning-to-Break-Deep-Perceptual-Hashing), [cutmap](https://github.com/xykong36/cutmap) | Explicit low-information rejection and perceptual-collision negative fixtures | Equal visual hash must never become exact-file or exact-pixel evidence |
| Normalization evidence | [dhash](https://github.com/benhoyt/dhash), [pil-agent-plugin](https://github.com/bsmi021/pil-agent-plugin), [imghash](https://github.com/ajdnik/imghash) | Alpha handling, alignment tests and compare-input validation | Existing rrrah color/orientation/RAW paths, HDR normalization, multipage/animation frame policy |

This selection is based on source suitability and exposed tests, **not measured
superiority**. The actual licenses of selected packages/files and dependencies
must be checked at the review commit before incorporating upstream code. In
particular, GPL/AGPL sources remain references unless a compatible license route
is explicitly chosen. No third-party source was copied in this pass.

## Implementation and remaining work

`rrrah-dedup::HammingIndex` is independently implemented and tested against full
search. `rrrah-dedup::exact` now implements a bounded sample/full-hash/byte-confirm
scan with aliases and diagnostics. Its finite authored nested-partition,
collision, cancellation and source-change gates are recorded in the acceptance
contract; native Windows and broad scale qualification remain pending.
Neither establishes the complete image-duplicate library by itself.

Source-inspired local descriptors, projective geometry and bounded color
confirmation now have independent implementation and explicit resource/refusal
evidence. The integrated registration/two-filter file API executes all 229 Copydays
strong-subset pairs and recovers 30, leaving 199 omitted. Its reported per-family geometry,
pixel counts and decisions match the fixed standalone measurements on all 229
pairs; all 386 normalized input images were verified. Full measured evidence:
`../dedup-complementary-filter-portfolio-full-state.json`.

The subsequent shared-view four-search file API retains all30 and recovers five
additional publisher-origin copies:35 of229, with194 still omitted. All reported
constituent geometry, pixel counts and decisions equal the standalone references;
all386 normalized inputs rehash. Its six-query negative gate finishes936
different-origin comparisons without candidates, errors or timeouts. This is a
measured improvement on this fixed corpus, not broad copy recall or semantic/burst
precision. Evidence: `../dedup-four-lane-full-parity-audit.json` and
`../dedup-four-lane-six-query-negative-terminal-audit.json`.

A separate explicit radius-3 smoothing diagnostic recovers 27 of 229, with eight
gains and one loss compared with radius 1. It preserves the initial native
geometry on all pairs, but is not promoted as a replacement default because it
loses `203102.jpg`. Two queries each reject all 156 different publisher-origin
groups; these finite negative gates do not establish all-query precision or
burst-photo discrimination. Evidence: `../dedup-pyramid-blur-state.json`.
These measured gaps prevent a claim that the source selection already covers
every duplicate-search case.

The full objective remains the acceptance matrix in
`docs/DUPLICATE_LIBRARY_CONTRACT.md`: exact copies, decoded-pixel equality,
transformed visual candidates, RAW/raster color, alpha/HDR, animation and pages,
mutation/cache invalidation, resource bounds and measured search behavior.

Reproduce evidence integrity and 500-record coverage with:

```sh
python3 scripts/verify-dedup-source-review.py
```

The verifier checks counts, unique identities, pinned commits and all archived
file checksums. It does not certify algorithm correctness or broad coverage.


## Follow-up of six ambiguous source records — 2026-10-11

All six records previously marked `needs-deeper-source` now have a separate
pinned-source follow-up in `deeper-source-20261011/review.json`. Nine additional
full files are archived and checked against the original tree's Git blob IDs.
The follow-up distinguishes an actual byte-confirmed exact-file funnel from
MD5 comparison of JPEG streams after EXIF removal, identifies a concrete
63-bit image pHash, and records question-ranking/game-item projects and the
sample-only Android repository as unsuitable algorithm evidence for this scope.
The original 500 reviews remain historical. This selected-section follow-up
neither executes upstream tests nor proves full library case coverage.


2026-10-11 evidence refresh: `source-coverage-audit-20261011.json` verifies all500 historical individual review records and every referenced archived source/readme/license digest, with zero integrity errors. The historical records contain source material for496 repositories; the four source-absent followups remain separate evidence. This checks material integrity, not complete algorithm or runtime review of all500.

The full pinned `kornelski/dupe-krill` archive now matches all17 original Git blob IDs. All Rust source and tests were read; eight original locked macOS library/file tests pass. Separate public-API controls reproduce stale equality after a cached same-length rewrite and after distinct suffix growth beyond previously captured sizes. Incremental mismatch exit, lazy handles and physical-alias accounting remain useful ideas; unvalidated range-hash caching, digest-only identity, fallible IO inside ordering and destructive default linking are not adopted. No upstream code is copied into production. The exact rrrah regression covers both observed counterexamples and passes all17 macOS exact-file tests. Evidence: `dupe-krill-full-source-20261011.json`, `dupe-krill-full-review-20261011.json`, `../dedup-exact-upstream-adversary-verification-20261011.json`. This does not qualify scanner/linking modes or native Windows/Linux.
