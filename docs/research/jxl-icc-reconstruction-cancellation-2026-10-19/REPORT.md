# JPEG XL ICC reconstruction cancellation — 2026-10-19

## Change

`jxl-color::icc::decode_icc_with_check` adds cancellation opportunities before
reconstruction output allocation, between tag/main commands, inside shuffle and
predictor loops, and during long literal/shuffled copies. Shuffle loops poll at
4096-iteration boundaries; predictor positions poll every 4096 bytes; copies
poll at 64 KiB chunks. These are work-unit bounds, not wall-clock latency bounds.
Legacy `decode_icc` remains a no-op-check wrapper.

The existing optional jxl-oxide ICC admission callback is reused with the same
absolute high-water byte bound. Callback errors are unwrapped to their original
boxed type. The rrrah callback checks the generation and reserves only high-water
increases, so reconstruction polling adds no duplicate memory charges and keeps
`DecodeError::Cancelled` typed. Small-profile streaming initialization now makes
70 callbacks instead of the earlier 50, while holding the same 1445-byte working
credit and the same 70355-byte public peak.

## Evidence

The low-level regression uses six 131072-byte command payloads: literal copy,
shuffle2, shuffle4, and predictor widths 1/2/4. Cancellation on callback four is
inside the second copy chunk, a shuffle loop, or the width-one predictor loop.
All six return the injected cancellation error and then reconstruct successfully
with checks enabled, byte-identical to the no-op-check path. This tests command
processing and interruption; it is not an independent color-profile oracle.

The JXL integration regression injects a generation change at callback four on
the independent libjxl embedded-ICC fixture: two buffer admissions, the initial
reconstruction output check, then a tag-command checkpoint after the reconstructed
header is built. It verifies typed cancellation, 1445-byte admitted working
storage, full teardown to zero, and three successful selected-frame retries with
a fresh generation and the same root. This is deterministic callback injection,
not a live GUI/worker scheduling or measured latency test.

Existing independent libjxl native-sample/profile controls, ICC display references,
678 truncation prefixes, exact-peak/refusal tests, and entropy cancellation remain
passing. The actual Metal integration preserves 18 frames across six JXL fixtures
through pinned RAM, native/prepared-float swap, pressure refusal/retry and
SDR/half readback; final managed root usage is zero.

Terminal validation:

- `suites.log`: jxl-color 19; animation 35 /1 ignored; decode 229 /5 ignored.
- `app.log`: 288 passed /81 ignored.
- Ordinary total: 571 passed /87 ignored.
- `metal.log`: explicitly selected actual Apple M4 Max test passed, 18 frames.
- `format.log`: workspace formatting passed; source whitespace check passed.

One existing upstream lifetime-syntax warning remains; no strict Clippy claim.

## Open scope

Entropy-table construction, profile parsing/synthesis, and frame rendering still
lack equivalent internal checkpoints. Header/TOC/entropy/parser and temporary
synthesis allocations still need separate accounting; allocator/RSS limits are
not claimed. Maximum-sized profiles, broad producer coverage, live GUI endurance
and physical HDR remain unqualified. Broader 100-format correctness, six pending
catalog formats, full-history animation cost, physical slow disks and deferred
NVIDIA/CUDA remain part of the active overall goal.


Follow-up: the initial accumulated codestream header Vec now has separate
admission; see [header-buffer report](../jxl-initial-header-buffer-2026-10-19/REPORT.md).
Other header/container/TOC/entropy/parser metadata gaps above remain open.
