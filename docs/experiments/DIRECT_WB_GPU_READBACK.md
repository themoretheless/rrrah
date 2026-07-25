# Direct WB: решение и GPU readback

Дата решения: 2026-07-25.

## Решение

Renderer загружает `RawMetadata.white_balance` без изменения:

```text
uploaded_white_balance = decoder_white_balance
```

Backend уже формирует multiplicative camera-space correction gains и выбирает
их общий green-normalized scale. Exposure остаётся отдельным scene-linear
параметром.

Rec.709 luminance weights определены для linear display RGB. Применять их к
вектору до camera-to-display matrix некорректно: это сохраняет отношения
каналов, но скрыто меняет экспозицию.

## Численное различие

Для EOS R8 gains `[1678/1024, 1, 1659/1024, 1]` отвергнутая
Rec.709-нормализация имела scale `0.847059846`, то есть `−0.239464 EV`.

Первичный Metal readback кандидата:

| Вариант | Exposure | R | G | B |
|---|---:|---:|---:|---:|
| unit-gain reference | 0 EV | 141 | 141 | 141 |
| EOS R8 direct gains | 0 EV | 141 | 141 | 141 |
| Rec.709-normalized gains | 0 EV | 129 | 129 | 129 |
| normalized + compensation | +0.239464 EV | 141 | 141 | 141 |

Это показывает, что normalization была общим exposure scalar, а не
необходимой частью chromatic correction.

## Production gate

Основной workspace содержит integration test
`decoder_white_balance_scale_is_preserved_through_real_camera_profile`.
Он рендерит через настоящий wgpu pipeline и реальную EOS R8
`xyz_to_camera`-матрицу:

```text
RAW normalization → demosaic → direct WB → EOS R8 camera matrix
→ exposure → ACES → Rgba8UnormSrgb → readback
```

Тест требует:

- direct EOS R8 case совпадает с unit-gain neutral reference в пределах
  двух 8-bit code values;
- direct case совпадает с CPU ACES/sRGB reference;
- отвергнутая Rec.709-нормализация отличается минимум на восемь code values.

Запуск:

```sh
WGPU_BACKEND=metal cargo test -p rrrah-gpu --test readback \
  decoder_white_balance_scale_is_preserved_through_real_camera_profile \
  -- --nocapture
```

На Apple M5 / Metal тест проходит.

## Граница доказательства

Readback проверяет контракт выбранного pipeline, но не является внешней
колориметрической калибровкой EOS R8. Абсолютную экспозицию и матрицу камеры
по-прежнему следует сверять с независимым RAW/color-chart reference.

Исторические `d.md` и `F.md` сохраняют арифметику отвергнутой policy, но не
описывают актуальную renderer boundary.

