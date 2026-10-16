# Finite animation repeat phase and prefetch

Automatic animation requests now carry the playback clock repeat phase into the foreground worker. The final pass uses linear neighbours; earlier passes and infinite playback retain circular neighbours. Automatic direction stays forward across the last-to-first transition. Manual requests clear the phase hint and retain direction inference from selection. A changed source clears stale repeat evidence.

The hint is shared across coalesced same-frame requests and read after decode. A regression checks repeat-enabled → final-pass → manual transitions on the same pending request, with unchanged generation and no extra decode request. The planner regression covers final-pass end, forward wrap, manual reverse, and non-looping sources. Existing clock model exercises the phase getter across 48 scenarios.

Validation: focused planner and coalescing tests pass. Full viewer sequential run: 272 passed, 72 ignored. Parallel run: 271 passed, 1 failed, 72 ignored; edit_store concurrent-writer lock acquisition failed, then passed in isolation and in the sequential full run. The parallel instability remains open; this report does not claim that gate is clean.

Physical presentation pacing, HDR display qualification, and NVIDIA/CUDA remain outside this evidence. Ignored hardware tests were not enabled by this run.

## Controller and changed-source follow-up

Two additional regressions exercise the actual App::advance_animation and ForegroundLoader submission path. A three-frame, two-pass animation emits five requests, marks the final pass linear starting at its last-to-first transition, never repeats requests while a frame is pending, and stops after the sixth presented state. The final frame prefetch plan is empty with ahead=2/behind=0. A real changed temporary source triggers asynchronous source validation and a replacement generation; the old phase hint resets to unknown on both the original shared request and the replacement.

Both focused regressions pass. Full parallel viewer now: 275 passed / 73 ignored. These headless controller checks do not prove native window presentation or GPU timing.
