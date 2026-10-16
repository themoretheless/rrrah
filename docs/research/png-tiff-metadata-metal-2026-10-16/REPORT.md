# PNG/TIFF metadata cache and Metal transport

2026-10-16. Stable toolchain, locked/offline app binary tests. Actual adapter: Metal Apple M4 Max.

Seven cases: pattern PNG, TIFF, indexed PNG, ICC PNG; oriented ICC TIFF; grayscale gamma-2 16-bit PNG with alpha and TIFF. Pixel oracles are existing producer fixture RGBA files, explicit TIFF orientation samples, and the grayscale fixture generator samples. Color metadata is read through the current decoder and CMM preparation is shared. This is transport/rendering equivalence, not independent ICC color accuracy or full PNG/TIFF qualification.

Both CPU and explicitly ignored Metal tests passed. Exact prepared float pixels and selection metadata survive RAM and swap. Each test performs seven RAM hits while all remaining root budget is reserved, seven swap writes and seven reads, with zero errors/drops. Foreground DecodeGate is held during background restores, proving they bypass decoding. Metal performs 42 complete output comparisons: reference, decoded and restored frames in SDR and half-float. Accounted root memory returns to zero after last owners drop.

Root budget 131072 bytes; RAM 8192/count1; swap 1048576/count2048; queue 65536/count4; restore 65536. Half-float readback does not prove physical HDR display calibration. NVIDIA remains deferred. Private codec allocations are not fully covered by this root budget assertion.

Commands:
```
CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah --bin rrrah --locked --offline png_tiff_metadata_ram_swap_and_blocked_prefetch -- --nocapture
RRRAH_GPU_BACKEND=metal CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah --bin rrrah --locked --offline png_tiff_metadata_ram_swap_and_metal -- --ignored --nocapture
```

Logs and production/test source SHA256 are adjacent. No broad workspace release gate was run for this change.
