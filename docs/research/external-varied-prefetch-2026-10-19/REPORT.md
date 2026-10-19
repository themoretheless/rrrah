# External collection: asymmetric prefetch windows

The existing external collection qualification now supports RRRAH_COLLECTION_VARY_PREFETCH=1. Every source SHA256 was rechecked before interpreting the run (1024 checked, zero mismatches). Sources remain in place; the report retains hashes and metadata, not source media. This is the previously selected mixed collection with repository assets and frozen fixtures, not 1024 independent camera photographs.

Both navigation directions exercise each window: disabled (0/0), ahead-only (behind0/ahead3), behind-only (behind3/ahead0), mixed (behind1/ahead2). Each of eight direction/policy combinations runs 120 or 121 steps. Assertions independently check neighbour count and side bounds. A nearest successful prefetch must provide a lease and subsequent foreground cache lookup must succeed without invoking the decoder; selected-frame pin is then restored. Far admission remains bounded by two resident count slots.

1024 sources attempted; 962 pass strict-color display preparation and navigate. There are 720 nearest-prefetch navigation hits without decoding, 776 expected protected-count refusals for farther neighbours, 61 TTL restore/retry checks under deliberately full managed reservation, and 962 swap writes / 1332 reads. Swap errors and dropped writes are both zero in this run; disk storage ends at eight objects / 22,579,712 bytes. Final managed usage zero. Runtime 9.29 seconds excluding build.

Refusals remain 47 source-color interpretation, 13 EXR color constraints and two nonconforming AVIF pixel-aspect declarations. Same-decoder baseline hashes prove transport consistency, not independent format/color correctness. Peak 512 MiB includes artificial reservation-only pressure, not measured physical memory exhaustion; maximum observed pre-pressure usage 14,689,472 bytes.

This exercises shared application loader/cache helpers and navigation planning. It is not a live GUI event-loop or hot settings-update qualification, nor slow physical disk, NVIDIA or HDR display proof. No production logic changed. The prior fixed-window default is retained; the additional nearest-navigation cache-hit assertion also applies to default runs.

Reproduce with RRRAH_COLLECTION_VARY_PREFETCH=1, RRRAH_COLLECTION_MANIFEST pointing at the preserved external manifest and RRRAH_COLLECTION_REPORT at a result path, then cargo +stable test -p rrrah --bin rrrah external_mixed_collection_preserves_navigation_prefetch_ttl_swap_and_pressure --locked --offline -- --ignored --nocapture.
