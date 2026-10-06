# RAM cache and swap library contract

This document defines the existing ownership boundaries and the next integration
requirements for fast RAW, raster and 3D viewing. It is not a claim of complete
format, device or physical display qualification.

## Dependency boundaries

`rrrah-memory` owns allocation admission, shared immutable buffers and generic
byte-weighted cache policy. `rrrah-swap` depends on it and owns temporary storage,
integrity and bounded restoration. Neither library depends on image formats or
GPU APIs. `rrrah-cache` supplies payload codecs and the asynchronous spill engine.
The viewer owns navigation, prefetch priority and selection of a loading path.

Persistent disk cache and temporary swap have different lifetimes. They must
retain separate quotas, identities and cleanup rules even when sharing codecs.

## Ownership and limits

Allocation reservations precede allocation and remain attached to the last buffer
owner. Cloning a shared buffer must not charge its allocation again. Cache weight
measures resident membership; a shared allocation budget measures retained
buffers, including objects no longer in the cache. Neither measures total RSS.

RAM byte and entry limits are independent. TTL is a fixed deadline from insertion;
hits do not renew it. Expiry prevents new leases without invalidating existing
ones. A live consumer lease protects its resident object from ordinary eviction.
Explicit owner pins are independent of consumer leases. Changing limits must
either return owned victims or refuse atomically when protected objects cannot
fit. Victims retain their allocation reservations until their owners release them.

GPU upload retains its source lease until queue completion, including when no
visible frame is rendered. GPU allocation limits remain separate from CPU limits;
driver staging and physical device retention require additional qualification.

## Spill and restore

Spill admission is nonblocking and reports queued, duplicate, disabled, oversized,
full, busy or stopped-worker outcomes. Queued is not a persistence acknowledgement.
Queue byte occupancy and waiting-command count are independently bounded. Queue
occupancy must not double-charge pixels already reserved under a CPU allocation
root. The active write can retain an object in addition to waiting commands.

Writes stream the payload and publish a handle only after successful completion.
Restoration reserves output memory before allocating, checks length and digest,
and preserves pixel and metadata identity. Memory refusal is retryable and must
retain the stored object; corrupt data must not be returned. Generation changes
cancel obsolete queued writes without deleting completed objects. Cancellation
of an active codec is cooperative at stream I/O boundaries.

`ImageSwapCache::prune_expired` provides explicit idle maintenance without a new
spill or a lookup of each key. Run it outside the render thread because dropping
handles performs filesystem cleanup. Its count measures removed index entries;
an in-flight restore can retain the file and quota until its last handle drops.
With TTL enabled, the spill worker also performs maintenance after one second
without commands. With TTL disabled it uses blocking receive without periodic
wakeups. Active writes delay idle maintenance; sustained writes prune expired
entries before quota admission. Expiry remains exact for lookups, while physical
cleanup is cooperative and can be delayed by active operations or held handles.

## Loading policy

### Object state and failure contract

These are ownership states, not a mutually exclusive per-key enum: a completed
swap handle can coexist with a resident allocation, and several consumers can
share that allocation. A coordinator must track those owners independently.

| State | Owner and accounting | Next transition |
| --- | --- | --- |
| Resident | Shared buffer owns RAM credit; membership has byte/count/TTL policy | Lease, owned eviction victim, or last-owner release |
| Leased | Consumer protects membership; cloned leases share the allocation | Last consumer release permits later eviction; expiry blocks new leases |
| Spill queued / writing | Command owns the payload and queue occupancy; disk quota includes active writes | Completed handle publication, or cancellation/failure releases command ownership |
| Stored | Immutable handle owns disk quota independently of RAM | Restore, index expiry, or final handle release |
| Restoring | Retained handle protects the source; decoded output owns newly admitted RAM | Validated output, retryable pressure/cancellation, or invalid-object removal |

`ImageSwapCache::try_enqueue` consumes the supplied victim even on refusal.
A caller needing a fallback owner must retain one explicitly before calling it;
the API does not return a rejected victim. `Queued` acknowledges admission only.
The viewer must not wait for its write barrier on the render thread.

Memory-pressure refusal retains the stored entry. Cooperative restore cancellation
returns no value and retains it too. A corrupt/failed restore removes only the
same handle that failed: a newer replacement under the same key survives.
Success does not remove the stored copy. This enables repeated restores but
requires independent disk expiry rather than assuming a RAM hit frees disk.

Foreground headroom, equivalent-load joining and automatic restore-versus-decode
selection belong to the coordinator. Child budget ceilings alone do not reserve
foreground capacity. Before adopting a more elaborate cache policy, measure
completed-frame latency, duplicate in-flight allocation, managed peak bytes and
spill backlog under alternating and rapidly reversed navigation.

