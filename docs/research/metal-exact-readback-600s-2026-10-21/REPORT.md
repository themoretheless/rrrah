# Full-frame Metal collection: 600-second pass

Current frozen candidate passed3088cycles in600.121seconds on Metal AppleM4Max. Each cycle verifies complete4000x3000 SDR and binary16 HDR output against its pre-spill reference, every restored float bit, full managed-root refusal, admitted parallel scratch cancellation and exact retry, visible TTL protection and expiry spill. The eight fixed seeded frames produced8initial writes and3088reads with no redundant writes. Final root/queue/resident credit is0; scratch is empty.

All121five-second RSS samples were available. Sampled peak 2100854784bytes (1.957GiB), below the unchanged4GiB guard. This is a sampled maximum, not continuous peak or indefinite stability proof. Inherited allocator configuration is recorded: MallocNanoZone=0 (also present in baseline); no MallocLargeCache override. Binary and all recorded source hashes remain unchanged after the run.

The sole harness change reserves the exact HDR output vector element count before copying rows, replacing geometric flat_map collection growth. No renderer, budget, source pixels, comparisons or pixel coverage were weakened. Original failed baseline, failed explicit-destruction diagnostic, storage-only isolation and allocator diagnostic remain adjacent reports. This qualifies the stated headless transport/storage/GPU scenario, not a production viewer memory fix, live GUI, physical HDR luminance/calibration, a physical slow drive, all100formats, or NVIDIA.

Companion checks:44gallery tests passed/16manual or fixture tests ignored; the frozen candidate also passed the small HDR TTL/swap control on M4Max (log in the60-second pilot directory). No speed claim follows from differing cycle counts; these runs were not paired throughput benchmarks.

## Current-build two-minute requalification — 2026-10-24

Freshly built `ttl_swap_metal` ran the retained full-resolution workload under the existing monitor. Eight authored 4000×3000 RGBA32Float sources, one-frame RAM and queue credits, disk swap, CPU pressure, cancellation and retry, full-resolution SDR and float16 HDR readback remain enabled. The frozen binary and covered source hashes are recorded in `current-120s-2026-10-24/state.json` and `sources.json`.

Terminal monitor status passed, workload evidence valid, exit 0: 555 cycles over 120.090 s workload time, 555 accelerated restores, 555 cancellations after scratch admission, 555 exact full-frame SDR and 555 exact float16 HDR readbacks. Final managed root=0 and queue=0; swap reads=555/writes=8. Test: 1 passed/0 failed/0 ignored, 127.97 s including fixture setup.

Sampled RSS peak 2,283,913,216 bytes (2178.1 MiB) over 27 samples, below the 4 GiB guard. Sampling does not establish an absolute unsampled peak or long-term leak freedom. Full raw progress and RSS samples are retained in the run directory. This is current-binary functional requalification, not a speed benchmark or replacement for the older 600-second accepted run above. Sources here are authored decoded frames, not an eight-file external codec collection; physical HDR, real slow storage and NVIDIA remain outside this gate.
