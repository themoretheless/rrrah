# Current WAL palette/cache path on native Metal

Actual adapter: **Metal Apple M4 Max** (`RRRAH_GPU_BACKEND=metal`).

The existing 2019-case legacy raster pipeline passed after external WAL dependency-key and allocation-free PCX-validation changes: 12114 SDR/half-float GPU readbacks, 2019 RAM hits under full root pressure, 2019 swap writes and reads, blocked DecodeGate during restoration, and final root use zero. This authored/producer corpus includes the two WAL fixtures and many other supported raster subsets. It does not qualify all variants of those formats.

A new explicitly ignored hardware regression reuses the same-size/same-mtime palette-change regression in two modes (RAM and spilled swap). The original independent fixture RGBA oracle supplies the black frame; the red oracle changes only pixels whose original WAL index is zero. Both old held and new decoded frames match their respective oracle through SDR and half-float shader readbacks, 16 actual/reference render outputs in total. The WAL source fingerprint stays unchanged, the cache key changes, no old palette swap entry is read, missing palette fails, held old pixels remain immutable, and final managed use is zero. Hardware test passed in 2.30 seconds.

This tests color-source cache coherence and GPU-buffer preservation, not physical HDR luminance, native window presentation, concurrent file writes, NVIDIA/CUDA, or full format qualification.

Ordinary viewer suite after adding the hardware regression: **285 passed, 80 ignored, zero failures** (`viewer.log`). The added hardware test was executed separately and passed, as recorded above.

## Cancellable source and palette identity — 2026-10-24

Raster cache identity now uses `SourceFingerprint::from_path_with_cancel` with the request generation token for both source and external palette fingerprints. The small WAL dependency-header read also checks cancellation before opening, each retry/read and dependency resolution; no cancelled partial hash is published as a key. Non-WAL and recognized raster bytes retain existing dependency semantics. All hash prefixes, component order, timestamps and sampled hashes remain unchanged.

A focused deterministic test derives the expected dependency hash from ordinary source/palette fingerprints, cancels at every reached checkpoint, verifies the exact callback stop, retries and checks that actual request-token invalidation prevents `raster_display_key` publication. Existing recognized-PNG-with-WAL-suffix/no-palette and palette invalidation controls remain enabled. Focused 1 passed/0 failed/0 ignored (`fingerprint-cancel-current.log`). Full app 325 passed/0 failed/84 ignored, 24.92 s; latest full log replaces its prior duplicate at `../prefetch-count-swap-hit-2026-10-16/app-current.log`. App-only test opt-level=0.

Required actual Apple M4 Max `wal_palette_change_ram_swap_and_metal`: 1 passed/0 failed/0 ignored, 4.17 s (`metal-current.log`). This retains the existing RAM/spilled-key palette replacement and SDR/HDR readback gate under current identity code. No broad color/format, physical HDR, NVIDIA or speed completion follows. Active blocked OS calls remain non-interruptible; cancellation is cooperative between requests.

## Cancellable foreground and thumbnail source stamps — 2026-10-24

`SourceStamp::try_read_with_cancel` now propagates cancellation through source and optional external-palette identity. Missing/non-cancellation palette I/O errors retain the previous optional-palette semantics, but Cancelled is returned explicitly rather than being silently converted to a partial `palette=None` stamp. Required foreground source admission and both pre/post-decode thumbnail stamps pass their generation tokens. Legacy non-cancellable `try_read` remains a compatibility wrapper; ordinary `read` semantics are retained for existing callers.

Three focused controls pass: prior non-file/identity and palette semantics plus the new all-reached-checkpoint cancellation matrix with palette present (65,537 bytes) or missing, unchanged successful identities and retry. Cancellation precedes a nonexistent source open. Raw focused output in `source-stamp-cancel-current.log`. Full app: 327 passed/0 failed/84 ignored, 21.70 s, app-only test opt-level=0. Latest shared full-app success log supersedes its prior duplicate at `../prefetch-count-swap-hit-2026-10-16/app-current.log`. This adds cooperative identity cancellation coverage, not interruption within blocked OS I/O, hardware requalification or a speed measurement. FolderScanner's separate legacy source-reader callback is not changed by this revision.

## Folder-scan source identity now observes cancellation — 2026-10-24

The separate FolderScanner callback identified as remaining in the prior revision now uses cancellable source stamps with its own scan generation. The production reader returns a typed result: ordinary source errors invalidate the cached thumbnail, Cancelled ends the current source-validation loop, and the existing generation/publication guard discards the stale scan result. Cover-change invalidation still bypasses unnecessary source reads. Legacy one-argument injected readers remain test-only compatibility helpers.

A real FolderScanner-thread control blocks the first source reader on its supplied cancellation predicate, submits a superseding scan, observes cancellation without manually releasing the reader, and requires only the latest unchanged-source result with no stale invalidation. This is a controlled read callback, not a stalled physical disk. Existing real filesystem/source/palette controls remain enabled. Folder-focused gate 17 passed/0 failed/1 ignored (`folder-cancel-current.log`); full app 328 passed/0 failed/84 ignored, 21.85 s, app-only test opt-level=0. Latest shared full-app successful output supersedes its prior duplicate in `../prefetch-count-swap-hit-2026-10-16/app-current.log`. Cooperative OS-call limitations remain unchanged.
