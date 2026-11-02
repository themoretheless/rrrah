# Sustained mixed-worker failure — 2026-11-02

The requested10800-second run terminated with a real test failure, not a monitor
timeout or RSS guard. Monitor and child handles were polled to terminal status:
child exit101, monitor exit1, workload evidence invalid. Preserve the original
[state](initial/state.json), [log](initial/test.log), [RSS samples](initial/rss.jsonl)
and [source pins](initial/sources.json).

Metal Apple M4 Max completed1152 logged selections over181.9 seconds, then at
selection1156 refused retention of nearest neighbor032.bmp. Root usage in the
assertion was83,886,080 bytes under a134,217,728-byte root. This does not prove
all sub-pools had headroom or identify the cause. Test elapsed187.26 seconds
includes setup; requested three-hour duration was not achieved.

The preceding finite minute passes remain valid for their executions but do
not prove sustained correctness. Source-level investigation added test-only
diagnostics distinguishing load refusal from successful loading without a
consumer lease. No fix for this sustained failure is claimed here. Inspect any
subsequent process handle before deciding whether to restart it.

## Diagnostic reproduction and candidate repair

The900-second diagnostic run also terminated with exit101, after4224 logged
selections/690.6s; selection4228 refused nearest032.bmp. Its new diagnostic
explicitly reports `swap restore memory admission refused`, rather than a
successful load losing its lease. The error still appeared at83,886,080 root
bytes, so the root total alone did not establish sub-pool availability. Original
[log](diagnostic/test.log), [terminal state](diagnostic/state.json),
[RSS samples](diagnostic/rss.jsonl) and [source pins](diagnostic/sources.json)
are retained. The900-second target was not achieved.

The candidate fixes two distinct handoff gaps:

- Retained prefetch now passes its first successful RAM-hit lease directly to
  the worker instead of dropping it and requesting a second consumer lease.
  A regression crosses a real TTL deadline: new consumers still refuse expiry,
  the earlier lease remains valid and blocks replacement, and releasing it
  restores admission. This is a separate correctness gap, not the diagnosed
  error message in the second run.
- An unpinned cache victim may still be owned by an asynchronous cancelled
  writer. `ImageSwapCache::wait_idle_with_cancel` offers an interruptible fence,
  including when its bounded command queue is full. After exhausting unpinned
  LRU victims, an eligible prefetch waits for outstanding spills once and retries
  the same restore. If the writer already drained between checks, one immediate
  retry covers that race. Cancellation or an individually oversized request
  does not start this wait; a new foreground generation can interrupt it.

A gated-codec regression reproduces restore-pool exhaustion while a cancelled
spill still owns the last restored allocation, then proves fence completion
releases the root/queue and permits exact restoration. Two queue states test
that cancellation returns while the writer is still blocked and leaves its
owners intact. Both existing queue APIs and subsequent writes remain usable.

Current finite checks pass: [159 cache tests/5 ignored](fix-candidate/fence-cache-tests.log),
[65 prefetch tests/22 ignored](fix-candidate/fence-prefetch-tests.log), and
[production check](fix-candidate/fence-production-check.log). Formatting and
diff whitespace checks also passed. A fresh10800-second monitored current-binary
run was launched after both old handles were confirmed terminal. Its completion
is not claimed here; the two sustained counterexamples remain open until
long-run evidence establishes the candidate's behavior.

## Cold-decode counterexample and recovery

The fenced repetition also failed: selection6802 refused nearest035.bmp while
preparing a cold source, after6784 logged selections/1069.9s. The error now
specifies a 16,777,216-byte reservation with 121,634,816 bytes used against a
134,217,728-byte limit; after temporary decode buffers unwound, the assertion
reported 117,440,512 bytes. This was not the preceding swap-restore refusal or
a monitor RSS stop. [Original log](fenced/test.log), [terminal state](fenced/state.json),
[RSS](fenced/rss.jsonl) and [source pins](fenced/sources.json) are retained.

Retained cold raster loads previously reclaimed unpinned victims only for
foreground requests. They now do so for retention-admitted background candidates
as well, and fence outstanding cancelled spills once if necessary. Count/byte
protection is checked first; cancellation or individually oversized allocations
do not reclaim victims. Source-headroom refusal now has a typed recovery flag
while retaining its prior user-visible message. Protected current/neighbor
leases remain protected. No limits or TTLs were increased.

