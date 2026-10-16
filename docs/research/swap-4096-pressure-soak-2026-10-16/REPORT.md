# 4096-object sustained swap pressure

The retained ignored soak accepts RRRAH_SWAP_SOAK_OBJECTS and RRRAH_SWAP_SOAK_SECONDS; default 256 objects / 60 seconds is unchanged. Run: RRRAH_SWAP_SOAK_OBJECTS=4096 RRRAH_SWAP_SOAK_SECONDS=120 cargo +stable test -p rrrah-cache --lib sustained_collection_restores_under_full_memory_pressure --offline -- --ignored --nocapture.

Actual result: 120.204 seconds, 4096 synthetic objects, 647,980 cycles / 1,295,960 successful restores; every restored RGBA8 pixel checked (84,932,034,560 pixel bytes). Each cycle also checked refusal while a previous result clone holds the remaining memory, immediate cancellation, and successful retry after dropping the owner. Disk object count/bytes remained fixed; errors and queued bytes zero. Tracked root peak 131,072 bytes / final zero. Shutdown removed all temporary store files.

This is a local-filesystem synthetic swap soak with OS caching, not 4096 unique photographic files, physical slow-disk latency, total RSS, TTL expiration, or GPU/display proof. The configured limits and separate small tests cover other policies; they are not inferred from this run.


Reproducibility follow-up: ordinary cache library 149 passed / 3 ignored. Configured 4-object one-second run passed; separate 11-second run emitted periodic progress and completed successfully. The retained soak now reports elapsed seconds, object/cycle/read counts and current/peak tracked credits approximately every ten seconds, sampled every 1024 cycles. Defaults and correctness assertions remain unchanged. Logs: cache-suite.log, config-smoke.log, progress-smoke.log.
