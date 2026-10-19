# Raster prefetch under partially free protected RAM

Retained raster worker prefetch now checks the swap index's guaranteed minimum
restored pixel weight before reading a nonresident swap object. RAM hits are
checked first. A refused hint remains a refusal after both color-assumption keys
are probed, preventing a fallthrough into cold decoding. Count-limit diagnostics
retain their priority. Foreground and explicit ephemeral lookups keep their
existing behavior.

The hint lives with the swap handle and expires/replaces with that index entry.
It does not verify payload integrity. Lookup does not read/decode pixel payloads
or reserve restore memory; expiry can still unlink backing storage. Unknown
payload hints fall back to the existing four-byte minimum. This change does not
add exact admission for cold sources, persistent disk-cache objects, model
payloads, or RAW restore paths. The hint excludes ICC weight and is only a lower
bound, so passing this check is not a guarantee of final admission.

The existing associated-EXR regression now covers count pressure, fully occupied
protected bytes, and partial free space (selected plus nearest occupy 2W of a
2.5W cache). In the last case the four-byte test explicitly fits while the whole
frame does not. Refusal records zero swap reads and unchanged managed used/peak;
a warm nearest hit remains available. Releasing the nearest lease permits one
swap read, exact serialized raster equality, and a retained lease. The selected
frame remains resident and all managed credit returns to zero after shutdown.

Targeted regression passed. Full application validation passed: 293 tests,
zero failures, 83 ignored. See `application.log` and `targeted.log`. This does not qualify physical slow disks, HDR displays, CUDA,
NVIDIA, or all 100 formats.