The foreground worker services RAW, raster and model RAM expiry after one second
without a request when any RAM TTL is configured; otherwise it blocks without
periodic wakeups. Expired unpinned/unleased entries are offered to each cache's
bounded swap queue, or released when swap is absent. Visible/leased entries keep
their ownership protection. Ready foreground work precedes idle maintenance;
active decoding and neighbour preparation can delay this cooperative cleanup.
The idle-receive regression verifies managed model memory returns to zero without
another load, request priority and disconnected-worker termination. The viewer
model-swap regression also routes STL, OBJ, PLY and OFF expiry through the idle
timer: completed writes release managed RAM, and restores preserve payload bytes
without invoking the decoder. This timer-path evidence uses a short interval;
physical one-second worker scheduling is not separately qualified by this test.
A raster timer-path regression uses the real profiled PNG
in both native ICC-bearing and prepared float representations. It verifies one
completed spill, zero managed RAM before restoration, byte-exact complete payload
restoration without decoding/color preparation, and final owner release to zero.
This proves transport preservation, not independent color accuracy or presentation.
The separately executed ignored RAW timer regression uses local EOS R8
`tests/IMG_9043.CR3` (SHA256
`d06cbb10e84882cf130430bdc88ed2652c2622b96dea8d6a854c09e3bee37e59`).
It verifies one completed spill, zero managed RAM before restoration, every
sensor value and metadata field preserved, one successful restore, and final
managed-owner release to zero. Before restoration it also exhausts the shared
RAM root and verifies refusal without a successful read or resident insertion;
after releasing pressure, a cancelled request performs no restoration, and the
same disk key still restores successfully. The independent comparison Vec is test-owned
and outside that budget; managed accounting is not process RSS. Log:
`/tmp/rrrah-idle-raw-pressure.log`. This is native-output transport preservation, not a
new camera decoding oracle or measured navigation/presentation performance.
Viewer regression: 123 passed, 12 ignored, recorded in
`/tmp/rrrah-idle-raw-viewer-final.log` on 2026-10-06. The new ignored RAW
timer test was separately executed successfully as documented above.

Foreground supersession is checked while a worker waits for the decode permit.
The obsolete waiter returns without waiting for the active decoder to finish,
and its ticket teardown preserves the newer request's priority. Dropping the
current ticket while its permit remains live does not admit speculative work;
only permit release does. The dedicated gate regression passes all ten tests
on 2026-10-06 (`/tmp/rrrah-decode-gate-current.log`). This verifies admission and
supersession, not equivalent-load joining or active decoder cancellation latency.

The current explicit `cache-first` and `decode-first` policies remain available.
An eventual automatic selector must compare measured decode and restore costs
for the source representation and recipe, with a conservative fallback and
hysteresis. Extension alone is insufficient. RAM hits reuse buffers; resident GPU
frames reuse uploaded resources. Measuring lookup alone does not establish time
to a completed frame or physical presentation.

