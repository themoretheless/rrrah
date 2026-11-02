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
