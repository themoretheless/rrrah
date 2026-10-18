# Streaming Tensor planning optimization

Removed redundant triangle capacity counting for streaming/unmanaged sinks. Managed owned meshes still count and reserve complete output globally; emission triangle limits remain enforced. Geometry, tolerances, order and memory limits are unchanged.

Five-second baseline CPU sample identified interior_count among hot functions (222 of 3477 main-thread samples). Sampling is diagnostic, not a latency measurement or memory admission proof.

Six alternating baseline/candidate pairs, nine timed iterations after three warmups per process, dev optimized build, warm filesystem and shared host:

| Fixture | Baseline median p50 ms | Candidate median p50 ms | Median paired ratio |
|---|---:|---:|---:|
| Flat Tensor | 0.214605 | 0.202230 | 0.966969 |
| Parabolic Tensor | 18.608771 | 14.605917 | 0.784864 |
| VectorApple | 456.653230 | 420.146355 | 0.919874 |

Curved and VectorApple improve in all six pairs (~21.5% and ~8%). Flat results are noisy; no reliable flat speedup claimed. All prepared pixel hashes match across variants. Accounted peaks unchanged: 163148, 3632810 and 26534362 bytes respectively. Baseline is the previous accurate adaptive streaming renderer, not the old fixed grid. These are CPU load+prepare timings excluding startup, hashing, destruction and GPU; no cold disk, GUI, release, physical HDR or NVIDIA claim.

Raw timings, benchmark script, CPU sample and validation logs accompany this report. The benchmark script's generic scope label says encoder; the actual sole candidate source change is shading/adaptive.rs. Full format qualification and AI fidelity remain open.

Validation: vector185, decode216, app287, interpreter134, hayro312 passed (1134 total;109 ignored;0 failed). Selected actual Metal test additionally passed on Apple M4 Max,10readbacks, decode peak3632714bytes with8MiB root/final0. Workspace format and source whitespace checks pass. Preserved raw logs and patch artifacts contain original trailing/EOF whitespace; those artifact-only warnings are retained for fidelity. This is focused validation, not a fresh all-target/all-feature or strict Clippy gate.
