# Live Mac Metal HDR presentation

Current native surface baseline: 156 presented frames over 3.013 seconds, resize to 800x400 confirmed with 126 subsequent frames; RGBA16Float / ExtendedSrgbLinear; no GPU validation errors. Display API reported active HDR and current headroom 2.6666667 after presentation (initially 1.0 before activation).

Pinned six-case HEIF/EXR live script passed all cases, 1048 presented frames total. Each source decoded/prepared/uploaded into the live native HDR surface, released source credit after upload, remained presenting for at least three seconds, and presented at least 30 frames after resize. Fixture, binary and log hashes are in result.json.

This closes the previously observed locked/occluded-window gate for this run. It is actual native window presentation and OS-reported HDR activation, not calibrated luminance/color accuracy, full application navigation/cache/prefetch, all 100 formats or NVIDIA qualification. See individual logs and result.json.
