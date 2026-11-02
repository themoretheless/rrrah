# Model prefetch lease and memory recovery — 2026-11-02

Retained model prefetch now hands its first RAM lease to the worker. Previously it cloned the resident model and looked it up again, allowing TTL expiry between the two operations. The regression checks both lease modes across TTL expiry for authored STL, OFF, OBJ and PLY fixtures; protected allocations stay resident and final managed usage returns to zero.

Retained swap restore now reclaims an unpinned LRU neighbor on memory refusal, then waits for at most one cancellable spill fence before retrying. Cold native decode uses the same recovery policy, including typed source-headroom failures. Cancellation, pinned neighbors and individually oversized requests remain terminal for that attempt; no speculative native fallback is introduced for transient swap pressure.

## Evidence

- [Ordinary viewer suite](viewer.log): 354 passed, 100 ignored, zero failures.
- [Prefetch suite](prefetch.log): 69 passed, 24 ignored, zero failures, including actual cold native decode and warm TTL regressions for all four fixtures.
- [Required Metal replacement](metal-replacement.log): Apple M4 Max, one passed, zero ignored. Two protected restore allocations refuse a third without a read; releasing the old neighbor permits replacement (reads 2 then 3). Selected model remains resident; restored payload and SDR/HDR output match; final managed bytes zero.
- [Required Metal pressure](metal-pressure.log): one passed, zero ignored. All four fixture types preserve selected geometry and refuse speculative native decode under transient restore pressure, then restore exactly on retry.
- [Production check](check.log), [format check](fmt.log): pass. Existing vendored jxl-color lifetime warning remains.

The cold regression failed before recovery on STL: output reservation 108 bytes was refused with used=338, limit=338, although the old unpinned model could be reclaimed. [Original failure](cold-before.log) and [reverse patch](cold-before.patch) retain that counterexample against the source hash in [sources.json](sources.json). The repaired test measures each fixture's actual native working peak and retained geometry, sets root to their sum, and requires exactly two decode attempts while selected geometry survives. Keys are synthetic distinct identities over the same fixture source, not four physical cold files.

GPU comparisons use the same renderer for native-reference and restored payloads: they prove cache/swap equivalence, not independent rendering correctness or physical HDR presentation. These focused fixtures do not qualify all variants of their formats, large 3D collections, NVIDIA, or RAW cold/swap paths. The separate three-hour mixed-raster Metal worker remains pending; this report does not claim its completion.
