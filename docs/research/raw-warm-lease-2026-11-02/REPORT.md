# Direct warm RAW lease handoff — 2026-11-02

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

The required original-source test passes **1/0 failed/0 ignored** in [raw.log](raw.log).
Ordinary prefetch regression passes **66 tests/23 manual ignored** in
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