Foreground selection has priority over neighbour preparation. Previous/next
counts are navigation-relative, bounded and independently configurable. Obsolete
prefetch generations must release retained buffers and stop consuming foreground
capacity. Joining equivalent in-flight loads, automatic cost selection and their
interaction with memory pressure still require explicit integration evidence.
The app now submits RAW neighbours after raster and model Ready events as well
as RAW Ready. Previously the raster/model branches cleared the RAW command even
in a mixed gallery. The command retains the same navigation window and nearest
ordering, filtering out dedicated raster/model extensions before the RAW worker.
A mixed PNG/OBJ/CR3/NEF/ARW/JPEG gallery regression verifies forward/backward
commands and boundaries. This is scheduling evidence; no mixed-gallery physical
navigation latency is established by the test.
An ignored test was separately executed with local EOS R8 CR3: a selected-PNG
command to the actual RAW worker plans/stores one neighbour, filters the JPEG
candidate without a failure, releases managed worker buffers, and restores every
sensor value and metadata field exactly against native decode. Log:
`/tmp/rrrah-mixed-real-raw-final.log` (one pass). The selected raster paths are
scheduling inputs; this test does not open a UI window or qualify GPU presentation.
For TIFF candidates, the RAW worker now uses bounded source classification before
requesting a sensor recipe. Ordinary raster TIFF completes as a skip rather than
a RAW decode failure; sensor TIFF remains eligible and classification errors are
failures. A live-worker regression verifies one completed ordinary-TIFF check,
zero hits/writes/failures, zero managed peak and no disk cache entry. Viewer:
129 passed, 13 ignored, `/tmp/rrrah-tiff-raw-prefetch.log`.
RAW worker metadata/classification requests now carry the same generation token
as the following native decode. NativeRawDecoder checks cancellation before
format resolution or source opening for both recipe and decode entrypoints.
Cancelled missing CR3/TIFF/NEF/unknown paths therefore return Cancelled rather than
I/O/unsupported errors and allocate no managed memory. This establishes entry
admission, not measured interruption latency inside every camera metadata parser.
Decoder 687 passed/35 ignored, viewer 129 passed/13 ignored:
`/tmp/rrrah-raw-metadata-cancel.log`.
The shared model decode entrypoint also checks cancellation before dispatching
STL/OBJ/PLY/OFF or resolving an unsupported extension. A cancelled absent-source
regression verifies Source(Cancelled) and zero managed allocation for all five
cases. Focused test passes in `/tmp/rrrah-model-early-cancel.log`; this establishes
entry refusal, not measured cancellation latency within an active model parser.
The shared source classifier, decode_image, decode_raster and scientific
decode_raster_with_window entrypoints have a regression for pre-cancelled absent
PNG/TIFF/CR3/PDF/NRRD/unknown sources. All return Source(Cancelled) before source
I/O or managed admission, even with a zero-byte budget. This is a routing contract,
not format decoding qualification. Focused test passes in
`/tmp/rrrah-image-entry-cancel-final.log`.
NativeRawDecoder rejects nonzero sensor image indices before format resolution
and source opening, for both recipe and decode calls. Existing camera backends
retain their own checks. The router regression covers index 1 and usize::MAX for
missing CR3/TIFF/NEF/MRW/unknown sources, preserving the exact requested index in
UnsupportedImageIndex. Cancellation remains the first admission check. All nine
native-router tests pass in `/tmp/rrrah-raw-index-admission.log`. This establishes
explicit refusal for unsupported selections, not multi-image RAW support.
The single-model STL/OBJ/PLY/OFF entrypoint now explicitly refuses nonzero
image_index before source I/O instead of silently decoding the same geometry.
The regression checks indices 1 and usize::MAX for every supported model extension;
the cancellation check still takes priority. Full decoder/viewer regression:
691 passed/35 ignored and 129 passed/13 ignored, `/tmp/rrrah-model-index.log`.
The raster thumbnail color-preparation stage now forwards its existing decode-gate
generation token into the cancellable ICC/linear transform API, matching source
decode and thumbnail row checks. Viewer regression: 129 passed/13 ignored,
`/tmp/rrrah-thumbnail-color-cancel.log`. This verifies compatibility of the wiring;
no deterministic mid-transform thumbnail race or cancellation-latency measurement
was added here. Viewport generation also participates in loader cancellation as described below;
same-intent foreground joining is now wired as described below; independent
subscriber/delivery generations remain pending.
GenerationToken now offers combine(): either owner's generation change cancels
the operation, flattened conditions avoid recursive checking, and repeated equal
owner/expected pairs are deduplicated. The focused regression checks all three
independent owners, retained clones, 100 repeated combinations and conflicting
expectations on one owner. Log: `/tmp/rrrah-combined-cancel.log`. The thumbnail worker now passes its viewport generation into the loader, which
combines it with decode-gate cancellation. Superseding a viewport cancels permit
waiting, native decode, color preparation and preview checks cooperatively;
publication retains its generation guard. A synchronized worker regression
verifies cancellation of the active old window and publication of the new one.
A pre-cancelled missing-source regression verifies zero managed peak and no
result. Full viewer regression: 131 passed, 13 ignored,
`/tmp/rrrah-viewport-viewer.log`. This does not measure interruption latency inside
every parser or establish physical navigation performance. Condition metadata
is outside pixel-buffer budgets.
Display preparation now forwards the request generation token into the existing
cancellable ICC/linear conversion API. A deterministic test supersedes generation
after native decoding of a profiled PNG and a GIF, verifies typed color cancellation
before any new output-memory peak, and confirms all managed source owners release
to zero. The library's row checks remain cooperative; indivisible profile parser
and transform-construction work is not interruptible. Viewer: 125 passed,
12 ignored, `/tmp/rrrah-color-cancellation.log`.
Raster display RAM/swap keys now include both explicit untagged-color assumptions
in addition to source fingerprint, selected image, scalar window and resulting
assumed-color marker. Tests for an untagged TIFF and synthetic scene-linear PFM
verify strict requests cannot reuse prepared pixels from an assumed-color request;
the strict color refusal preserves the original entry and pointer identity, and
all managed owners ultimately release. Viewer: 126 passed, 12 ignored,
`/tmp/rrrah-color-key-final.log`. These ephemeral keys do not require persistent
disk migration.
The swap-path color-policy regression expires an assumed-sRGB TIFF out of RAM,
waits for disk write completion and verifies zero managed RAM. A strict request
performs no successful swap read, attempts its own decoder/color validation and
refuses. The original policy subsequently restores the exact full payload without
decoding, with its assumed-color marker intact; all owners release to zero.
Viewer: 127 passed, 12 ignored, `/tmp/rrrah-swap-color-key.log`.

