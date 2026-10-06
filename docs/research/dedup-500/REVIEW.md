# Open-source duplicate-search research

Research date: 2026-10-06. The goal is a Rust library covering exact file
duplicates and visual image candidates. Discovery and source review are separate:
`repositories.json` records 500 unique public non-fork candidates with recognized
GitHub SPDX metadata, filtered by topic. All 500 now have individual source-excerpt or repository-inventory triage records. This is not a full-file audit; see `SELECTION.md`.
Raw query snapshots and the discovery script preserve the selection evidence.
GitHub license metadata is screening evidence; code reuse requires checking the
actual file and package licenses at the pinned commit.

## Source-reviewed shortlist

Exact revisions are recorded in `source-pins.json`. Source was read from clones
under `/tmp/rrrah-dedup-research`; it has not been vendored into rrrah.

| Project | Evidence inspected | Adoptable approach | Limit or tradeoff |
| --- | --- | --- | --- |
| [Czkawka](https://github.com/qarmin/czkawka) | `czkawka_core/src/tools/duplicate/mod.rs`, `tools/similar_images/mod.rs`, hardlink tests, core Cargo manifest | Staged exact filtering, explicit hardlink handling, metric index, reusable core | Core package MIT; whole repository has component licenses. Current core requires Rust 1.94.1; rrrah declares 1.89. Direct dependency needs compatibility review. |
| [imagededup](https://github.com/idealo/imagededup) | `handlers/search/bktree.py`, evaluation module and LICENSE | Candidate retrieval separated from evaluation; Hamming BK-tree | Apache-2.0; Python code is a reference, not a Rust dependency. Recursive tree insertion needs care on degenerate data. |
| [ImageHash](https://github.com/JohannesBuchner/imagehash) | `imagehash/__init__.py`: phash, whash, crop_resistant_hash; LICENSE | Multiple complementary fingerprints; segment-level crop matching | BSD license in source; crop robustness is algorithm-specific, not universal. Depends on grayscale/resizing behavior; requires independent fixtures. |
| [img_hash](https://github.com/abonander/img_hash) | Cargo manifest; repository structure | Small Rust hash-library dependency candidate | MIT OR Apache-2.0; algorithm and current-toolchain compatibility still need execution checks before selection. |
| [OpenCV contrib](https://github.com/opencv/opencv_contrib) | `modules/img_hash/src/phash.cpp`, module test inventory, LICENSE | Independent hash oracle and comparison of complementary algorithms | Apache-2.0; useful as test oracle, heavy as runtime dependency. Its pHash uses mean after zeroing DC, while ImageHash uses median including DC. Hashes are not interchangeable. |

## Selected first implementation

`rrrah-dedup::HammingIndex` independently implements BK-tree candidate search.
It retains multiple ids for identical fingerprints, handles empty input, uses
iterative insertion/search and exact distances for triangle-inequality pruning.
Results are deterministic by `(distance, id)`. It makes no claim about image
identity or hash quality. A seeded corpus compares indexed retrieval against
an independent exhaustive search across radii including 0 and 64.

This selection adopts a useful architecture from Czkawka/imagededup. No upstream
source code has been copied. It is not a measured claim that BK-tree is always
the fastest index: large radii and adversarial distributions can approach a scan.

## Remaining selection and implementation

1. Exact stage: size grouping, small sampled prehash only as a filter, full
   streaming digest, byte confirmation, file identity, mutation diagnostics.
2. Pixel stage: canonical color/orientation/alpha normalization using rrrah's
   existing RAW/raster pipeline. Compare all selected frames for animations.
3. Visual stage: complementary global fingerprints, transform variants and
   local crop evidence; keep pairwise scores separate from exact groups.
4. Persistent cache: version algorithm/normalization parameters and invalidate
   changed files. Measure hashing, decode memory, candidate recall and latency.
5. Selection gate: positive and negative fixtures for every row of
   `docs/DUPLICATE_LIBRARY_CONTRACT.md`, including solid colors, unrelated scenes,
   burst frames, rotation, crop, watermark, HDR, RAW and multipage inputs.

The five shortlisted projects above have different depths of review. Neither
the registry nor a project star count proves full coverage or algorithm quality.
More source review and cross-implementation tests are required before the
library's complete coverage can be claimed.
