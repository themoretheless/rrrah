# rrrah-dedup

Native Rust primitives for exact-file discovery, decoded-pixel equality and local visual-copy search. These results have different meanings: byte-confirmed groups identify equal files; decoded-pixel equality applies to the selected decoding contract; local visual support can identify a crop or embedded copy without making the whole files equal.

## Features

| Cargo feature | Capability |
| --- | --- |
| Default (none) | Exact-file scanner, fingerprints and matching primitives |
| `raster` | Operations on admitted raster payloads |
| `decode` | File decoding, guarded visual search and collections |
| `loftr` | Cached native CPU LoFTR provider; requires compatible libtorch and model weights |

## Exact files

```rust
use rrrah_dedup::exact::{scan_verified, Options};
use std::path::PathBuf;

let options = Options {
    max_entries: 100_000,
    max_file_bytes: 512 * 1024 * 1024,
    ..Options::default()
};
let report = scan_verified(&[PathBuf::from("./images")], &options, || false);
if report.complete() {
    for group in &report.groups {
        println!("{} equal files, {} bytes each", group.paths.len(), group.bytes);
    }
}
// Surface report.issues and report.cancelled to the caller even with no groups.
// report.aliases describes multiple paths to the same physical file separately.
```

The scanner confirms full BLAKE3 hash equality with byte comparison. Traversal/resource omissions and changed files appear in diagnostics. Results are stable observations rather than an atomic filesystem snapshot. The scanner does not delete files.

## Decoded pixels and visual copies

With `decode`, use `scan::scan_equal_pixels` for direct selected-frame pixel comparison, or `pixel_index::scan_indexed_pixels` for indexed candidate discovery followed by pixel confirmation. A fingerprint or Hamming distance alone does not establish pixel or file equality. Decode requests must carry the appropriate frame/color/owner contract; unsupported sources and changes must remain visible to the caller.

For ordinary-bank search followed by calibrated exposure recovery, the explicit APIs are:

- `model_banks::search_model_banks_exposure_recovery_files_with`: one directed pair with a caller-supplied proposal provider.
- `model_banks::scan_model_banks_exposure_recovery_collection_with`: one canonical-ID direction per unordered collection pair.
- `model_banks::scan_model_banks_bidirectional_exposure_recovery_collection_with`: reverse the pair after an inconclusive forward result.
- `model_loftr::scan_loftr_model_banks_bidirectional_exposure_recovery_collection`: the same collection with one lazy CPU model cache (`loftr`).

Construct the bidirectional batch limits with `ModelBankExposureCollectionPolicy::for_bidirectional_pairs`; the one-direction constructor does not reserve reverse work. Both complete directed searches are admitted before input consumption, even when support permits skipping a suffix. Recipes and thresholds are explicit policy inputs, not a universal trained precision preset.

Collection reports retain every unordered pair, including inconclusive evidence. Forward transforms map lower-ID source to higher-ID target; reverse transforms map higher-ID source to lower-ID target. Keep this direction when using the geometry. Local visual support is nontransitive: do not infer duplicate groups by connecting supported pairs. Source/weight changes, owner/caller cancellation and provider errors discard the guarded collection rather than returning its supported prefix.

Keep the report alive while reading its borrowed pair slice; its managed storage credit is released on drop. `MemoryBudget` accounts for admitted payload/report storage. Provider tensors, model caches, snapshot metadata and other external allocations require separate process RSS qualification.

## Coverage evidence

The [coverage contract](../../docs/DUPLICATE_LIBRARY_CONTRACT.md) records exact scope, frozen test sources, CPU controls and remaining requirements. The [research registry](../../docs/research/dedup-500/REPOSITORIES.md) is a candidate inventory; it does not establish full source review of all 500 repositories. Full corpus recall, held-out difficult negatives, format/platform coverage and total resource bounds remain separate acceptance requirements.