TIFF neighbours are admitted to the foreground RAM-preload queue despite sharing
the potential-RAW extension set. The worker's content classifier selects raster
or sensor handling. A real ICC-bearing raster TIFF regression without untagged-sRGB
assumptions verifies prepared pixel pointer reuse even when the remaining managed
RAM budget is exhausted; final owner release returns usage to zero. Dedicated
camera extensions retain the separate RAW worker. Viewer suite: 124 passed,
12 ignored, `/tmp/rrrah-tiff-prefetch-final.log` (before the ICC regression).
The Pillow ICC TIFF failure was traced to image's TIFF tag allocation limit being
tied to pixel-buffer size. The decoder now reads ICC metadata under bounded TIFF
defaults before applying pixel limits. A dedicated test verifies profile retention
and successful color preparation. This is a metadata/transport regression, not
independent TIFF color accuracy or qualification of every ICC profile.
Its budget regression separately admits the decoded TIFF with one byte less than
pixels plus the 588-byte ICC profile: profile reservation refuses and all pixel
credits roll back to zero. Successful managed decoding retains both allocations
until the last frame clone drops. Source storage is transient and released before
the final metadata-admission stage; retained byte count is not process peak RSS.
Focused test passes in `/tmp/rrrah-tiff-icc-budget-final.log`.
An authored Pillow TIFF with two RGBA pixels, the same ICC and EXIF orientation
6 independently specifies a 2x1-to-1x2 clockwise result. The native decoder
matches dimensions, pixel order and straight alpha exactly, retains ICC under a
4096-byte managed root, and preserves dimensions/alpha during color preparation.
All managed owners release to zero. The focused regression passes in
`/tmp/rrrah-tiff-orientation.log`; source hash/construction and expected values
are in `tests/fixtures/raster/oriented-tiff-manifest.json`. This checks the
specific orientation/metadata combination, not every TIFF layout or color profile.
Both profiled TIFF fixtures now run in the existing ICC raster swap/Metal test,
alongside PNG/ORA/KRA. On Metal Apple M4 Max, prepared output matches the supplied
sRGB pixel reference within one byte, and readback after native ICC-bearing swap
restore matches the original GPU frame exactly. RAM exhaustion refuses restore
without consuming the disk entry; retry succeeds, ICC is retained and all managed
CPU owners release to zero. Log: `/tmp/rrrah-oriented-tiff-metal.log` (one test,
five fixture cases). This is SDR offscreen readback, not physical presentation or
general ICC-profile qualification.

### Equivalent foreground requests: original integration requirements

Before the same-intent joining implementation below, the foreground queue had
one worker and one pending request. Submitting another request advanced the
cancellation generation even when the path matched;
serialization therefore does not constitute equivalent-load joining. A future
join must compare source identity, page/frame index, scalar window, development
recipe and representation. Path equality alone is insufficient, particularly
when a file changes during decoding. Unsupported or unverified identities must
retain the existing cancellation behavior.

A matching request must transfer result delivery to the latest UI generation
without invalidating the shared decode token. A different request must invalidate
that token and replace pending work. UI delivery generation and decode ownership
generation therefore need separate lifetimes. Errors must reach the latest
subscriber, release all retained buffers and allow a subsequent retry; joining
must not create another allocation reservation for the same shared payload.
Completion racing with submission must neither lose delivery nor publish a stale
frame. Acceptance requires an instrumented decoder invocation count, distinct
page/window/recipe cases, source replacement, failure/retry and cancellation
races under the shared RAM cap, followed by real navigation measurements.

## Acceptance evidence

Current locked library regression on 2026-10-06 passes 164 tests with no failures
or ignored tests: cache 111 unit and five integration, memory 35 unit, one TTL
integration and one doctest, swap nine unit, one expired-spill integration and
one doctest. Command: `cargo test -p rrrah-memory -p rrrah-swap -p rrrah-cache
--locked --target-dir /tmp/rrrah-required-corpus-target`. Log:
`/tmp/rrrah-storage-current-regression.log`. Concurrent restore tests share a
parent RAM cap across distinct child budgets and verify refusal rollback, retained
disk ownership and successful retry. Cancellation tests cover all streaming
checkpoints through final digest verification before publication. These are
library ownership and integrity checks; they do not qualify navigation latency,
physical RSS, GPU presentation or all image formats.

Library checks must cover byte/count/TTL independence, live-lease eviction refusal,
atomic limit changes, shared-budget restoration refusal and retry, bounded spill
admission, cancellation, integrity rejection and reservation release. Integration
checks must compare full payloads and metadata across RAM, disk and swap, including
RAW calibration, HDR float values and 3D geometry. GPU checks must hold source
leases until completion and return tracked usage after teardown.

Performance evidence must distinguish cold and warm storage, decode, restore,
upload, completed GPU frame and physical presentation. Report p50/p95 and managed
peaks under foreground navigation and prefetch pressure. Existing fixture timings
and limitations are recorded in `MEMORY_CACHE_PERFORMANCE.md`; they do not prove
universal format performance, CUDA/NVIDIA execution or HDR display output.

## Optional automatic foreground RAW policy

