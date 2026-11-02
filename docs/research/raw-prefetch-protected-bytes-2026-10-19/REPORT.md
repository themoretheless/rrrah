# RAW protected-byte lower-bound admission

The shared cache policy now exposes a non-evicting minimum-weight admission query. It combines count eligibility with capacity remaining after other protected entries. Existing-key replacement excludes that key's old protected weight. Summation saturates on overflow; minimum above capacity is rejected. The lease layer reconciles released leases first. The query does not renew TTL, change recency or remove membership; its protected-weight scan is O(number of protected entries), not an O(1) byte-accounting claim.

MosaicRamCache forwards this query. Retained RAW prefetch uses the conservative minimum two bytes: every valid nonempty u16 sensor needs at least one sample. This detects an exhausted protected byte budget before swap, persistent restore or native sensor decode, while leaving resident keys eligible. Recipe and fingerprint resolution still occur. It preserves the separate gallery disk warmer.

This is not a dimension/decoded-size estimator. If protected entries leave a small positive allowance that exceeds two bytes but cannot fit a particular sensor, decode may still be attempted. The ordinary final byte admission remains authoritative. No throughput, RSS or general avoidance of all oversized RAW decoding is claimed.

The byte-only regression fills an eight-byte cache with selected and leased mosaics, confirms count-only admission would pass while minimum-weight admission refuses, checks existing-key and oversized-bound behavior, then verifies release reconciliation without eviction.

The original CR3 navigation test now checks unchanged peak managed sensor usage for both count-limited and byte-only cases in both directions. Malformed neighbour, protected nearest, ready hit under full managed pressure, far admission after release and final zero all pass. This remains one original sensor source with temporary distinct timestamp/path keys, not multiple independent cameras. No source media is stored in the report.

151 cache tests and44 memory tests pass (195 combined,three ignored);291 ordinary app tests pass (82 ignored); the strengthened original-CR3 manual case passes. Formatting and whitespace checks pass.


## Transient swap restore refusal before a busy decode gate — 2026-10-24

The original CR3 regression reproduced the old background lookup erasing a retryable swap-memory failure into a cache miss, then waiting for a foreground-held native decode gate. The typed MosaicRamCache background API preserves the error. Retained RAW preload returns without gate acquisition or native fallback on temporary capacity refusal; cancellation wins, ready RAM stays eligible, and permanently too-small configured restore caps retain native fallback. Existing Option APIs remain compatible.

Actual tests/IMG_9043.CR3 contributes 25,494,560 sensor samples. With zero or one free managed byte and a held foreground ticket, preload returns within the three-second guard. After pressure release, restoration succeeds while that gate remains busy; all u16 sensor values (BLAKE3), metadata, visible pin and swap accounting agree. Ready RAM also works at a full root. A separate restore cap of one byte exercises native fallback and exact sensor equality. Every case releases the root to zero. Two keys share one original capture; this is not multiple-camera coverage.

CPU and required Metal tests pass (2 passed, 0 ignored); actual offscreen SDR 64x64 and float16 HDR 63x65 attachments agree before/after restore. This is not physical HDR-display, NVIDIA, general latency or independent camera-color qualification. Cache suite:155 passed/3 ignored; app:331 passed/88 ignored; production check succeeds. Existing explicit hardware/external ignores remain open.

Evidence: [reproduced failure](restore-pressure-red.log), [current CPU/Metal](restore-pressure-current.log), [current cache suite](libraries.log), [production check](restore-pressure-check.log), and the shared [current app suite](../prefetch-count-swap-hit-2026-10-16/app-current.log).

Reproduce: `RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 cargo +stable --config 'profile.test.package.rrrah.opt-level=0' test -p rrrah --bin rrrah raw_swap_admission_tests:: --locked --offline -- --include-ignored --nocapture --test-threads=1`.
