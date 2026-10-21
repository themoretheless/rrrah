# Large allocator cache disabled — diagnostic only

The exact original failing binary (see binary_sha256 in state.json) passed the same full 4000x3000 SDR/HDR collection workload for 60 seconds with MallocLargeCache=0 inherited by the monitor and child. Result: 144 cycles in 60.012 seconds, sampled peak RSS 1808515072 bytes, exit0 and workload evidence valid. No renderer or test source change was used for this control. environment.json explicitly records the override; inherited MallocNanoZone=0 was also present in both baseline and controls.

The ordinary-allocator long gate remains failed. The override is an isolation diagnostic, not a product fix, default-environment pass, or performance comparison. Explicit readback destruction failed separately, and its vmmap shows substantial freed-but-resident allocator memory. Together these observations support allocator retention as a substantial contributor; they do not exclude other retention.

Apple source supports the diagnostic switch: https://github.com/apple-oss-distributions/libmalloc/blob/main/src/malloc.c (MallocLargeCache parses0/1 and sets large_cache_enabled). Local macOS26.7.2 implementation behavior is evidenced by the actual binary control, not inferred solely from upstream source. Next work should reduce allocation churn/reuse buffers or add measured explicit memory reclamation without hiding normal-environment RSS.
