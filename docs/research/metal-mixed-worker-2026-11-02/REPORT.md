# Mixed-format real-worker pressure and prefetch — 2026-11-02

The existing real foreground-worker soak now supports `RRRAH_SOAK_MIXED=1`:
64 independently authored uncompressed PPM/TGA/BMP files, each 1024×1024 with
an asymmetric RGB pattern. The TGA and BMP writers use top-left rows and BGR
storage; BMP includes row padding. The ordinary all-PPM mode remains available.
This is a test-only extension; production behavior is unchanged.

Required Metal Apple M4 Max run: **292 selections in 60.084 seconds**, with
forward/backward steps and long jumps. Prefetch is behind=1/ahead=2. Every selected
prepared-image digest and 63×65 SDR/HDR readback matches that file's initial
same-decoder/renderer baseline. These baselines prove transport consistency,
not independent color accuracy or physical HDR calibration.

Limits: shared root 128 MiB, fixed pressure 64 MiB, RAM cache 32 MiB/two entries/
250 ms TTL, disk swap 64 MiB/three objects, restore pool 32 MiB. Peak root usage
was exactly 128 MiB; largest settled usage was 96 MiB including fixed pressure.
The nearest planned neighbor was retained for all 292 selections; 557 farther
candidates were refused with occupied protected slots. Every completion's path,
generation and priority were checked. Swap bounds held and final managed usage
was zero.

The required soak passed one test with zero ignored. Five ordinary worker tests
also passed (six manual tests ignored). Formatting and `git diff --check` passed.
Original evidence: [test.log](test.log), [ordinary-worker.log](ordinary-worker.log),
[run configuration and binary hash](run.json), [source hashes](sources.json).

Reproduce after `cargo test --locked -p rrrah --bin rrrah --no-run`:

```sh
RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 RRRAH_GPU_VENDOR=any RRRAH_SOAK_METAL=1 RRRAH_SOAK_PREFETCH=1 RRRAH_SOAK_MIXED=1 RRRAH_SOAK_SECONDS=60 <rrrah test executable> folder_worker_qualification::real_worker_large_collection_pressure_soak --exact --ignored --nocapture --test-threads=1
```

This finite three-format run leaves other formats, varied prefetch counts,
multi-hour workloads, physical slow disks, OS-wide pressure, calibrated HDR and
NVIDIA/CUDA qualification open.
