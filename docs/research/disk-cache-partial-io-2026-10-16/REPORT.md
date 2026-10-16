# Disk RAW cache partial-read cancellation

Both managed and unbudgeted disk-cache pixel payload reads now check cancellation before and after every Read call, retry Interrupted, and reject premature EOF. This replaces chunk-level read_exact, which could retry partial/interrupted reads without observing cancellation inside a chunk. Header parsing and individual blocking OS calls remain separate indivisible work.

New controlled tests cover cancellation on the third Interrupted call and on the third one-byte partial read, successful retry after interruption with one-byte chunks, and truncation refusal. Existing cache cancellation, quota, swap, recovery and ownership tests run in the full library suite (suite.log). This does not establish a physical slow-disk soak or NVIDIA/HDR qualification.
