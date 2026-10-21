# Exact HDR readback capacity — 60-second pilot

The shared test readback harness now reserves the exact binary16 output element count before extending rows. The former flat_map collect has no exact lower size hint and grows geometrically, retiring multiple large backing stores per frame. Pixel decoding, complete comparison, Metal submission waits, swap limits, pressure and cancellation are unchanged. No allocator override or readback destruction is used.

The current frozen candidate passed324 cycles, including324 parallel scratch cancellations and324 exact full4000x3000 SDR/HDR readbacks. Sampled peak RSS 1993883648 bytes. This is a test-harness allocation improvement; it is not proof that production viewer memory has been fixed, a controlled speed benchmark, or full format/HDR-display qualification. Original baseline and failed destruction controls are retained in adjacent report directories.

A600-second full-frame run using the same frozen candidate and unchanged4GiB guard is running separately in metal-exact-readback-600s-2026-10-21. Its terminal outcome is not yet known.
