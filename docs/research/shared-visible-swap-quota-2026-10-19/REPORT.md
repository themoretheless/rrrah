# Expected shared swap quota refusal is not an I/O failure

RasterSwapCache.region_store shares its hard disk quota with visible segmented backing. That backing is externally owned and cannot be evicted from the asynchronous spill worker's LRU. With a single allowed object already occupied, an otherwise admitted spill necessarily receives SwapError::Quota. Previously the worker counted this expected bounded refusal as an error.

The worker now classifies the typed quota error with dropped writes, preserving errors for codec/corruption/filesystem failures. No quota is relaxed, visible backing is not evicted and admission/payload ownership behavior is unchanged. The public counter comments describe the distinction. This changes monitoring semantics for expected quota exhaustion; consumers must not use errors alone to count all unsuccessful spills.

The new integration reproduces errors=1 before the fix. Afterwards: segmented associated-HDR visible data restores exactly under the full object quota; the speculative spill is refused with dropped1/errors0 and its queue credit releases. Dropping visible backing frees the object quota and index reservation. Retrying the same key succeeds, restores exact samples/association, and final pixel/queue roots return to zero. The pending-key state therefore does not poison retry.

149 library tests pass (three ignored), including existing codec-failure and corrupt/truncated restore tests that still count real errors. All four associated-raster integration tests pass. Formatting and whitespace checks pass. Logs and source hashes are retained. This is deterministic shared-quota behavior, not physical slow-disk or GUI endurance proof; it does not explain the earlier gallery switch or presentation stall.
