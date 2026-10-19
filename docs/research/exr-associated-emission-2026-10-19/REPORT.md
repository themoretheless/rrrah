# Associated EXR emission qualification — 2026-10-19

Qualified FLOAT linear RGB EXR now preserves alpha-associated emission (nonzero RGB at zero alpha) and values that cannot safely be unassociated. Ordinary EXR retains the straight-alpha contract. Association is restricted to float linear RGB and follows color conversion, RAM leases, cropped restore, CPU/swapped LOD and GPU upload.

Swap v6 records association; straight payloads retain existing versions. Invalid associated descriptors are rejected before allocation. The independent OpenEXR fixture retains signed/HDR samples exactly. Actual Apple M4 Max Metal checks cover single/array textures, resident compute, association transitions, RGBA16Float compositing, LOD, RAM pressure, swap refusal/retry and final zero managed usage.

Prior validation logs record 802 ordinary passing tests, 93 ignored, plus two cache integration cases. Eight hardware integration cases passed in aggregate: the broad run passed seven and required a separate rerun of the AI case with its missing corpus environment configured. This does not establish AI appearance parity.

The mixed collection rerun accepts 962 of 1024 files. The former emission refusal is now a missing-source-color refusal: 47 source-color, 13 EXR color constraints and two nonconforming AVIF pasp. Swap errors are zero; two bounded queue drops occurred. Managed root returns to zero. Artificial managed-credit pressure is not physical RAM exhaustion.

Physical HDR display, NVIDIA/CUDA, live GUI endurance, full EXR coverage and complete 100-format qualification remain open. No performance improvement is claimed for this change.

Publication gate: fresh 802 ordinary tests, two associated-cache integration tests and one actual Metal EXR integration test passed. Formatting and whitespace checks passed. Initial validation invocations used an incorrect package name / backend environment variable; corrected invocations above passed.
