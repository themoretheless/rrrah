# Extreme prefetch windows and controlled reversal — 2026-11-02

The current real-worker soak extends its dynamic matrix to seven behind/ahead
windows: `(0,0)`, `(3,0)`, `(0,3)`, `(1,2)`, `(2,5)`, `(0,usize::MAX)` and
`(usize::MAX,usize::MAX)`. Windows change every16 selections; the independent
signed-distance gallery scan still checks every observed path, generation,
priority and retention. Production code is unchanged from `816ce826e`.

On required Metal Apple M4 Max the 64-file PPM/TGA/BMP collection completed
**392 selections in60.035s**. Per-window selections were `[64,64,64,56,48,48,48]`.
All finite and maximal windows matched the available gallery neighbors without
extra completions or observed overflow.326 nearest neighbors were retained;
4684 farther candidates were refused at protected capacity. Disabled windows
and outward-only windows at a collection edge legitimately have no neighbor.

Every selected prepared-image digest and63×65 SDR/HDR frame matched initial
same-decoder/renderer references. Peak managed root128MiB, settled maximum96MiB
including64MiB fixed pressure, RAM cache32MiB/two entries/250ms TTL, swap64MiB/
three objects, restore pool32MiB; final managed usage0.

A separately controlled Metal reversal blocked old neighbor4 at the decode gate,
changed selection3→2, and verified cancellation with no extra pixel allocation.
The new foreground generation published before backward neighbors `[1,0,3]`;
CPU and Metal SDR/HDR references matched, final managed usage0. This specifically
tests cancellation while waiting for decode admission, not cancellation at an
arbitrary instruction inside restore-slot reclamation.

Both required Metal tests passed1/failed0/ignored0 individually.64 ordinary
prefetch tests passed,22 manual tests ignored. Formatting and diff whitespace
checks passed. Original logs and source/binary/configuration pins:
[window soak](window-soak.log), [reversal](cancel-reversal.log),
[ordinary tests](ordinary-prefetch.log), [run.json](run.json).

Reproduce after building the current `rrrah` test executable:

```sh
RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 RRRAH_GPU_VENDOR=any RRRAH_SOAK_METAL=1 RRRAH_SOAK_PREFETCH=1 RRRAH_SOAK_MIXED=1 RRRAH_SOAK_DYNAMIC_WINDOW=1 RRRAH_SOAK_SECONDS=60 <rrrah test executable> folder_worker_qualification::real_worker_large_collection_pressure_soak --exact --ignored --nocapture --test-threads=1
RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 RRRAH_GPU_VENDOR=any <rrrah test executable> folder_worker_qualification::real_worker_metal_direction_reversal_cancels_blocked_prefetch --exact --ignored --nocapture --test-threads=1
```

These finite64-file controls do not qualify arbitrary collection sizes, all100
formats, independent color accuracy, multi-hour/physical disk/OS pressure,
calibrated HDR or NVIDIA/CUDA.
