# Current Metal compute, full sensor and cache transport

Seven compute_readback tests explicitly including both ignored full-resolution cases pass on Metal AppleM4Max; zero ignored or failed. The managed full-resolution case checks all25494560pixels at6188x4120 with nonperiodic green and independent alpha across7dispatch windows. Both array and interleaved APIs match exact authored exposure results. CPU/GPU pressure refusal and cancellation after admission retain the source and release temporary credit. GPU managed peak1223738992bytes; CPU managed peak815825920bytes; finalused0 including last-clone ownership.

The other full-resolution case samples only three output pixels while timing completed submissions. Its timing observations are retained in the raw log but are not an optimization, paired benchmark or every-pixel proof. Every-pixel correctness comes from the separate managed case. Smaller cases cover signed finite f32 boundary values, HDR/alpha/tail, cancellation/retry and resident ownership.

Both explicitly ignored compute_cache_swap cases pass:164x50 source with128x96 output and4000x3000 source/fulloutput. A CPU-authored exposure oracle checks all component bits, signed finite binary16 HDR, RAM pinning while limits tighten, one swap write/read, full-budget refusal, admitted cancellation, exact SDR/HDR retry and finalCPU/GPU/queue0. The existing strict compute_cache_evidence validator accepts both logs.

This proves the authored linear-exposure compute and cache transport scenarios, not all RAW demosaic/camera color, live GUI, physical HDR calibration, every image format or NVIDIA. Source hashes and raw logs are retained. Timings are single-run observations and do not establish performance improvements.
