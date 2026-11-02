# RAW lease handoff and retained swap recovery — 2026-11-02

The resident RAW prefetch path now obtains its consumer lease in one lookup.
Previously it cloned the resident mosaic, discarded the temporary clone, and
then requested a consumer lease. A TTL boundary between the two calls could
lose retention. The direct lookup returns the initial lease and checks generation
cancellation before handing it to the worker. Source identity/recipe validation
still precedes the lookup; no pixel decoder or cache format was changed.

An original local EOS R8 CR3 was decoded once with the owned native backend.
Its warmed mosaic was put into a one-entry cache with a one-second TTL. The
test then occupied all remaining managed root capacity and held the foreground
decode permit. Warm prefetch returned before that gate was released, with the
same sensor allocation, samples and metadata; the visible pin remained unset.
This does not claim zero source-identity I/O or measure navigation latency.

After the TTL elapsed, new consumer lookups failed while the already acquired
lease stayed valid and prevented count-based replacement. Releasing that lease
allowed admission. Final managed usage was zero. The five-second response timeout
is a test guard, not a performance result. The original stays outside this evidence
directory; [pins.json](pins.json) records its size/hash and the test binary/source
hashes without copying the photograph.

The current required original-source suite passes **4/0 failed/0 ignored** in [raw.log](raw.log).
Ordinary prefetch regression passes **69 tests/24 manual ignored** in
[prefetch.log](prefetch.log); [production check](production-check.log), formatting
and diff whitespace checks pass. The manual case is among those skipped by the
ordinary filter and was executed separately; it is not counted as an additional
ordinary-suite pass.

```sh
cargo test --locked -p rrrah --bin rrrah real_cr3_prefetch_returns_first_ram_lease_under_pressure_and_ttl -- --ignored --nocapture --test-threads=1
```

This closes the double resident lookup for this RAW path. It does not qualify
every camera, swap/cold RAW prefetch under all pressures, calibrated display,
NVIDIA/CUDA, or the separate pending three-hour raster-worker run.

## Retained swap recovery

`MosaicRamCache::try_get_retained_with_cancel` is an explicit retained-consumer path. It checks byte/count admission before restoration, may reclaim unpinned/unleased LRU entries on recoverable memory refusal, and fences active spills at most once with cancellation before retrying. Visible pins and consumer leases remain protected. The legacy background-clone API keeps its refusal-without-eviction behavior. RAW worker prefetch receives the returned lease directly. Cold source decode and persistent disk-cache memory recovery are not changed by this repair.

The new original CR3 case uses three actual file copies with distinct timestamps/keys. One native decode supplies their identical expected sensor payload; three swap objects are written and read. A restore pool of two sensor allocations is full while the managed root's remaining capacity is reserved. Two held leases refuse the third restore without a pixel read (reads=2). Dropping the old neighbor's lease lets retained prefetch reclaim that neighbor and restore the third (reads=3), with exact samples/metadata, the same selected allocation still pinned, and final managed usage zero. This is controlled managed admission pressure, not physical OS memory exhaustion or slow-disk qualification.

The four original-source tests also cover both prefetch directions and count/byte limits, partial protected-budget admission, and warm TTL handoff. [Cache suite](cache.log): 160 passed, 5 ignored, zero failures, including a small managed-pool regression with cancellation and protected refusal/replacement. Current ordinary prefetch:69 passed/24 ignored; required original-source:4 passed/zero ignored. The final added case is test-only; production check ran on the same production implementation before that case was added. Format and diff checks pass.

```sh
cargo test --locked -p rrrah-cache --lib
cargo test --locked -p rrrah --bin rrrah gpr_preload_tests::real_cr3 -- --ignored --nocapture --test-threads=1
```

These are CPU sensor/cache proofs. They do not add new physical HDR, Metal shader, camera-format-wide, or NVIDIA qualification. Logs and pins above replace the earlier positive checkpoint rather than duplicate it.
