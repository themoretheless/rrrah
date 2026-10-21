# Full-frame Metal collection: 600-second pass

Current frozen candidate passed3088cycles in600.121seconds on Metal AppleM4Max. Each cycle verifies complete4000x3000 SDR and binary16 HDR output against its pre-spill reference, every restored float bit, full managed-root refusal, admitted parallel scratch cancellation and exact retry, visible TTL protection and expiry spill. The eight fixed seeded frames produced8initial writes and3088reads with no redundant writes. Final root/queue/resident credit is0; scratch is empty.

All121five-second RSS samples were available. Sampled peak 2100854784bytes (1.957GiB), below the unchanged4GiB guard. This is a sampled maximum, not continuous peak or indefinite stability proof. Inherited allocator configuration is recorded: MallocNanoZone=0 (also present in baseline); no MallocLargeCache override. Binary and all recorded source hashes remain unchanged after the run.

The sole harness change reserves the exact HDR output vector element count before copying rows, replacing geometric flat_map collection growth. No renderer, budget, source pixels, comparisons or pixel coverage were weakened. Original failed baseline, failed explicit-destruction diagnostic, storage-only isolation and allocator diagnostic remain adjacent reports. This qualifies the stated headless transport/storage/GPU scenario, not a production viewer memory fix, live GUI, physical HDR luminance/calibration, a physical slow drive, all100formats, or NVIDIA.

Companion checks:44gallery tests passed/16manual or fixture tests ignored; the frozen candidate also passed the small HDR TTL/swap control on M4Max (log in the60-second pilot directory). No speed claim follows from differing cycle counts; these runs were not paired throughput benchmarks.
