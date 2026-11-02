# PNG PQ display-policy swap isolation — prepared, not yet executed

Live source review: raster_display_key includes PQ reference-white bits and all three HLG display-policy float bit patterns. Existing pq_white_cli_separates_cache_and_reaches_thumbnail coverage uses AVIF and multiple RAM entries; it does not establish policy isolation across count-one RAM eviction into swap for the new narrow-PQ PNG route.

Added narrow_png_pq_white_policies_remain_distinct_through_count_one_swap in the app binary tests. It uses the authored 16-bit chromatic/alpha narrow-PQ PNG, white policies 100/200, one RAM entry and two disk entries. It checks RGB halving with unchanged alpha, four alternating swap restores with decoder closure forbidden, bit-exact matching to each policy's prior output, expected read increments, stable managed weight and final zero root credit.

Status: source formatting/whitespace checked; no compilation or runtime claim. Heavy work is deferred for the explicitly coordinated GPU measurement window. No process from this thread is paused; no signal was sent. Run after coordination release:

    cargo +stable test -p rrrah --bin rrrah narrow_png_pq_white_policies --locked --offline -- --nocapture

Then broaden app tests only if the focused test passes and the resource window is released. Do not count this pending regression as a completed cache qualification gate.

## Execution started, then paused by coordinator

After controller4842 terminated with admission timeout and both controller/test executable were absent, the deferred CPU build started as exec session67847. A subsequent coordinator SIGSTOP paused cargo49496 and rustc50236 (recorded in its paused-common-builds.json, timestamp1792616902). Their state was verified T/Ts. Resume these identities instead of restarting the build. Compilation duration is contaminated by this pause; no test result/performance claim exists yet. The initial no-process coordination acknowledgement has been updated with both identities and signal provenance.

## First execution and corrected test headroom

After explicit coordination release the original session completed compilation and ran the test. It failed on the decoder-forbidden restore path (rejected-one-frame-restore.log). Source review of ImageSwapCache::try_get confirms live restored pixels consume the shared restore quota; with one frame's restore quota, the visible restored frame leaves no space to restore its replacement. The revised test keeps count-one/one-frame RAM and a two-entry disk index, but gives restore quota two frame weights so the old visible frame can remain protected until publication. Four alternating no-decode restores and all ownership assertions remain required. This is a correction to test admission setup, not a runtime cache change. Retest is in progress; no pass claim yet.

## Verified final result

Corrected focused test passed (1/0 ignored/0 failed); full app gate passed (297/84 ignored/0 failed). See REPORT.md for the current conclusion and the preserved rejected one-frame restore setup. Earlier pending statements above describe historical execution stages only.
