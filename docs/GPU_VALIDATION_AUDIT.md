# GPU validation, benchmark и security audit

Дата аудита: 2026-07-21. Область: `rrrah-gpu`, wgpu `30.0.0`, WGSL
`raw_view.wgsl` и путь запуска `rrrah-app`.

## Что проверяется в CI без физического GPU

`rrrah-gpu` теперь имеет host-only smoke test
`viewport_shader_parses_and_validates_with_naga`. Он использует Naga
`30.0.0` — ту же версию, которая входит в wgpu — и проверяет синтаксис,
типизацию, entry points и uniform/resource validation. Это не benchmark
драйвера: тест гарантирует только, что shader может быть передан в backend.

Запуск:

```bash
cargo test -p rrrah-gpu --all-targets --locked
cargo clippy -p rrrah-gpu --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

При изменении WGSL тест должен падать на несовместимом signed/unsigned
выражении, binding или layout. В частности, uniform `GpuParameters` остаётся
224 bytes и 16-byte aligned; это закреплено отдельным Rust-тестом.

При первом реальном Metal smoke-run Naga-проверка прошла, но runtime выявил
два класса ошибок, которые host-only parse не ловит:

* default texture view был `D2`, тогда как bind layout/shader используют
  `D2Array`; `upload_mosaic` теперь задаёт `dimension=D2Array` и полный
  `array_layer_count`;
* старый Rust uniform занимал 208 bytes и не учитывал 16-byte WGSL alignment
  перед `cfa`; Metal требовал 224 bytes. Добавлены явные host padding поля.

## Adapter matrix

Реальный GPU smoke запускается отдельной job на каждом backend. Один зелёный
Metal run нельзя экстраполировать на Vulkan, DX12 или WebGPU.

| Backend | Минимальный smoke | Что измеряем | Типичные ограничения |
|---|---|---|---|
| Metal (macOS) | `WGPU_BACKEND=metal cargo run -p rrrah -- <raw>` | first-visible, upload, frame p50/p95, RSS | unified memory не равен VRAM; timestamp queries могут быть отключены; размер texture array зависит от GPU поколения |
| Vulkan (Linux/Windows) | `WGPU_BACKEND=vulkan cargo run -p rrrah -- <raw>` | те же метрики + queue wait | драйвер/validation-layer overhead; discrete и integrated adapters дают разные limits |
| DX12 (Windows) | `WGPU_BACKEND=dx12 cargo run -p rrrah -- <raw>` | те же метрики + device-loss recovery | WARP нельзя считать production GPU; TDR/driver reset нужно тестировать отдельно |
| WebGPU (browser/WASM) | browser harness, same shader/fixture | upload/first paint/frame budget | no portable VRAM query; browser throttling and texture limits vary |

Перед загрузкой кадра приложение обязано записать в manifest:
`adapter_name`, `backend`, `vendor_id`, `device_id`, `limits`, enabled
features, driver/runtime version, fixture hash и quality tier. Без этого число
`GPU enabled` не является воспроизводимым benchmark-результатом.

## Texture-limit and failure matrix

Проверять pure planner tests на границах (8192/16384/adapter limit):

1. RAW dimension larger than `max_texture_dimension_2d` must select tiled
   residency or return a typed error, never allocate the full frame.
2. `tile_grid.x * tile_grid.y` overflow and
   `max_texture_array_layers` overflow must be rejected before allocation.
3. `row_pitch` must be a multiple of `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`;
   odd-width u16 rows need padding and must round-trip byte-for-byte.
4. Eager compatibility atlas is capped at 512 MiB. This is a safety guard, not
   a residency budget: large files must use the P0.1 tile path.
5. Device-lost or `OutOfMemory` must invalidate the current GPU generation and
   fall back to CPU/low-quality display. Retrying the same allocation forever
   is a release blocker.

Inject these failures in a backend test double; do not attempt to induce a
real device reset in ordinary CI.

## Benchmark protocol

### Реальный headless Metal smoke (synthetic, 2026-07-21)

На Apple M4 Max удалось запустить backend без окна через постоянный пример:

```bash
WGPU_BACKEND=metal cargo run -p rrrah-gpu --example gpu_smoke --locked \
  > target/bench/gpu-smoke-metal-20260721.csv
