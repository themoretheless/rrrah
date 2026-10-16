# Explicit edit writer unlock

The previous viewer parallel run failed acquiring an edit lock immediately after a completed write. The guard previously relied solely on closing its file descriptor. A duplicated descriptor retains that lock after the guard closes its descriptor on this Mac; transient descriptors inherited during another test process spawn are a plausible explanation for the parallel failure, not independently proven.

Added a deterministic regression retaining File::try_clone while dropping the writer guard. It failed before the fix with WouldBlock, and passes after explicit File::unlock in Drop. The lock inode remains persistent and competing writers remain refused while the guard lives. Unlock errors are logged; file closure remains the fallback.

Validation on current Mac: edit_store suite 12 passed / 1 ignored, including process-kill recovery; full parallel viewer 273 passed / 72 ignored. A single passing full run does not prove the absence of every concurrency race. No physical HDR/NVIDIA or ignored Metal coverage is claimed.
