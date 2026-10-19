# Local deferred frame initialization

Base: jxl-oxide0.12.6, registry checksum d36c662923f47586880211f3bc7c0d83fb3a9b410d278c7bde93450748abeef3. Original MIT/Apache licenses preserved.

Adds optional try_init_header_only and resume_frame_loading so the caller admits codec working buffers before buffered frame payload is loaded. Existing try_init remains eager; feed_bytes/finalize resume deferred data before processing new bytes. Container parsing occurs only once. This covers tracked codec groups, not all parser/ICC metadata allocations or RSS.