`RRRAH_RAW_LOAD_POLICY=auto` enables a worker-owned history of at most 64
source/recipe keys. The default remains `cache-first`; explicit `decode-first`
also remains available. RAM hits always reuse resident buffers. Successful decode
and non-RAM restoration durations update separate exponentially weighted means
(three quarters old, one quarter new). Cancelled and failed operations are not
successful cost samples. Policy requires three samples from each path and a
20 percent advantage to change its preference. Every sixteenth request for a key
tries the alternative if RAM misses; it does not decode a duplicate object or
change the stored preference solely because of a probe. Least recently used
histories are discarded; source/recipe changes do not inherit old observations.

Automatic decode admission refusal attempts existing swap/disk restoration before
publishing a load failure. A decode memory refusal temporarily prefers restoration;
periodic alternative probes and successful decode allow recovery when memory
conditions change. Failed decode is not recorded as a successful timing sample.
History is process-local and records combined successful
restore paths; it does not yet distinguish cold storage, disk versus swap, GPU
upload, or physical presentation. Unit/loader regressions validate decision and
existing ownership contracts. The shared foreground recovery helper is qualified with the real X1D 3FR:
128 MiB rejects native input-plus-output admission, while disk and swap restoration
retain exact full sensor and metadata and release managed buffers. Evidence:
`/tmp/rrrah-auto-pressure-real.log`. This is a native helper test, not a physical
window/navigation test. Real navigation latency in this optional mode still requires
end-to-end qualification; fixture tier benchmarks
alone do not establish an automatic-policy performance gain.

## Prefetch count admission after failed neighbours

RAW neighbour prefetch applies the disk count limit to retained/protected
neighbours, rather than the number of source attempts. An absent or failed
closest source does not consume the slot for the next usable neighbour. Once
the configured number of usable neighbours has been retained, the lower-priority
tail is not attempted. The navigation window still independently bounds attempts.
A zero count admits no work; cancellation and byte admission retain their
existing contracts.

`/tmp/rrrah-prefetch-retained-count.log` records real-source regressions: one
missing closest source followed by DSC-F828 retains one validated RGBE entry
under count=1 and returns managed usage to zero; the existing EOS R8 priority
neighbour check continues to stop after its first successful retained entry.
`/tmp/rrrah-prefetch-retained-regression.log`: 109 normal app tests passed,
8 external tests ignored.

## Filmstrip thumbnail managed ownership

When the viewer has a managed CPU budget, thumbnail source decoding and raster
color preparation use that same root. RAW thumbnails reserve bounded RGBA output
before development through `DecodedMosaic::thumbnail_rgba8_managed`; raster
thumbnail generation reserves its RGBA output before allocation. Ready results
carry shared `PixelBuffer<u8>` ownership through queued results and UI handoff
instead of cloning Vec pixels. Dropping stale/consumed results releases credit
after their last owner. Legacy unbudgeted operation remains explicit when no
managed root was configured. Driver staging and physical GPU retention remain
outside this CPU allocation accounting.

`/tmp/rrrah-managed-thumbnail-regression.log`: 110 normal app and 76 core tests
passed, including raster color preservation, zero-budget refusal, RAW preview
pre-admission and output last-owner retention. `/tmp/rrrah-managed-thumbnail-srf.log`
qualifies the real DSC-F828 filmstrip loader: 32 MiB rejects source-plus-output
admission without retained usage; 64 MiB produces a managed 128x96 RGBA result
with exactly 49,152 bytes retained after decoder/source teardown, then zero
after dropping the result. Cooperative thumbnail cancellation on new foreground
intent still needs integration; bounded queues alone do not prove prompt release.

## Foreground cancellation of active thumbnail work

A speculative decode permit now captures its generation cancellation token
under the gate state lock, before foreground intent can be registered. The
filmstrip loader passes it through `DecodeRequest`, checks it while producing
raster RGBA rows, and rejects cancelled results before publication. RAW preview
APIs support cancellation at output-row boundaries; RGBE area reduction also
checks each sensor-cell row. Managed preview cancellation releases its output
reservation, and permit RAII releases decoder admission on every early return.
Unbudgeted RAW previews use the same cancellation-aware development path.

`/tmp/rrrah-thumbnail-cancel-regression.log` records 111 app and 77 core tests
passed, including token invalidation before foreground waits, non-cancellable
foreground permits, zero-allocation pre-cancellation and release after preview
admission. `/tmp/rrrah-thumbnail-cancel-final.log` records six calibration checks
after the RGBE inner-row checkpoint. Raster color preparation retains its
existing cancellation granularity; these checks do not establish physical
foreground latency under every decoder or sustained navigation.

A real-time TTL ownership integration test (ttl_ownership) verifies the boundary
between cache membership and allocation ownership. Once insertion TTL expires,
new leases are refused while active/cloned leases keep readable pixels and block
eviction/limit reduction. Releasing all leases allows expiry pruning; an external
shared-buffer owner still retains the root reservation after cache removal.
New allocation admission remains refused until that last buffer owner drops,
then full budget admission succeeds again. The test uses a one-second TTL and
checks root usage/admission at each transition, independently of decoder format.

## Thumbnail worker lifetime (2026-10-06)

