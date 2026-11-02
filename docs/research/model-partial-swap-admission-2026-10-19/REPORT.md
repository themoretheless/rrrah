# Model swap admission under partial protected byte pressure

STL, OBJ, OFF, and PLY now supply a minimum restored cache weight alongside
their temporary-swap handle. The estimate follows the existing model cache's
accounting convention, using logical vector/string lengths rather than spare
source capacities. It covers model headers, geometry, nested attributes and
metadata; shared OBJ strings remain charged per reference, just as in retained
capacity accounting. Existing retained-capacity weights use the same traversal
with capacities and preserve their prior behavior.

Retained model prefetch still returns ready RAM hits first. On a RAM miss it
checks count admission, then the indexed restored-weight floor before reading
the swap payload. Insufficient protected-byte headroom refuses without restoring
or falling through to native decode. Unknown/cold payloads keep the prior
one-byte minimum. Foreground and explicit ephemeral lookups keep their behavior.
No on-disk payload version changes were made.

## Validation

- Application suite: 294 passed, zero failures, 84 ignored.
- Mesh library: 55 passed, zero failures.
- All 28 model fixture payloads round-trip exactly. Their minimum weights remain
  unchanged after restoration and never exceed either source or restored cache
  weights. This covers the local fixture corpus, not all possible model files.
- An empty STL with reserved unused facet capacity retains its nonzero header
  floor while the source capacity weight remains larger.
- Retained prefetch tests cover each supported model variant under count, full
  protected-byte, and partial protected-byte pressure. In the last case cache
  capacity is 2W + F/2, where W is source retained weight and F the far object's
  logical restored floor; the old one-byte probe fits and the new floor does not.
  Refusal leaves swap reads zero and managed used/peak unchanged. Releasing the
  nearest lease permits one exact payload restore and a retained lease; the
  selected visible key stays pinned and final managed use returns to zero.

This lower bound is not a guarantee of final admission under independent RAM,
queue, or swap budgets. Cold source sizing, persistent cache hints, physical
slow disks, NVIDIA/CUDA and physical HDR remain separate qualification work.

## Actual Metal readback

The model-readback suite passed on Metal Apple M4 Max: 7 passed, zero failures,
1 manual repeated-pressure test ignored. Six model swap-cycle payloads and
rendered frames remained exact, including SDR/HDR render targets; cancellation
and pressure recovery released all managed and queued credit. This is offscreen
readback, not physical HDR-display validation. See `metal.log`.

## Transient shared-root restore refusal — 2026-10-24

The restored-weight floor preflight protects RAM membership, but an independently occupied shared memory root can still refuse the actual swap allocation. Previously the retained-background model path treated that transient memory refusal as a cache miss and entered native decode. The new real-cache regression reproduced the fallthrough on triangle.stl (restore-pressure-red.log). In the worker the fallback could also enter/wait for the decode gate and re-read the model source unnecessarily.

The retained, non-visible path now returns a specific refusal after a retryable memory-capacity error from swap; it checks cancellation first. It preserves the immutable swap object for another attempt instead of treating it as invalid. Foreground and explicitly non-retaining paths keep their existing fallback behavior. A permanently insufficient configured restore cap (requested allocation greater than its limit) also keeps native fallback, allowing normal decoding when the shared root has headroom. No payload format or estimate changes were made.

The combined CPU/actual-Metal regression covers STL, OFF with binary attributes, OBJ with shared metadata and float64 PLY. A real native payload is stored in swap; another selected model stays pinned in RAM. With zero or one byte free in the128KiB shared root, count/minimum-weight preflight still permits the neighbour, but atomic restore admission refuses. A decoder panic guard proves this never enters native fallback. Visible key and membership stay unchanged, successful swap reads/errors remain0 and pressure release restores the exact retained object without native decoding. Payload bytes compare exactly, including source attributes/metadata. The successful restore is retained alongside the selected model.

A separate one-byte restore-cap cache proves a real indexed payload was written (write count1 and a known minimum weight above1), then permits exactly one native decode under otherwise available shared memory; payload bytes still match. This distinguishes permanent restore configuration from transient occupancy. Every source ends with managed root0.

On actual Apple M4 Max, the retry produces nonempty neutral-geometry SDR and float16 HDR frames exactly matching the original pose at64×64 and63×65. This covers existing geometry rendering, not source materials/textures or physical HDR. Focused CPU1 passed/0ignored/0failed (restore-pressure-current.log); required Metal1 passed/0ignored/0failed, four source variants (restore-pressure-metal.log). Full app330 passed/85ignored/0failed, shared current log at ../prefetch-count-swap-hit-2026-10-16/app-current.log. Ordinary production check passed (restore-pressure-check.log). No speed, real slow-disk, physical VRAM or NVIDIA claim follows.

Implementation/source SHA-256 (crates/rrrah-app/src/main.rs): 1b8bc9bc6a47c34277f2064f8fd51dc05bec1d589ff5f6c1075e7045cb76d42d.
