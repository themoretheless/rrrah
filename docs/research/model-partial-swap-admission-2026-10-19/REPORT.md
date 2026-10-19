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
