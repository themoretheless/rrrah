# RAW swap prefetch with partially free protected RAM

The RAW prefetch worker checks ready RAM first, then obtains the optional
minimum restored sensor weight from the temporary swap index. If protected
entries leave insufficient capacity for that floor, it returns an unadmitted
result before temporary swap restore, persistent cache load, or native decode.
Unknown/cold objects retain the existing two-byte lower bound. The separate
prepared-CR3 source admission continues to guard cold CR3 sensor decoding.

The index hint is logical u16 sample bytes, not allocated source capacity and
not compressed wire length. It is not an integrity check or a promise that
subsequent restoration will fit every independent memory limit. Reading an
index hint does not read sensor payloads; expiry may still remove backing files.
Persistent cache objects without a temporary-swap hint are not covered by this
partial-space preflight. Model payload admission remains a separate gap.

## Evidence

- Full ordinary application suite: 293 passed, zero failures, 84 ignored.
- Explicit local-original CR3 swap regression: passed. Two full sensors occupy
  protected entries of a 2.5-sensor RAM cache. A two-byte floor fits while a
  complete sensor does not. The far object is already in temporary swap.
  Refusal leaves reads zero and managed used/peak unchanged. A nearest RAM hit
  still succeeds with all remaining root credit reserved. Releasing its lease
  permits one swap read and a retained far lease. Every sensor value and all
  metadata equal the original native decode; the selected frame remains
  resident and final managed use returns to zero after shutdown.
- Existing real-CR3 forward/backward nearest-lease test: passed for count,
  full-byte, and partial-byte policies, including cold source preparation.
- Formatter and patch whitespace checks passed.

The tests use disposable copies of the existing local EOS R8 original. No source
media is stored in this report. These checks do not qualify physical slow disks,
all camera families, NVIDIA/CUDA, physical HDR, or the full 100-format objective.
See `application.log`, `original-swap.log`, and `original-cold.log`.