Dropping the thumbnail prefetcher advances its cancellation generation and drains
waiting jobs and ready results under the publication lock. Active loaders observe
cancellation cooperatively; destruction does not join or block on a decoder. A
synchronized regression retains the active token across destruction and verifies
the loader observes cancellation before finishing. The loader also checks its
combined viewport/gate token immediately after permit acquisition, before source
stamp I/O. This narrows admission races but does not make cancellation atomic
with filesystem calls or measure active parser interruption latency. Viewer:
132 passed, 13 ignored, `/tmp/rrrah-thumbnail-shutdown.log`; diff check passes.

A synchronized decode-gate regression now verifies viewport cancellation after
the prefetch waiter has checked an initially live token while a foreground
permit remains held. The waiter finishes without obtaining a permit, before
foreground release, and the foreground gate remains busy. The five-second
receive bound is a test timeout, not a cancellation latency benchmark. This
qualifies gate/token cooperation; the synchronization is injected at the gate
callback rather than inside a native thumbnail decoder. Full viewer regression:
133 passed, 13 ignored, `/tmp/rrrah-viewport-gate-regression.log`.

## Same-intent foreground joining (2026-10-06)

The loader now keeps one mutex-protected flight identity shared with its worker.
Readable equivalent submissions reuse the existing generation without replacing
the pending request or requesting a new foreground ticket. Identity includes
path, size/mtime/sampled BLAKE3 source fingerprint, optional WAL palette identity,
image index, scalar window, development options (including curve), and load-policy
flags. Worker representation is fixed by routing/configuration; there is no
caller-selected representation in this API. Unreadable sources do not join.
This reuses the original delivery generation and original request timing for
identical intent; it does not introduce independent subscribers or delivery IDs.
Fingerprint sampling is not a complete file digest or an atomic source snapshot.
Source sampling currently occurs on the submitting thread; its navigation cost
requires measurement before claiming improved presentation latency.

Completion clears only its own generation, both after worker execution and before
the UI handles terminal events. A stale completion cannot clear a newer flight;
clearing before UI failure handling permits a new retry. Submission and flight
registration share a mutex so concurrent equal submissions cannot enqueue two
requests. Joined requests add no decoded-buffer reservation.

A scheduling regression checks pending and dequeued/active identity reuse, two
concurrent identical submissions, unchanged decoder-gate generation and live
decode token, index/window/development/source changes, stale completion, retry
after terminal clearing, and unreadable-source refusal to join. These are real
loader admission tests with an injected queue, not a live native decoder/UI
completion race or memory-pressure qualification. Full viewer: 134 passed,
13 ignored, `/tmp/rrrah-foreground-join-final.log`. Core: 79 passed,
`/tmp/rrrah-foreground-join-core.log`. Independent latest-delivery semantics,
real decoder completion races, pressure recovery and navigation timing remain
to be qualified against the original requirements.

## Foreground loader shutdown (2026-10-06)

ForegroundLoader destruction now invalidates the decode generation, clears the
shared joining identity and drains its waiting request. Dropping that request
releases its foreground-priority ticket; disconnecting the sender lets an idle
worker terminate. An active decoder still independently owns its permit and
buffers until cooperative cancellation completes. Destruction does not join
the worker or guarantee immediate parser interruption or physical RSS release.
Two regressions verify an unsuperseded active token is invalidated, a newer
waiting token is invalidated, the waiting queue disconnects empty, flight state
is cleared, and speculative admission recovers after retained active owners
release. Full viewer: 136 passed, 13 ignored,
`/tmp/rrrah-foreground-shutdown-final.log`; diff check passes. Native decode
shutdown latency and live UI completion races remain unqualified.

## RAW prefetch lifetime (2026-10-06)

RAW prefetch owns an additional lifetime generation, combined with the shared
decode-gate generation for each command. Destruction closes speculative store
admission, advances only its own lifetime, and drains waiting commands. The
combined token is used for foreground-state waiting, metadata/routing, permit
waiting, native decoding and streaming store cancellation. Other speculative
owners' gate tokens remain live when this prefetcher alone is destroyed. An
already admitted codec observes cancellation cooperatively; existing published
disk entries remain valid, and destruction does not wait for worker completion.
The regression verifies lifetime-token invalidation, unchanged shared generation,
empty disconnected queue and closed store admission. Full viewer: 137 passed,
13 ignored, `/tmp/rrrah-raw-prefetch-shutdown-final.log`. This is ownership/admission
evidence, not measured active RAW shutdown or disk-write interruption latency.

## Partial restore cancellation under memory pressure (2026-10-06)

A new rrrah-swap integration regression stores a 192 KiB patterned object under
a one-object disk quota. Restoration first refuses an exhausted shared RAM root
without charging its output child. After pressure is released, a streamed decoder
allocates the complete managed destination, reads and verifies a 4096-byte prefix,
then cancels before consuming the remainder. Cancelled completion releases both
root and child allocations while preserving the same disk handle and quota.
A subsequent complete restore matches every byte; cloned output remains charged
once until its last owner drops, and dropping the disk handle releases its quota.
The fixture/reference Vec is outside the managed budget. This proves library
transport/ownership and retry behavior, not a format decoder or GPU presentation
contract. Swap regression: 12 passed (9 unit, 2 integration, 1 doc),
`/tmp/rrrah-swap-cancelled-restore.log`; diff check passes.

