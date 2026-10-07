# Native CUDA exposure foundation

Owned PTX kernel multiplies scene-linear RGB by `2^stops`, preserving signed
and HDR values and alpha. The Rust driver boundary dynamically loads NVIDIA's
CUDA Driver API on 64-bit Linux/Windows, with explicit errors and no CPU or
wgpu fallback. `CudaExposure::execute` returns a managed immutable CPU buffer.

CPU output and device input/output are admitted under independent budgets
before allocation. Device accounting excludes context, JIT and driver overhead.
Each call owns a context; submitted work is synchronized before readback and
the context is destroyed before releasing device credit. Cleanup failure
conservatively retains device credit and library ownership when resource
release cannot be established. Input upload borrows the source in at most 4 MiB synchronous transfers and checks
cancellation before each transfer. A cancelled upload does not launch the kernel.
Readback checks cancellation between 64 KiB transfers. Cancellation after
submission discards results after synchronization. Source CPU accounting belongs
to the caller. These byte bounds do not establish a time bound or measured
throughput on NVIDIA hardware.

This is a foundation, not a qualified viewer backend. It is connected to the
existing offscreen raster timing example via `--cuda-exposure`; it is not
connected to the interactive viewer, graphics interop or GPU texture cache. No
NVIDIA execution or performance result has been established on the macOS host.
PTX is JIT-loaded by a real NVIDIA driver; local Rust checks do not assemble it.
Independent offline assembly with NVIDIA ptxas 12.8.93 passed for `sm_52`,
`sm_61`, `sm_70`, `sm_75`, `sm_80`, `sm_86`, `sm_89`, `sm_90`, `sm_100`
and `sm_120`. This validates compiler acceptance, not execution or driver JIT.
The kernel declares PTX 6.0 and `sm_52`; compatibility is not assumed for every
driver/device. Context/module creation per call is not an optimized hot path.

The existing `raw_view_timing` example accepts this explicit mode:

```sh
RRRAH_GPU_VENDOR=nvidia RRRAH_GPU_BACKEND=vulkan RRRAH_VIEW_CPU_MB=512 RRRAH_CUDA_GPU_MB=512 RRRAH_VIEW_GPU_MB=256 cargo run --locked -p rrrah --example raw_view_timing -- --raster-view --cuda-exposure 1.25 SOURCE
```

Input must follow the existing raster route (including supported X3F files);
this does not develop Bayer mosaics. The total interval includes decode,
color preparation, CUDA context/JIT setup, host/device transfers, exposure,
host-mediated renderer upload and completed offscreen frame. Three warmup
and fifteen measured frames follow existing timing conventions. CUDA device
ordinal is zero; the graphics adapter is separately selected among NVIDIA
devices, with no claim of matching physical devices or zero-copy interop.
Interleaved managed output avoids a flattening copy; CUDA and renderer
device budgets remain independent. The existing non-CUDA path stays available.

Local checks:

```sh
cargo test --locked -p rrrah-cuda
cargo clippy --locked -p rrrah-cuda --all-targets --no-deps
cargo check --locked -p rrrah-cuda --all-targets --target x86_64-pc-windows-gnu
```

Required hardware qualification on 64-bit Linux/Windows with NVIDIA CUDA:

```sh
cargo test --locked -p rrrah-cuda nvidia_full_output_boundaries_hdr_alpha_and_managed_limits -- --ignored --nocapture
```

The hardware gate fails if the driver/device is absent. It checks every output
channel against independent CPU multiplication, preserves alpha bits, exercises
partial thread blocks, and checks managed lifetime/admission/cancellation.
An ignored hardware test is not evidence of CUDA correctness.

Offline kernel assembly can be repeated on Linux with:

```sh
python3 scripts/qualify-cuda-ptx.py --ptxas /path/to/ptxas --report /tmp/cuda-assembly.json
```

On a foreign host, `--docker-image` accepts a pinned Linux image and copies
only the compiler and owned PTX into an isolated, network-disabled container.
The gate removes its own container, records compiler/kernel/cubin hashes and
requires the expected entry compilation marker and ELF output for every target.

Primary API references:

- https://docs.nvidia.com/cuda/cuda-driver-api/cuda_driver_api/group__CUDA__CTX.html
- https://docs.nvidia.com/cuda/cuda-driver-api/cuda_driver_api/group__CUDA__EXEC.html
- https://docs.nvidia.com/cuda/parallel-thread-execution/