```

Пример создаёт детерминированную 14-bit RGGB `DecodedMosaic` (не CR2/DNG),
строит `R16Uint` tile-array и выводит CSV для 3 RAW-размеров × 3 viewport × 4
zoom. Для каждого размера два warmup и 20 render samples; p50/p95/p99 поэтому
являются exploratory GPU-completion числами, а не release confidence interval.
`upload_enqueue_ms` — CPU allocation/`queue.write_texture` enqueue без GPU
timestamp; `upload_wait_ms` — отдельный `device.poll(Wait)` перед первым view;
`render_*` включает submit и GPU completion. Render target создаётся ровно
размером viewport, формат smoke — `Rgba8UnormSrgb`.

Фактический артефакт: `target/bench/gpu-smoke-metal-20260721.csv`.
Зафиксированные limits: Metal, Apple M4 Max, `max_texture_dimension_2d=16384`,
`max_texture_array_layers=2048`, `max_buffer_size=4294967295`. Наблюдения:

| Synthetic RAW | eager upload enqueue (ms) | render p50 range (ms) | max p95 (ms) |
|---|---:|---:|---:|
| 1024×1024 | 17.37 | 0.385–1.229 | 1.494 |
| 2048×1536 | 14.85 | 0.296–1.435 | 1.465 |
| 4096×3072 | 15.39 | 0.349–1.414 | 1.864 |

Текущий eager atlas выбирает `tile_size=4094` на этом adapter (texture
width=4096), поэтому даже 1024² синтетический кадр резервирует один ~32 MiB
слой; это feasibility/over-allocation сигнал, не показатель CR2 first-open.
Результаты нельзя переносить на RAW entropy decode, cache hit, UI present,
другой surface format или Vulkan/DX12: каждый backend/driver требует
отдельного прогона и manifest.

Use the existing `rrrah-bench` JSONL/Chrome trace schema. A valid GPU run has
at least 10 warm iterations after one cold iteration, records p50/p95/p99 and
does not call `device.poll(Wait)` in the hot path. Report separately:

The current desktop app does not yet emit all GPU stage spans; until that
integration lands, these commands are a protocol for the benchmark harness,
not a claim that the viewer already produces complete GPU traces.
`gpu_smoke` is a bounded validation harness and intentionally waits for each
submission so its render columns mean GPU completion; it must not be read as
the application's asynchronous hot-path throughput.

* cold open: probe, decode, first visible tile, first present;
* warm open: RAM/disk cache hit and first present;
* pan/zoom: visible-tile misses, upload bytes and frame time;
* quality refinement: bilinear → MHC/RCD and cancellation latency;
* memory: RSS, CPU cache bytes, staging bytes and GPU residency estimate.

The CI gate is hardware-labelled and uses distributions, not one timing:

```text
first_visible_ms p95 <= target on the named adapter
frame_ms p95 <= 16.7 for 60 Hz (or <= 8.3 for 120 Hz)
missed_deadline_ratio < 1%
stale_generation_publish == 0
shader_validation_errors == 0
```

GPU timestamps are optional. If unavailable, report CPU submit-to-present and
mark `gpu_timestamp=false`; never manufacture a GPU duration from CPU wall
time. The `gpu_smoke` table is a measured synthetic completion baseline only;
this audit makes no CR2/DNG decode, cache, UI first-present, or cross-backend
performance claim.

## Security and supply-chain findings

`cargo audit` (advisory DB fetched 2026-07-21) reports:

* `quick-xml 0.39.4`, high severity RUSTSEC-2026-0194 and RUSTSEC-2026-0195,
  pulled by `wayland-scanner 0.31.10` in the Linux/window target. The fix is
  `quick-xml >=0.41.0`, but the current `wayland-scanner` requirement is
  `0.39`; upgrade the Wayland stack once a compatible release is available or
  carry a reviewed downstream patch. Do not silence the advisory.
* `ttf-parser 0.25.1` is reported unmaintained through `winit`/`sctk-adwaita`.
  This is a warning, not an exploit, but it must remain in release SBOM and be
  revisited with the next winit stack update.

The advisories are target-specific; they do not make the standalone GPU crate
unsafe, but the desktop Linux binary still needs a release decision. Run:

```bash
cargo audit
cargo deny check advisories bans licenses sources
cargo tree --target all -i quick-xml
```

No `unsafe` code is allowed by the workspace lint. Untrusted shader text is
not accepted at runtime: only embedded WGSL is compiled. RAW dimensions,
layer counts, row pitches and byte products must be checked before every GPU
allocation; malformed input must produce a typed error rather than a panic.

## Critic stop conditions

Reject a GPU performance claim or release if any of the following is true:

* shader was not validated by the same Naga major/minor as wgpu;
* result lacks backend, adapter, driver, limits, fixture hash or cache state;
* benchmark synchronizes the queue per tile or includes shader compilation in
  a warm-frame number without labelling it;
* CPU fallback and stale-generation/device-loss paths are not exercised;
* a texture-limit failure produces a blank window without visible diagnostics;
* a quality result is compared across different demosaic/tone-map tiers;
* RSS/VRAM is reported without stating whether the platform has unified memory.

This audit therefore establishes a trustworthy validation gate, not a claim that
the current eager atlas is production GPU residency. The latter remains the
P0.1 implementation task.
