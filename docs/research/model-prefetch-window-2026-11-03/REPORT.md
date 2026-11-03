# Preserve the requested model prefetch window

The production loader now tentatively leases already-resident model neighbours
before loading a cold member of the same previous/next window. This prevents
a cold insertion from evicting a warm member that is about to be requested.
Tentative leases are separate from delivered prefetch leases: if admission
fails, future tentative leases release and the current higher-priority request
retries once. The visible model and delivered neighbours remain protected.

When a spill is still being written, an opt-in `SwapPayload::pending_clone`
can now hand off a shared immutable resident owner instead of decoding again
or waiting for disk. The default remains disabled. Production `ModelAsset` opts
in for GLB/STL/OBJ/PLY/OFF; its clones share admitted backing and memory credits.
The pending marker removes its owner on completion, rejection or cancellation.
Lookup checks write generation and cancellation, and rechecks the disk index
to cover publication between the two lookups. `pending_hits` is reported
separately from disk `reads`. Completed blobs still obey the restore budget;
resident pending handoff spends no new pixel/geometry allocation or restore pool.
This opt-in contract excludes deep-copy implementations.

## Evidence and reproducibility

The opt-in application test uses the actual foreground worker, model cache,
native swap writer/restorer and retained-neighbour protocol. It creates distinct
filesystem copies of five pinned fixtures: original BoxTextured GLB, authored
STL/OFF, shared-metadata OBJ and 64-bit PLY. These are many paths, not 1024
independent producers or different scene contents. Each foreground event must
preserve the expected format and triangle count. Per-source audit requires
exactly one native decode over the entire forward/backward traversal.

Limits: 4 MiB shared managed CPU root, 1 MiB/three-entry model RAM cache,
256 MiB native swap with at most collection-size objects, 4 MiB/eight-write
queue and 2 MiB restore pool. One previous and one next neighbour are awaited
and retained each generation. RAM TTL is configured to one second; this test
does not independently assert a particular expiration deadline. Separate
existing TTL regression controls remain authoritative for expiration semantics.
The CPU root does not cover filesystem paths, test-audit metadata, all codec
scratch or physical process/driver RSS.

```sh
RRRAH_MODEL_COLLECTION_COUNT=1024 cargo test -p rrrah --bin rrrah \
  production_collection_prefetch_count_ttl_and_native_restore --locked -- \
  --ignored --nocapture --test-threads=1

RRRAH_MODEL_COLLECTION_COUNT=128 RRRAH_QUALIFY_SWAP_DELAY_MS=1 \
RRRAH_QUALIFY_SWAP_CHUNK_BYTES=65536 cargo test -p rrrah --bin rrrah \
  --features qualification-slow-io \
  production_collection_prefetch_count_ttl_and_native_restore --locked -- \
  --ignored --nocapture --test-threads=1
```

Delayed codec I/O is not a real slow/cold disk: it does not simulate source reads,
seeks, fsync or blocked OS calls. The test rejects the delay environment without
the feature and requires actual read/write/completion counters when enabled.
Assertions remain strict about one decode per file in both runs. Resident
pending handoffs are counted separately from completed native disk restores.
Test timing includes scheduling and injected sleeps, not a latency benchmark.

A deterministic cache control stalls the writer, hands off its admitted shared
source with a zero-byte restore pool, checks cancellation/generation exclusion,
releases queued credit and checks final ownership lifetime. After successful
publication, disk restore correctly refuses the same zero-byte restore pool.
The application also checks that a tentative warm neighbour cannot block a
higher-priority cold neighbour under a two-entry RAM limit.

Pixel/color fidelity, physical HDR, GPU transport and general format support
remain governed by their separate qualification reports. This CPU/cache test
does not raise the 100-format catalogue counts or qualify large varied-content
producer collections. Sustained near-limit RAM pressure and real slow storage
remain broader gates; NVIDIA/CUDA remains deferred by the user.

## Final results

| Gate | Result |
|---|---|
| [1024-path collection](collection.log) | 2047 foreground visits, 4091 retained neighbours; 1024 native decodes, 1021 disk restores; three-entry bound; managed peak 483798 bytes, final zero |
| [128-path delayed codec I/O](delayed-codec-io.log) | 255 visits, 507 retained neighbours; 128 native decodes, 124 disk restores and one pending resident handoff; 6498 real stream read calls and 6415 write calls; managed peak 556638 bytes, final zero |
| [Cache regression](cache-regression.log) | 163 passed, five manual tests ignored |
| [Application regression](app-regression.log) | 363 passed, 109 manual/external tests ignored |
| [Production check](production-check.log) | Passed; only existing vendored jxl-color lifetime warning |

Maximum observed foreground cache residency is three entries/286198 bytes in
both collection runs. These are sampled cache observations, not physical RSS
peaks or near-limit RAM-pressure qualification. No native decoder fallback was
needed for any file in either complete traversal. [Result](result.json) and
[source hashes](sources.json) retain exact source/fixture/log pins.

[Fresh Metal regression](metal-regression.log): three viewport controls pass
including snapshot/worker SDR-HDR frame equality and cancellation.
[Current live viewer result](live-result.json): ten guarded GLB native-swap
roundtrips pass on M4 Max/Metal, as does the expected no-cache decoder refusal.
The [positive](live-positive.log.gz) and [negative](live-no-cache-negative.log.gz)
logs are retained losslessly, with readable excerpts alongside them. This
checks that shared pending ownership did not break the actual viewer path.
The [production binary build](production-build.log) also passes.
