# Source-owned spatial patch replay

Native spatial replay now partitions Tensor patch geometry by source UV priority before composition. Cached geometry is restricted to its exact pixel domain; background and luminosity-mask replay have regression controls. Admissions and cancellation release retained credits atomically. Ordinary spatial integral accumulation uses compensated sums.

Exact orientation falls back to fixed-size integer arithmetic for ambiguous floating-point determinants. Independent Python Fraction controls cover 260 cases and 780 permutations, including subnormal, huge and signed-zero coordinates. The generator is scripts/generate-orientation-controls.py; fixture is tests/fixtures/geometry/orientation-sign-cases.rs.

Validation (stable, offline, locked): vector 212 passed / 11 ignored; native 319 passed / 13 ignored; interpreter 140 passed / 1 ignored. Raw logs retained here. Rejected pilots are retained separately from successful final logs.

Synthetic one-pixel cache control: 12 alternating pairs, 32 replays per leg. Median paired time change -27.80%; cached faster in 11/12 pairs. Preparation 8334 ns. This already-parsed RGB control does not measure disk, startup, large pages, GUI, GPU or physical HDR, and the first pair regressed. No broad speed claim.

Native Coons capture, arbitrary asymmetric compatibility folds, full AI fidelity and broader format qualification remain open. NVIDIA and physical HDR remain unqualified. Optional diagnostic and certificate modes are not comprehensively qualified by these controls.