## Post-lifetime real RAW and Metal verification (2026-10-06)

After foreground joining and worker-lifetime changes, all five hdr_raster_swap
and ttl_swap_metal integration tests pass on Metal Apple M4 Max. Coverage includes
profiled PNG/TIFF/ORA/KRA (five cases), HDR float preservation, corrupt swap
replacement, positive-TTL protected leases, real EOS R8 CR3 restoration and equal
offscreen readback. Three actual-CR3 prefetch regressions also pass: byte-limit
priority, count-limit priority, and shared-RAM pressure/recovery. The SRF-dependent
missing-neighbour regression is excluded because RRRAH_SRF_SOURCE is unavailable;
the initial broad filter's prerequisite failure is retained in its log.
Source hashes, exact commands, logs and exclusions are recorded in
`docs/research/post-cancellation-metal-raw-2026-10-06.json`. These checks do not
qualify CUDA/NVIDIA, physical HDR, active shutdown latency or foreground
completion races.

## Storage regression after XCF integration (2026-10-06)

All 165 current memory/cache/swap tests pass with no ignored cases after the XCF
raster, visibility and opacity work. The command covers 111 cache unit tests,
five cache integration tests, 35 memory unit tests plus TTL integration/doc
coverage, and nine swap unit tests plus partial-cancellation/expiry integration
and doc coverage. Command, log and source hashes are recorded in
`docs/research/storage-after-xcf-2026-10-06.json`. This reconfirms generic storage
ownership/policy behavior; format-specific viewer transport still relies on its
separate native/swap/Metal tests and no physical HDR/CUDA/NVIDIA completion is
implied.

## Staged buffer admission (2026-10-06)

`Reservation::try_buffer` converts already admitted credit into mutable storage
without charging it twice. Conservative excess credit stays attached; extra
capacity must be admitted before initialization. Failure releases the owned
reservation, including its parent credit. Freezing and cloning preserve the
last-owner release rule.

Managed exposure compute now admits CPU output and GPU resource credit before
allocating its CPU output buffer. GPU admission failure rolls back CPU credit;
cancellation after admission occurs before that allocation. This accounts managed
credit rather than process/driver RSS and does not interrupt submitted GPU work.
All 166 storage tests and the actual Metal compute readback/cancellation regression
passed. Evidence: `docs/research/staged-buffer-admission-2026-10-06.json`.

The Metal compute regression also holds a competing GPU reservation while
attempting admission. Refusal preserves that owner's credit and releases CPU
credit. After the competing reservation is dropped, the same budgets successfully
produce exact pixels; cloned output retains CPU credit until its last owner drops.
Evidence: `docs/research/compute-competing-owner-2026-10-06.json`.

## Directional neighbour contract (2026-10-06)

An exhaustive small-gallery test checks 6,528 combinations of gallery length
0..15, selection including out-of-range, both independently configured window
sizes (including zero and usize::MAX), and all three navigation directions.
Its independent distance-based oracle checks membership and nearest-first
priority, including direction reversal and clipping at folder edges. The focused
`rrrah` binary test passes; log: `/tmp/rrrah-neighbour-distance-contract.log`.
This verifies planning, not decoder throughput or live viewer latency.

Application regression after staged allocation and directional planning passes
138 ordinary tests (13 intentionally ignored). The three available real EOS R8
CR3 prefetch tests also pass with explicit ignored-test execution: byte priority,
count priority and shared-budget refusal/recovery. The separate Sony SRF case is
excluded because it needs an external source. Evidence and source hashes:
`docs/research/staged-allocation-app-regression-2026-10-06.json`.

Staged buffer growth is verified through three budget levels with a competing
root owner: expanding 4 bytes of reserved credit into a 16-byte u32 buffer
charges all ancestors and preserves unrelated credit after drop. Arithmetic
overflow before allocation releases root/child/leaf credit. All 39 current
memory tests (unit/integration/doc combined) pass; evidence:
`docs/research/staged-growth-memory-2026-10-06.json`.

A compound TTL/lease/owner-pin regression now exercises expiration while two
consumer leases and an explicit owner pin are active. Expired lookups miss;
dropping the last lease on another thread still preserves the owner pin. After
unpinning, expiry returns an owned spill victim, so budget credit remains until
that victim drops. A new object then fits the same byte/count budget. Focused
test passes: /tmp/rrrah-ttl-last-owner-pin.log. This is ownership correctness,
not a bound on expiration worker scheduling latency.

