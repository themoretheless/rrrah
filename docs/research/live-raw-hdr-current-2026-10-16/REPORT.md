# Live native CR3 HDR surface

Local CR3 decoded through NativeRawDecoder with a 512 MiB source budget and uploaded to RawRenderer configured SceneLinear. Sensor 6188x4120 / 25,494,560 samples. The probe asserts source credit is zero after dropping the uploaded mosaic.

Actual Metal M4 Max window: 131 frames over 3.002 seconds; resize to 800x400 confirmed, 101 frames after resize. OS HDR active and headroom reached 2.6666667. RGBA16Float / ExtendedSrgbLinear; no validation errors. Probe exited successfully. Source media and image pixels remain local and were not copied into this report.

This is live native RAW surface and resize qualification, not calibrated photographic color/luminance, full application navigation/cache/prefetch or NVIDIA. Independent compute/readback correctness is separate evidence.
