# Owned managed PNG ICC inflation

Used embedded PNG ICC is now inflated by the native application metadata route, not by image/png's private ICC parser followed by a cloned getter. The image PNG pixel decoder always skips ICC/text because the validated native color route already supplies the selected profile/declaration. Existing external vendored constructors retain their original metadata behavior. cICP-priority images still skip unused ICC entirely.

The inflater borrows compressed source, reserves the existing conservative65536-byte pinned zlib-rs state/history credit before Decompress construction, uses4096-byte bounded stack output and at most65536inputbytes per step, checks cancellation around each step, and reserves output capacity before fallible Vec growth. Growth is geometric when budget allows and falls back to exact required capacity under pressure. Capacity credit travels with the returned profile/shared color metadata; state credit is released before PNG pixel decoding. Complete stream, checksum, nonempty output and no trailing compressed data are required; malformed used ICC becomes a typed color error rather than a silently swallowed profile.

Controls verify:
- Exact65536state+588profile=66124peak, final588credit, one-byte-short typed refusal, invalid checksum and trailing-stream rollback.
-100000empty DEFLATE blocks (500KiB compressed transport) cancel after actual state admission, release everything, and retry to exactICCP bytes.
- Cancellation after the first4096outputbytes observes state+4096credit, then releases both and permits full retry of32769profilebytes.
- Integrated PNG inner route retains588profile+1024pixel bytes; last raster/pixel owners release their respective credits. Its peak expectation now includes native ICC state.
- MNG inherited-ICC control explicitly refuses the old32768-byte budget and succeeds with admitted inflate workspace; profile/pixel last-owner behavior remains intact.

Full public decoder library: **211 passed /5 ignored /zero failures**. Viewer: **285 passed /80 ignored /zero failures**. Initial two insufficient old-budget failures are preserved rather than discarded. Existing independent ICC/pixel/color/EXIF controls continue to pass.

The65536 state value is a conservative managed admission bound already used/allocator-qualified for the pinned zlib-rs backend, not a measured exact RSS amount. The4096stack scratch, allocator bookkeeping and fixed PNG pixel decoder's fdeflate/row/unfiltering/EXIF storage remain outside this component's heap credit. This closes used ICC inflater/profile lifetime admission only. APNG's direct image-based metadata path and other codecs remain separate. No unmeasured latency speedup, physical HDR/NVIDIA or complete100-format claim.
