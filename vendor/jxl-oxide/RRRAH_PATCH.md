# Local deferred frame initialization

Base: jxl-oxide0.12.6, registry checksum d36c662923f47586880211f3bc7c0d83fb3a9b410d278c7bde93450748abeef3. Original MIT/Apache licenses preserved.

Adds optional try_init_header_only and resume_frame_loading so the caller admits codec working buffers before buffered frame payload is loaded. Existing try_init remains eager; feed_bytes/finalize resume deferred data before processing new bytes. Container parsing occurs only once. This covers tracked codec groups, not all parser/ICC metadata allocations or RSS.

## Optional ICC working admission

`try_init_header_only_with_icc_admission` forwards two absolute high-water ICC
buffer sizes from the same-version patched jxl-color. Callback errors preserve
their original boxed type. Default eager and header-only APIs use a no-op
callback. Encoded/reconstructed/shuffle payload storage is covered; entropy state,
profile parsing/copies, container/header buffers, rendered ICC copies and RSS
remain outside this API. Credit must outlive the initialized image.

## Retained rendered profile

`rendered_icc_with_admission` admits the final profile allocation before copying
raw ICC bytes or allocating synthesized ICC output. Callback errors keep their
original type. Temporary synthesis metadata/tables remain outside this output
credit. The rrrah caller attaches the output reservation to `DecodedRaster`
color metadata, shared by clones and released with the last profile owner.


The ICC admission callback may also repeat at 4096-symbol entropy boundaries for
cancellation. Repeated bounds are absolute high water, never incremental charges.

ICC reconstruction now also polls the same absolute admission high-water bound
through `decode_icc_with_check`, retaining original callback error types. Polls
are cancellation opportunities, not additional allocation charges.

## Initial encoded-header buffer admission

`feed_bytes_with_header_admission` admits planned codestream-buffer capacity plus
old capacity before growth (potential realloc overlap). Geometric growth is
planned explicitly and allocated with `try_reserve_exact`; default `feed_bytes`
retains its previous growth path. A container may emit multiple codestream parts
from one input feed, each considered separately. Admission failure requires
tearing down the uninitialized decoder, as with a failed ordinary feed.

The rrrah caller retains this high-water credit through partial initialization
and deferred initial-frame loading, then releases it after the pending initial
Vec is dropped. Container/auxiliary boxes, inner frame-parser buffering, header/
TOC/entropy/profile metadata and allocator overhead are not covered by this API.
