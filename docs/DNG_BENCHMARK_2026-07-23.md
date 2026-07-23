# Native DNG vs Rawler 0.7.2 — 2026-07-23

This run compares the previous Rawler-backed release binary with the clean-room
native DNG release binary. Each fixture received five warmups followed by 30
measured AB/BA pairs. Rrrah's persistent mosaic cache was disabled; operating
system file pages were warm.

The benchmark `wall` value is process launch through CLI exit. It includes
startup, source read, decode, metadata adaptation and inspect output, but no UI
or GPU work. The telemetry window's `TOTAL WALL TIME` is a different boundary:
request enqueue through the first successful present request, excluding GPU
completion and scanout.

| Fixture | Rawler decode p50/p95 | Native decode p50/p95 | Rawler wall p50 | Native wall p50 | Rawler/native RSS p50 |
|---|---:|---:|---:|---:|---:|
| Canon A410, packed 10-bit | 26.15 / 28.29 ms | 7.37 / 7.71 ms | 37.44 ms | 17.58 ms | 49.96 / 18.33 MiB |
| Canon 5D III, lossless JPEG | 43.49 / 45.61 ms | 19.81 / 20.22 ms | 54.15 ms | 29.52 ms | 45.39 / 15.38 MiB |
| Leica M8, 8-bit + linearization | 31.24 / 34.40 ms | 10.87 / 12.99 ms | 42.14 ms | 20.73 ms | 69.45 / 37.73 MiB |

Native was faster in all 90 measured pairs. Against the previous binary, median
decode time fell by 54–72%, process wall time fell by 45–53%, and peak RSS fell
by 46–66% across these fixtures.

## Correctness gate

- Canon packed 10-bit and Canon lossless-JPEG mosaics exactly match the previous
  library's complete LE-u16 BLAKE3 hashes:
  `0981694d0816951e7973a76550ffd81f77f4b90ecda6d5593e2a3b53e8e84eac`
  and
  `8d646e9ec3324c8e1b7643795322fd702e6b3482b2bdfb7781398df89333dbb8`.
- Leica intentionally does not match Rawler's dithered output. Native performs
  the exact `LinearizationTable[raw]` mapping required by the
  [Adobe DNG 1.7.1 specification](https://helpx.adobe.com/content/dam/help/en/camera-raw/digital-negative/jcr_content/root/content/flex/items/position/position-par/download_section_733958301/download-1/DNG_Spec_1_7_1_0.pdf);
  its complete mosaic hash is
  `e90b617fe1e75edcd85a3510b5ca7f3016ce3bee267a1fae66c751aa1b24c117`.
- A DQT-bearing Blackmagic/lossy-JPEG fixture is rejected explicitly rather
  than entering the SOF3 lossless path.
- The opt-in regression command is:

  ```bash
  RRRAH_DNG_REGRESSION_DIR=/path/to/staged-fixtures \
    cargo test -p rrrah-decode --lib dng::fixture_regression -- --ignored
  ```

The fixture catalog is [raw.pixls.us](https://raw.pixls.us/). The measured
machine was arm64 macOS with ten logical CPUs. Initial load average was 7.15, so
absolute times are local exploratory measurements; AB/BA alternation reduces,
but does not eliminate, thermal and scheduler drift.