The compound TTL test uses a one-second expiry rather than a 20ms lifetime
to reduce sensitivity while acquiring its initial lease. It still uses real
time; private policy clock hooks are not exposed through LeaseCache. The full
memory library passes 40 tests including integration/docs; evidence:
`docs/research/ttl-compound-memory-full-2026-10-06.json`.

RGBE managed compute now shares exposure compute's staged admission contract:
CPU output credit is reserved first, GPU credit next, and CPU output allocated
from that reservation last. CPU refusal leaves GPU peak zero; GPU refusal
releases CPU credit before allocation. Both default Metal RGBE tests pass; the
full-size supplied-dump GPU/CPU parity test also re-passes after the production
change. Provenance limitations remain. Evidence:
`docs/research/rgbe-staged-admission-2026-10-06.json`.

RGBE compute now exposes execute_managed_with_cancel; the existing method
delegates with cancellation disabled. Checkpoints cover entry, each4096 input
samples, pre-admission, post-admission before allocation, each encoded dispatch,
pre-submission and completed readback. Cancellation releases owned CPU/GPU
credit. Metal tests exercise early/post-admission/post-readback cancellation
and subsequent successful compute. Log: /tmp/rrrah-rgbe-cancellation.log.
Submitted work/driver waits are not interrupted; viewer integration is separate.

Actual Metal RGBE validation cancellation is also tested with8,320 input
samples: cancellation at the second4096-sample checkpoint returns Cancelled
even when both output budgets are zero; neither budget's peak changes. The
input vector is caller-owned and outside these output budgets. All three
default RGBE integration tests pass; one full-dump case is ignored by default
and was explicitly qualified separately. Log: /tmp/rrrah-rgbe-validation-cancel.log.

## Current combined library verification, 2026-10-06

The locked combined test run for `rrrah-memory`, `rrrah-swap` and `rrrah-cache`
passes 168 tests, with zero failures or ignored tests. This includes the
compound TTL/lease/owner-pin regression and expired-cache streamed spill
ownership regression. The command, suite counts and log digest are recorded in
`research/current-storage-libraries-2026-10-06.json`. This verifies the exercised
library contracts; it does not qualify viewer scheduling, all 100 formats,
physical HDR, CUDA/NVIDIA, or total process RSS.

### Explicit CRW import, swap and Metal preservation (2026-10-06)

The ignored `crw_swap_metal` integration test was explicitly executed on Metal
Apple M4 Max against pinned EOS 10D and table-zero data. Full sensor/metadata
and the 128x96 GPU frame survive swap exactly; managed RAM reaches zero after
spill and final restoration release. Root-pressure refusal leaves the stored
object retryable. This is native result preservation, not an independent color
or physical-display oracle. Automatic CRW viewer routing is still absent.
Report: `research/crw-swap-metal-2026-10-06.json`.

### CRW lease TTL/size/count to swap and Metal (2026-10-06)

The real 10D import now exercises a one-entry byte-limited lease cache before
swap. Protected replacement and tighter byte/count limits refuse. Expiry blocks
new lookups, but cloned leases retain the sensor and unchanged Metal render.
Last lease release yields the owned victim; completed swap releases RAM and
restoration preserves all pixels, metadata and frame after pressure refusal.
Executed on Metal Apple M4 Max. This is explicit composition, not automatic
CRW navigation/preload. Report: `research/crw-ttl-swap-metal-2026-10-06.json`.

### Four CRW orientations survive swap and Metal (2026-10-06)

The actual-Metal CRW integration now renders Normal/Rotate90/Rotate180/Rotate270
variants before and after bounded swap, comparing complete frame bytes, sensor
pixels and metadata. Five writes/reads complete; shared source credit stays
owned until last release and final managed RAM is zero. The combined pinned
D30/10D sensor plus Metal gate passes. This verifies transport/presentation
preservation, not independently correct rotated pixels or color. Current logs:
`research/current-crw-internal-metal-2026-10-06.json`.

### Aggregate restore cap across budget replacement

ImageSwapCache::set_restore_budget does not reset restore_bytes accounting.
Live allocations from retired budget generations remain charged against the
same aggregate limit, including cloned payload owners. Old allocations retain
their original shared parent; new allocations use the replacement parent and
only the remaining aggregate allowance. Restoration admission is serialized
per swap cache to prevent concurrent callers spending the same allowance.
Retired zero-usage budget generations are pruned on subsequent replacements.
The temporary allowance failure is normalized to aggregate live occupancy and
the smallest configured local/ancestor cap when retired owners cause pressure.
This permits foreground release of unpinned old frames while a zero ancestor
cap or an intrinsically oversized object remains an impossible admission.
Releasing the last old owner allows retry without discarding the swap entry.
This changes the formerly incorrect behavior
that allowed rebinding to bypass the local cap; throughput under concurrent
restoration has not been benchmarked after this correction.

Restore admission wait checks request cancellation before trying the mutex and
between one-millisecond retries. A cancelled waiter returns without reading
the swap payload or waiting for the active restoration to finish. The interval
is an implementation polling interval, not a measured latency guarantee.