A controlled 32-byte regression fills the root with selected and stale 16-byte
frames. An admitted cold candidate fails once, releases only the stale frame,
then succeeds with a retained lease. The selected frame remains resident.
Cancellation during the failure and a 48-byte indivisible request each stop after
one attempt without removing the old frame. All branches finish at zero managed
usage. Current [66 prefetch tests](cold-retry/prefetch.log) and
[production check](cold-retry/production-check.log) pass.

A fresh10800-second run of the rebuilt candidate was started only after the
prior monitor and child were confirmed terminal. Its pending result is not
recorded as a pass. The three finite counterexamples and the broad goal remain
open until sufficient subsequent evidence is available.

## Real worker with delayed swap codec I/O

An explicit `qualification-slow-io` feature instruments the generic image-swap payload stream. The real foreground worker exercises delayed/short codec reads and writes through normal RAM/swap admission, cancellation, spill fences and rendering. The normal production feature graph excludes this feature ([feature check](slow-io/production-features.log)); [production check](slow-io/production-check.log) passes. No default runtime delay, counter or codec adapter is introduced.

The latest required actual Apple M4 Max [run](slow-io/test.log) passes290 selections over300.107 workload seconds. Each instrumented codec I/O call waits1ms and permits at most16KiB. The64 authored1024-square PPM/TGA/BMP sources prepare to16MiB frames; fixed pressure64MiB, root128MiB, RAM32MiB/two entries/250ms TTL, restore32MiB, swap64MiB/three objects and queue16MiB/one item are unchanged. All seven windows occur, including unbounded ahead and both directions; their selection counts are[48, 48, 48, 48, 34, 32, 32].240 nearest neighbors retain their leases,3175 farther candidates refuse at protected capacity. Selected CPU digests and full63x65 SDR/HDR references match on every selection. Root peak128MiB, settled maximum96MiB, final root0.

The process-wide counters are sampled only after the tested loader/cache workers join; the case requires a fresh process. Actual totals:119227 read calls/1,560,287,104 bytes,135811 write calls/1,777,493,632 bytes, maximum request16384 bytes in both directions.97 spill objects were published into the index and93 restores completed checksum verification and decode. Byte totals include metadata and any partial/error/unpublished transfers; they do not imply every attempted operation completed or every restored object was retained. This establishes full bulk reads in addition to publication, rather than accepting metadata-only read attempts. It is not a bandwidth or before/after speed measurement.

The monitor reaches terminal passed/exit0 with exact injection confirmation, both completed I/O directions and bounded byte totals: [state](slow-io/state.json), [source hashes](slow-io/sources.json), [RSS samples](slow-io/rss.jsonl).32 ten-second samples have maximum321519616 bytes; this is a sampled maximum, not an assertion of unsampled peak or OS exhaustion. MallocNanoZone=0 and the6144MiB RSS guard are recorded. Test305.95s and monitor320.34s include other lifecycle/sampling time and are diagnostic.

[Cache suite](slow-io/cache.log):164 passed/5 ignored with the feature enabled and no injection requested. Two [cancellable spill-fence cases](slow-io/cancel-fences.log) run with1ms/16KiB injection and pass. Six [monitor test methods](slow-io/monitor-tests.log) pass, including synthetic/replayed controls for missing/wrong/duplicate injection markers and missing/zero/oversized/inconsistent I/O directions and byte totals. A previously successful worker log without counters cannot satisfy this gate. These parser controls are not hardware evidence. Isolated adapter tests verify exact byte/request counts, no accounting when unconfigured, short transfers and propagated errors. Formatting/whitespace checks pass; [check pins](slow-io/checks.json). The earlier presence-only positive artifacts were replaced by this counter-gated run.

```sh
cargo test --locked -p rrrah --bin rrrah --features qualification-slow-io --no-run
# Use the executable printed by that command:
MallocNanoZone=0 python3 scripts/run-metal-collection-soak.py --suite worker-mixed --binary /absolute/test-binary --output /fresh/evidence --scratch /fresh/scratch --seconds 300 --rss-limit-mib 6144 --sample-seconds 10 --swap-delay-ms 1 --swap-chunk-bytes 16384
```

Latency is injected at codec calls over real checksum-verified swap files. The store may buffer/coalesce requests;16KiB is not a physical-device syscall bound. Source-file reads, seek/fsync behavior and physically slow disks are not modeled. This finite delayed-worker proof neither completes the separate10800s repetition nor proves physical HDR/NVIDIA qualification.
