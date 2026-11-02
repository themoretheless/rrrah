# Empty thumbnail window: managed result ownership

The existing thumbnail scheduler already advances its generation and drains queued jobs and ready results when submitted an empty window. This follow-up checks the combined ownership boundary with real managed pixel buffers and the actual background worker.

For each of 32 independent worker lifetimes, a 4 KiB result is already published, another 4 KiB result is held inside a blocked loader and a third job is queued. Channel handshakes establish these states without timing sleeps. An empty submission must cancel the active token, remove the queued job and synchronously release the published result: root usage falls from 8 KiB to 4 KiB. A fresh submission immediately follows. The old loader deliberately returns its cancelled result, exercising scheduler-side disposal; the next loader may run only after the stale result is dropped. The sole fresh result contains exact expected bytes. Dropping it and joining the worker must leave zero root usage. Each lifetime peaks at exactly 8 KiB under an 8 KiB cap.

No production change was required. This is a deterministic managed-allocation/concurrency gate with authored thumbnail payloads, not real codec decode, RSS, disk swap, TTL, GPU or physical display qualification.

Validation: focused gate 1 passed / 0 ignored / 0 failed; full app gate 296 passed / 84 ignored / 0 failed in 4.10 seconds excluding compilation. Hardware/manual ignored gates were not exercised. Source whitespace check passed. Logs and source hash are retained alongside this report.
