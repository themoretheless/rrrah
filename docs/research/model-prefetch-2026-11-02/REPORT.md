# Model prefetch lease and memory recovery — 2026-11-02

Retained model prefetch now hands its first RAM lease to the worker. Previously it cloned the resident model and looked it up again, allowing TTL expiry between the two operations. The regression checks both lease modes across TTL expiry for authored STL, OFF, OBJ and PLY fixtures; protected allocations stay resident and final managed usage returns to zero.

Retained swap restore now reclaims an unpinned LRU neighbor on memory refusal, then waits for at most one cancellable spill fence before retrying. Cold native decode uses the same recovery policy, including typed source-headroom failures. Cancellation, pinned neighbors and individually oversized requests remain terminal for that attempt; no speculative native fallback is introduced for transient swap pressure.

## Evidence

- [Ordinary viewer suite](viewer.log): 354 passed, 105 ignored, zero failures.
- The full viewer suite includes retained-slot replacement, actual cold native decode and warm TTL regressions for all four fixture types.
- [Required Metal replacement](metal-replacement.log): Apple M4 Max, one passed, zero ignored. Two protected restore allocations refuse a third without a read; releasing the old neighbor permits replacement (reads 2 then 3). Selected model remains resident; restored payload and SDR/HDR output match; final managed bytes zero.
- [Required Metal pressure](metal-pressure.log): one passed, zero ignored. All four fixture types preserve selected geometry and refuse speculative native decode under transient restore pressure, then restore exactly on retry.
- [Production check](check.log), [format check](fmt.log): pass. Existing vendored jxl-color lifetime warning remains.

The cold regression failed before recovery on STL: output reservation 108 bytes was refused with used=338, limit=338, although the old unpinned model could be reclaimed. [Original failure](cold-before.log) and [reverse patch](cold-before.patch) retain that counterexample against the original repair source hash in [cold-repair-sources.json](cold-repair-sources.json). The repaired test measures each fixture's actual native working peak and retained geometry, sets root to their sum, and requires exactly two decode attempts while selected geometry survives. Keys are synthetic distinct identities over the same fixture source, not four physical cold files.

GPU comparisons use the same renderer for native-reference and restored payloads: they prove cache/swap equivalence, not independent rendering correctness or physical HDR presentation. These focused fixtures do not qualify all variants of their formats, large 3D collections, NVIDIA, or RAW cold/swap paths. The separate three-hour mixed-raster Metal worker remains pending; this report does not claim its completion.

## Shared OBJ metadata quota correction — 2026-11-03

A128-triangle authored OBJ with one1024-byte object name, one1024-byte material and two512-byte groups exposed duplicate admission charges. The old cache weight was421576 bytes, while an independent pointer-set census of retained vector/string capacities and the mesh struct gave25336. The same shared Arc payload was charged for every face in the native parser, capacity/lower-bound estimator and swap reuse branches. The native25336-byte quota therefore failed with requested3120, used22888, limit25336: [original failure](metadata-before.log).

The common pointer-identity rules now count adjacent object/group/material allocation runs once. Parser state/geometry, cache weight and swap-reader reuse use consistent rules. Cloning an already-retained Arc does not allocate another payload or consume duplicate credit. Equal text in distinct allocations remains fully charged. Nonadjacent reused pointers remain conservatively counted because RROBJ001 only preserves adjacent sharing and recreates those later runs during restore. No runtime pointer set or additional heap index was added; the pointer sets exist only in the independent test census. Serialized format/geometry/material declarations and configured limits are unchanged.

[Mesh suite](mesh.log):61 passed/zero ignored/zero failed. The128-face native geometry case fits an exact25336-byte retained/peak budget. Restore retains22312 bytes and needs23336 peak bytes, including the1024-byte temporary material String-to-Arc overlap. Both native and restore one-byte-short budgets refuse and release all credit. The managed native owner survives cloning until the last owner drops. Serialized bytes and adjacent Arc identity are preserved. Separate equal-value/distinct-allocation and nonadjacent-sharing cases protect against an unsafe global/content deduplication of restoration weight.

These quotas cover the declared mesh/vector/string accounting contract, excluding allocator and Arc headers. The authored borrowed source and output comparison Vec are outside those geometry budgets. The source allocations were already shared before the repair: this frees incorrectly charged admission credit; no reduction in physical heap use or measured navigation time is inferred.

Current [viewer suite](viewer.log):354 passed/105 ignored; [production check](check.log), formatting and whitespace pass. Both required M4 Max tests above pass1/zero ignored across STL/OFF/OBJ/PLY: protected pressure refuses speculative native fallback, old-neighbor release permits replacement, payload and SDR/HDR match, selected remains resident, final0. Those GPU checks use the existing four file fixtures; the128-face census case itself is CPU-only. Same-renderer reference scope is unchanged.

Current positive [source/binary pins](sources.json) also record the three before-source hashes. Zero-context reverse patches [parser](metadata-before-obj.patch), [weight estimator](metadata-before-model.patch) and [reader](metadata-before-swap.patch) reconstruct the metadata counterexample with `git apply --unidiff-zero`. The earlier cold-load patch retains its separate original base pins. Superseded positive prefetch output is replaced by the full current viewer suite. Broad model variants/material rendering, GLB/glTF, the100-format objective and hardware qualification remain open.
