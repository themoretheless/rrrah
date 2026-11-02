# Empty RAW prefetch plan supersedes previous work

`RawPrefetcher::finish_foreground_and_submit` replaced a valid plan with a new generation, but its invalid-selection/empty-gallery branch only changed the HUD to idle and cleared the foreground flag. It did not invalidate the old generation or drain an existing queued command. Direct plan replacement could therefore leave obsolete speculative work eligible to run. Normal callers that already called begin_foreground had an additional cancellation guard; the new fix makes empty-plan replacement obey the same supersession contract on its own.

The regression first submits a valid forward plan, retains its generation token, then submits an out-of-range selection or an empty gallery. Before the fix it fails because the old token remains live (red.log). The fix advances the generation, drains queued work and publishes a zero-item telemetry generation before returning idle. Delayed phase/store reports from the old generation are ignored. A later backward plan is accepted with the expected previous neighbour and a fresh generation.

The test uses the existing deterministic no-worker scheduler harness. It proves queue invalidation, cancellation-token state, stale telemetry rejection and fresh scheduling; it is not evidence for a real disk-write race, a real camera collection or GPU output. Existing actual-camera/Metal evidence remains separately documented.

Initial gallery gate after the scheduling fix: 42 passed, 16 hardware/manual tests ignored, zero failures (gallery.log). The final app gate additionally covers stale telemetry rejection.

Final full app binary unit/integration gate: 295 passed, 84 ignored, zero failures in 4.14 seconds excluding compilation (app.log). This includes normal cache/swap viewer, selected pages/frames and blocked-prefetch tests; ignored hardware/manual gates were not exercised. Source hash pinned in source-hashes.json.

## Joined RAW prefetch shutdown — 2026-10-24

The RAW warmer previously discarded its JoinHandle immediately after spawning. Drop cancelled its private lifetime and drained pending commands, but could return while the worker still held managed decoded pixels or was finishing cache work. The warmer now retains the handle; Drop first cancels, drains, disconnects its command sender, then joins. This mirrors the existing thumbnail/foreground worker ownership and does not change the shared speculative generation. Spawn failures retain their existing disabled-worker behavior.

Focused lifecycle gate: 3 passed / 0 failed / 0 ignored (`shutdown-focused.log`): existing private-lifetime cancellation isolation, a real idle RAW worker woken by sender disconnection and joined, and a controlled active worker holding all 4096 managed bytes until cancellation/unwind. Drop must observe completion and root=0, not eventual cleanup after returning. The active-buffer control is not a real RAW codec or slow-disk test. Shutdown can wait for an already-started filesystem operation; this does not prove a bounded shutdown duration on stalled storage.

Full app: 324 passed / 0 failed / 84 ignored, 25.93 s. Current raw full-app output replaces the preceding successful duplicate at `../prefetch-count-swap-hit-2026-10-16/app-current.log`; unique original failures remain. App-only test opt-level=0. No performance, all-format or physical HDR/NVIDIA completion claim.

## Cancellation during sampled source fingerprinting — 2026-10-24

RAW prefetch previously used the uncancellable source-fingerprint entry point before its cancellable recipe/decode/store stages. New `SourceFingerprint::from_path_with_cancel` checks before filesystem work, between samples and at most 16 KiB requested read chunks, and before publishing a result. The previous `from_path` delegates with cancellation disabled. Sample offsets, prefix, encoded length/offset stream and sampled data are unchanged, so no key namespace or cache invalidation is needed. The RAW worker now supplies its combined generation/private-lifetime token; cancellation exits rather than recording a failed neighbour.

The new test covers empty/small, 64 KiB, overlap-union and disjoint-sample branches up to 256 KiB, every reached cancellation callback and successful retry. Cancellation precedes opening a nonexistent path. Existing independent overlapping-sample hash-stream controls remain enabled, separately checking compatibility. Full cache library: 152 passed/0 failed/3 ignored (`fingerprint-cache-current.log`). Full app including shutdown/control paths: 324 passed/0 failed/84 ignored, 31.61 s; current complete output supersedes the earlier duplicate at `../prefetch-count-swap-hit-2026-10-16/app-current.log`.

An already blocking OS metadata/open/read call cannot be interrupted by these cooperative checks; this is not a wall-clock shutdown bound for stalled storage. Fingerprint sample buffers remain bounded temporary allocations and are not newly charged to the managed pixel budget. Initial new-test compilation needed a qualified std::fs import; no behavioral gate was claimed from that failed compile.

## Foreground RAW and model identity cancellation — 2026-10-24

The same cancellable fingerprint API is now connected to main-worker RAW prefetch, selected RAW loading and model cache identity. These call sites previously still used uncancellable `from_path`. Selected RAW identity failure now returns silently when its generation token has been cancelled, instead of recording a disk-read error; genuine I/O errors retain their reporting path. The CLI inspection path without generation cancellation retains ordinary `from_path`.

A genuine red/green control on the existing OBJ triangle demonstrates the model identity issue: the previous helper returned a key after generation cancellation (`model-identity-red.log`), while the new helper returns None. A fresh generation retries to exactly the old key (`model-identity-green.log`, 1 passed/0 failed/0 ignored). This establishes the identity helper contract; it does not claim the old full renderer displayed a cancelled model or isolate a mid-system-call foreground telemetry race. Cache-library chunk/checkpoint and frozen hash-stream controls are retained separately.

Full current app 326 passed/0 failed/84 ignored, 21.85 s runtime, app-only test opt-level=0; the full successful log replaces the previous duplicate in `../prefetch-count-swap-hit-2026-10-16/app-current.log`. Source hashes in `foreground-model-identity-source-hashes.json`. Existing OS-call interruption limits remain.
