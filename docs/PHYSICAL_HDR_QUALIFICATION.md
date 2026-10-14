# Physical HDR qualification — RR-0006

Status: protocol ready; no physical display measurement recorded.
Offscreen Metal readback is an independent prerequisite, not a physical result.

Use a calibrated luminance meter, a documented HDR display, and a real adapter.
Record display model/firmware, OS, GPU/driver, connection, refresh rate, output
format/color space, HDR/EDR enablement, reference white and peak brightness.
Disable automatic brightness and record local dimming and ambient illumination.
Warm up the display and meter according to their manufacturer specifications.

1. Run the SDR/HDR raster swap readback gate and retain source hashes and logs.
2. Display independent 10% window patches on black: linear relative luminances
   0, 0.01, 0.18, 1, 2 and 4 with reference white 100 cd/m². Measure the center
   after stabilization three times, plus repeated 100% white to expose ABL.
3. Repeat using independently authored PQ and HLG sources. Record signal codes,
   transfer function, primaries, mastering metadata and the application's PQ
   reference-white/HLG-gamma options. Do not use the tested decoder as oracle.
4. Require finite readback, monotonic measured luminance, reference-white error
   ≤5% and patch error ≤10% above the meter's documented noise floor. Mark black
   as an upper bound at that floor. Record clipping at the measured display peak;
   do not require a display to exceed its documented capability.
5. Toggle HDR off and repeat the 1/2/4 patches. An SDR surface must not receive
   a physical HDR pass. Likewise, screenshots, missing meter readings and an
   unavailable HDR surface are unverified, never successful substitutes.

Save raw readings, meter calibration identity/date and uncertainty, source/output
hashes, three repetitions per patch, limits and each pass/fail decision in a JSON
report. A report is incomplete until every requested patch and negative control
has a recorded outcome. Physical qualification remains open until this protocol
has been executed on the target display.
