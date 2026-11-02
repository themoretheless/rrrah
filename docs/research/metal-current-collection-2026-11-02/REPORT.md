# Current-source Metal collection regression — 2026-11-02

Production sources at `180d10fc406aa1ed1f7e77607b5b1078e4b5c707` were rebuilt with
`cargo test --locked -p rrrah --test ttl_swap_metal --no-run --message-format=json`.
The required-device test ran on Metal Apple M4 Max with no optional GPU skip.

Eight authored 4000×3000 RGBA32F images (192,000,000 bytes each) completed
161 restoration cycles in 120.022 seconds. Every cycle checked all native float
bits, exact full-resolution SDR output and exact 4000×3000 HDR half-float output
against the initial per-image frames. These GPU references are regression
references from the same renderer, not independent color or display oracles.

Each cycle refused restoration while the managed root was exhausted, checked
immediate and admitted parallel-batch cancellation, retried successfully, tested
visible-owner retention and zero-TTL eviction, and returned root and queue usage
to zero. The run recorded 161 parallel restores, 161 mid-batch cancellations,
161 reads and eight writes; final managed root and queue usage were zero.
The monitor sampled a maximum process RSS of 2,090,024,960 bytes. Process RSS
includes allocations outside the explicitly managed budgets.

The test and independent workload-log validator both passed: one test passed,
zero failed, zero ignored. Source and binary hashes, allocator environment,
sampling and terminal status are preserved in [state.json](state.json),
[sources.json](sources.json), [rss.jsonl](rss.jsonl) and [test.log](test.log).

Reproduce with a newly built test executable and fresh output/scratch directories:

```sh
python3 scripts/run-metal-collection-soak.py --binary <ttl_swap_metal executable> --output <new evidence directory> --scratch <new scratch directory> --seconds 120 --sample-seconds 5
```

This finite current-source regression does not close heterogeneous multi-hour
navigation, physical slow disks, OS memory pressure, calibrated physical HDR,
NVIDIA/CUDA, or complete qualification of all 100 formats.
