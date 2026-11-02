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
