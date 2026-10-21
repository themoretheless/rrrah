# Group bounds potential — diagnostic only

The frozen decoder probes aligned groups of 8 and 16 retained fragments. Instrumentation was removed after profiling; no group index is enabled in production.

`counts.json` and `profile.log` record optimistic preflight opportunities. Queries can count a rejected boundary twice; the terminal query can exceed the final work debit. The two group sizes overlap and must not be added together. Summaries were recomputed diagnostically, so these counts are not timing gains or actual saved work.

All four source attempts still exhaust the unchanged 100M work budget. A useful implementation would need to reuse summaries across unchanged runs and preserve exact geometry, admission, cancellation and rollback. This report proves neither completed AI rendering nor viewer/GPU/HDR behavior.
