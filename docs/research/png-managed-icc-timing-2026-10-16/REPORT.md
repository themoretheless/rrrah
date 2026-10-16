# Paired PNG ICC route timing

Same optimized standalone process compares the prior image PNG metadata route (collect ICC, clone getter, decode pixels) with the current native managed ICC inflater + image pixel decoder skipping ICC. Both use the same linked image library, immutable source, preallocated caller output, and return the retained profile. The native route includes state/profile budget admission and cancellation checks. Warm-up:5iterations of both routes. Samples:40pairs per source, alternating old/new and new/old order. Every retained profile byte and output pixel is checked outside the timed interval; all native root credit is0 after each drop.

|Source/profile|Old median|New median|Median paired new/old|95% bootstrap interval|New faster pairs|
|---|---:|---:|---:|---:|---:|
|Pinned original16x16 PNG,588-byte ICC|8.083µs|7.125µs|0.8809|0.8728–0.8857|40/40|
|Synthetic16x16 PNG,4MiB padded ICC|0.4985ms|0.4135ms|0.8358|0.8251–0.8502|39/40|

Thus the native route's extra managed admission did not create a measured regression in these two local controls. Relative improvements are about12% and16% for this component, not whole application loading. Raw per-pair JSONL, deterministic bootstrap summary, generated-source script, optimized build command and library/source hashes are preserved.

Probe code includes the current production native ICC module at opt-level3. Vendored image dependency uses the optimized dependency build; production debug workspace crates otherwise use opt-level1. This is optimized component timing, not a packaged viewer/release benchmark. It excludes source file I/O, native declaration validation common to both routes, color-management/display conversion, retained-owner destruction, GUI/GPU presentation and RSS. Host background load was not controlled. The4MiB profile is zero-padded valid-tag synthetic data, not a representative high-entropy CLUT corpus. Bootstrap reflects these observed pairs, not cross-device confidence. No universal format/hardware speedup is claimed.

No production code changed in this measurement follow-up. Functional/admission/cancellation gates remain documented in `../png-managed-icc-inflate-2026-10-16/REPORT.md`. Full100-format, remaining codec-private workspace, NVIDIA/CUDA and calibrated physical HDR requirements remain open.
