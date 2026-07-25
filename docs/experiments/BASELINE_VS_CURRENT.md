# A/B: `9497871` против объединённого экспериментального дерева

Дата замера: 2026-07-24. Машина: Apple M5, macOS, Metal, release-профиль.

> Исторический benchmark snapshot. Подтверждённые CPU-изменения уже находятся
> в main (`60766c2`, `8ba4443`), где packed DNG дополнительно получил
> fixed-group и parallel decode. GPU policy после этого отчёта уточнена
> адаптивным follow-up в [`C.md`](C.md).

## Итог

- DNG hot paths ускорились во всех пяти синтетических сценариях:
  lossless-JPEG на 5–14%, packed 10/12/14-bit на 27–44%.
- Warm end-to-end CR3 decode двух реальных EOS R8 файлов ускорился на 17–35%.
- DNG differential tests и полные CR3 BLAKE3 совпали до/после.
- Фиксированный GPU default 1022 оказался размер-зависимым и потребовал
  последующей адаптивной эвристики.

## DNG hot paths

Два независимых batch по 30 interleaved old/new раундов. Harness сравнивал
byte-wise reference с word-wise implementation и требовал bit-identical output.
В таблице — среднее двух batch p50.

| Сценарий | Было | Стало | Изменение | Speedup |
|---|---:|---:|---:|---:|
| lossless-JPEG, gradient 4000×3000 | 385.185 ms | 366.277 ms | −4.9% | 1.053× |
| lossless-JPEG, random 1024×1024 | 38.237 ms | 32.783 ms | −14.3% | 1.166× |
| packed unpack, 10-bit 6000×4000 | 57.561 ms | 42.037 ms | −27.0% | 1.369× |
| packed unpack, 12-bit 6000×4000 | 59.441 ms | 33.204 ms | −44.1% | 1.790× |
| packed unpack, 14-bit 6000×4000 | 67.833 ms | 39.212 ms | −42.2% | 1.730× |

Это synthetic microbench, не end-to-end реальный DNG.

## Реальный EOS R8 CR3

Порядок процессов: `baseline → current → current → baseline`. На каждый
checkout и fixture: 14 измерений после warmup, 4 workers. Ниже pooled
wall-clock p50/p95.

| Fixture | Было p50 / p95 | Стало p50 / p95 | p50 изменение | Speedup |
|---|---:|---:|---:|---:|
| `IMG_9043.CR3` | 131.593 / 186.547 ms | 86.096 / 148.167 ms | −34.6% | 1.528× |
| `IMG_9074.CR3` | 107.608 / 177.354 ms | 89.389 / 133.989 ms | −16.9% | 1.204× |

Полные u16 mosaic digests:

- `IMG_9043.CR3`:
  `93e1edb11bcc962c1689c84709f3ac0a3b0aa5b8ab19f9116e12798316d875bd`;
- `IMG_9074.CR3`:
  `ef677bae0d39f0164e943aaa81c61c064151b5503bdead9400574ae2def9db62`.

## GPU snapshot

В этом snapshot сравнивались fixed `1022` и legacy `4096`. Результат оказался
неуниверсальным:

| RAW | Completed wall было → стало | Результат |
|---|---:|---:|
| 2048×1536 | 32.934 → 11.614 ms | 2.84× быстрее |
| 4096×3072 | 34.938 → 41.330 ms | на 18.3% медленнее |
| 6240×4160 | 144.886 → 96.416 ms | 1.50× быстрее |
| 8192×6144 | направление менялось между batch | паритет/шум |

Именно регрессия `4096×3072` привела к adaptive planner: aligned 1022
остаётся базой, а один frame-sized tile выбирается, когда materially уменьшает
atlas. Актуальные числа находятся в `C.md`.

## Воспроизведение

```sh
cargo run -p rrrah-decode --release --example decode_timing

RRRAH_CR3_BENCH_VARIANT=current \
RRRAH_CR3_BENCH_REPS=7 RRRAH_CR3_BENCH_WARMUPS=2 \
  cargo run -p rrrah-decode --release \
  --example cr3_end_to_end_timing -- \
  tests/IMG_9043.CR3 tests/IMG_9074.CR3

WGPU_BACKEND=metal RRRAH_GPU_UPLOAD_AB=1 \
RRRAH_GPU_UPLOAD_AB_REPS=16 \
  cargo run -p rrrah-gpu --release --example gpu_smoke
```

Ограничения: warm page cache, заметные фоновые пики, synthetic DNG и
нестабильный GPU p95 на 16 samples.

