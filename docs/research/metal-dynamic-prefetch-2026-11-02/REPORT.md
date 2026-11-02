# Changing prefetch windows under Metal memory pressure — 2026-11-02

The real-worker mixed PPM/TGA/BMP soak now switches its request window every
16 selections between behind/ahead `(0,0)`, `(3,0)`, `(0,3)`, `(1,2)` and `(2,5)`.
It independently scans gallery indices by signed travel distance to derive
expected neighbors; it no longer calls the production planner for its oracle.
Every observed neighbor path, generation, priority and retention is checked.

## Failure and production repair

The original run failed at selection32, the switch from `(3,0)` to `(0,3)`:
the first neighbor was refused despite an unprotected old neighbor being
replaceable. [Original log](test.log), [binary/configuration](run.json) and
[source pins](sources.json) preserve this negative evidence.

Two restored frames occupied the two-frame restore pool. The foreground frame
remained protected, but the old neighbor still owned its restored allocation
after its prefetch lease was released. The retained-prefetch path attempted the
new restore before releasing that old storage and then stopped on admission
failure. The production loader now retries a size-admissible restore after
releasing an unpinned LRU frame, only after count/byte retention preflight has
accepted the candidate. Protected current and nearest-neighbor leases remain
protected; cancellation and individually oversized allocations do not initiate
this reclaim loop. Retry stops when no unpinned victim remains.

A focused regression fills the restore pool with the selected and old-neighbor
frames. With the old lease held, the new neighbor is refused without a swap
read; after releasing it, the new neighbor restores with exact float bits while
the selected image remains a RAM hit. Final managed usage is zero.

## Fixed-run evidence

Actual Metal Apple M4 Max: **399 selections in 60.122 seconds**. Window selection
counts were `[80,80,80,80,79]`; 319 nearest neighbors retained and 921 farther
candidates refused. Disabled-window selections emitted no neighbor completion.
All selected prepared-image digests and 63×65 SDR/HDR readbacks match initial
same-decoder/renderer references. This proves regression transport consistency,
not independent color accuracy or physical HDR output.

The 64 authored 1024×1024 sources use the same pressure policy as the preceding
mixed run: 128 MiB root, fixed64 MiB pressure, RAM32 MiB/two entries/250 ms TTL,
swap64 MiB/three objects, restore pool32 MiB. Peak root128 MiB, settled maximum
96 MiB, final managed usage0. The required soak passed1/failed0/ignored0;
64 ordinary prefetch tests passed (22 manual ignored), and five ordinary worker
tests passed (six manual ignored). Production `cargo check --locked -p rrrah`,
formatting and `git diff --check` passed. The two ordinary test groups overlap
and are not reported as69 distinct tests.

Fixed evidence: [test.log](fixed/test.log), [prefetch tests](fixed/prefetch-tests.log),
[worker tests](fixed/ordinary-worker.log), [production check](fixed/production-check.log),
[configuration/binary hash](fixed/run.json), [source hashes](fixed/sources.json).
Reproduce with the current `rrrah` test executable:

```sh
RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 RRRAH_GPU_VENDOR=any RRRAH_SOAK_METAL=1 RRRAH_SOAK_PREFETCH=1 RRRAH_SOAK_MIXED=1 RRRAH_SOAK_DYNAMIC_WINDOW=1 RRRAH_SOAK_SECONDS=60 <rrrah test executable> folder_worker_qualification::real_worker_large_collection_pressure_soak --exact --ignored --nocapture --test-threads=1
```

This finite three-format run leaves complete100-format qualification, larger
prefetch windows, multi-hour workloads, physical slow disks, OS-wide pressure,
calibrated HDR and NVIDIA/CUDA qualification open.
