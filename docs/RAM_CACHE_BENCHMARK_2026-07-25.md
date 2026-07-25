# Mosaic RAM cache vs disk cache routes — 2026-07-25

This run measures the two cache tiers added in front of decode: the in-RAM
`MosaicRamCache` (byte-weighted LRU with visible-frame pinning) and the
existing `DiskMosaicCache` route it fronts. The benchmark is
`crates/rrrah-cache/benches/mosaic_routes.rs`:

```bash
cargo bench -p rrrah-cache --bench mosaic_routes
```

The benchmark is informational by default; there is no wall-clock gate.
Workload controls are `RRRAH_MOSAIC_BENCH_FRAMES`, `RRRAH_MOSAIC_BENCH_SAMPLES`
and `RRRAH_MOSAIC_BENCH_NAV_STEPS`.

## Methodology

Each synthetic frame is a valid 6000x4000, 14-bit, 1-component
`DecodedMosaic` (45.78 MiB of u16 pixels, xorshift-filled) stored through the
production `DiskMosaicCache::store`. The disk route replays the production
sequence per frame: `SourceFingerprint::from_path` (8 MiB synthetic source),
`CacheKey::for_mosaic_recipe`, then `DiskMosaicCache::load` with full BLAKE3
payload verification. The RAM route replays one loader-thread navigation step:
promoting `get` plus the visible-frame pin handoff (`mark_visible`). Thirty
measured samples follow an explicit warm-up; numbers below are the final run,
with cross-run drift noted where it matters.

Cold decode is deliberately **not** benchmarked: this workspace has no
synthetic RAW encoder, and a constant stand-in would misrepresent
entropy-decode cost. Real decode timings remain in
`docs/DNG_BENCHMARK_2026-07-23.md` (7–20 ms per fixture there).

## Results

| Route / scenario | p50 | p95 | best |
|---|---:|---:|---:|
| RAM hit (get + pin handoff), per frame | 190 ns | 307 ns | 182 ns |
| Disk hit (fingerprint + load + BLAKE3 verify), per 45.8 MiB frame | 63.4 ms | 91.9 ms | 47.5 ms |
| Navigation sweep (200 steps, 8 frames), RAM enabled | 0.20 µs/step | 0.22 µs/step | — |
| Navigation sweep, RAM disabled (disk-only) | 53.7 ms/step | 55.1 ms/step | — |
| Eviction storm: admission insert (budget = 3 frames, 64 rounds) | 0.25 µs | 0.29 µs | — |
| Eviction storm: pinned visible-frame lookup during pressure | 83 ns | 125 ns | — |

Navigation speedup from the RAM tier at p50: ~2.7×10⁵ per frame. Across four
full runs on a loaded machine the disk route p50 drifted between 63 ms and
118 ms and the RAM route between 190 ns and 411 ns; the ratio never dropped
below five orders of magnitude.

## Eviction / pin protection

With a budget holding exactly three frames and the visible frame pinned, 64
oversized admissions (each forcing an eviction) were issued. The pinned frame
survived every round (`visible_survived=true`), resident bytes never exceeded
budget, and lookups of the visible frame stayed at ~83 ns p50 / 125 ns p95
throughout. Admissions of over-budget work while all resident bytes are
pinned are rejected rather than displacing the visible frame (covered by unit
tests in `crates/rrrah-cache/src/ram.rs`).

## Interpretation

- A RAM hit is a HashMap lookup, an `Arc` clone and a pin-set update: hundreds
  of nanoseconds. The disk route is dominated by reading, verifying and
  reassembling ~46 MiB, at roughly 0.7 GiB/s effective on this machine. For
  back/forward browsing — the exact pattern the direction-aware prefetch
  window warms — the RAM tier turns a ~54–64 ms disk read into a sub-microsecond
  step, well below any frame-time budget.
- Pinning costs nothing measurable at lookup time (~0.1–0.3 µs) and provably
  protects the on-screen frame under budget pressure.
- Eviction is an O(resident entries) scan per admission. At 2 GiB of 24 MP
  frames that is ≤ ~45 entries, so the scan is noise; it would need
  re-examination only if entry sizes shrink by orders of magnitude (e.g. tile
  caches).

## Representativeness and caveats

- Mosaics are synthetic but structurally real: valid `DecodedMosaic` values
  stored and loaded through the production container/BLAKE3 path at a
  realistic 24 MP size. Pixel *content* (xorshift noise vs demosaiced sensor
  data) does not affect these routes.
- The disk route's 8 MiB synthetic source slightly under-represents
  fingerprint sampling of a real ~30 MB CR3; that cost is small next to the
  46 MiB mosaic load and does not change the conclusion.
- OS page cache was warm; no attempt was made to flush it (see
  `docs/BENCHMARKS.md` on why unprivileged cold-cache runs are unreliable on
  macOS).
- The navigation speedup compares warm RAM against warm disk. On a true cold
  disk the gap widens; against a real decode (7–20 ms, see the DNG benchmark)
  the disk tier itself is the win.
- Machine: Apple M5, 24 GiB RAM, macOS 27.0 (26A5388g), arm64; load average
  ~5–8 during measurement, so absolute times are local exploratory
  measurements, not calibrated-runner evidence.
