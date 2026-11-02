# Large float swap: 600-second qualification

Status: passed. 20 complete cycles in 612.069 seconds; terminal exit 0 and monitor validation passed. All 123 RSS samples were available; sampled peak RSS 70205440 bytes (five-second sampling, not a continuous peak). Scratch is empty after teardown and all recorded source hashes still match. See `summary.json`, `state.json`, `test.log` and `rss.jsonl`.

Frozen test binary: SHA-256 `de2d6e8e062dc9f7ba405f19ec798495bae022f845f20d3a319ba749eae11ade`. Source hashes are in `sources.json`. The monitor samples child RSS every five seconds and stops after three consecutive samples above 1 GiB.

Each cycle generates a changing 2048x2048 RGBA32Float raster (64 MiB), writes real temporary files, refuses restoration under full 128 MiB managed-root pressure, cancels after allocation admission and at least 64 KiB of actual payload read, retries and compares every float bit including negative zero and signed HDR values. Root and queue tracked credits must return to zero each cycle. Disk byte cap is 134217856 and entry cap is two. This case has no TTL; independent TTL controls are separate.

The wrapper delays each I/O call by 2 ms and caps each request at 64 KiB. This is injected latency on the local disk, not a measured physical slow-drive model. Managed-root exhaustion is not system-wide memory pressure. This test does not render via Metal and does not prove a physical HDR display or 100-format coverage.

Companion current checks: cache library 151 passed / 3 ignored; cache_stress 5 passed; sustained_swap non-manual controls 5 passed / 5 ignored. The ignored float case is the explicitly monitored process in this directory. Tests cover bounded LRU/accounting, stale generation publication, TTL cleanup, partial-read cancellation, lock cancellation and overloaded delayed writes.

## Current-build delayed-I/O requalification — 2026-10-24

Fresh `sustained_swap` binary passed the existing delayed 64 MiB RGBA32Float workload under the retained monitor. Requested minimum duration 120 s; five complete cycles took 149.481 s because the started fifth cycle was allowed to finish. Root cap 128 MiB, queue cap one 64 MiB frame, disk cap two objects/134,217,856 bytes; I/O wrappers delay requests by 2 ms and limit them to 64 KiB. Each cycle verifies full-pressure refusal, cancellation after allocation and actual 64 KiB payload progress, retry, and every signed/HDR/negative-zero bit. Final root/queue=0; disk bytes/count remain bounded.

Terminal monitor passed, workload evidence valid, exit 0. Test: 1 passed/0 failed/0 ignored. Sampled RSS peak 70,008,832 bytes (66.8 MiB), 33 samples, below the 2 GiB process guard. Raw log, RSS series, frozen binary identity and source hashes are retained in `current-120s-2026-10-24/`. This is a current-build requalification, not a replacement for the older accepted 600-second run or a speed benchmark. Delays are test wrappers on local storage; real physically slow/failing disks remain separate. Sampling does not prove the unsampled absolute peak or indefinite leak freedom.
