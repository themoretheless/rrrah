# OpenMosaic: единый путь открытия RAW

Status: accepted design draft for Epic 3. This document describes the
application use-case boundary. Stable file snapshots, the V3 filesystem
object store and global memory admission remain separate Epics 6, 5 and 7.

## Problem statement

The current application independently implements the same operation in three
places:

```text
inspect      -> fingerprint -> cache lookup -> decode -> synchronous store
foreground   -> fingerprint -> cache lookup -> gate -> decode -> async store
RAW prefetch -> fingerprint -> presence probe -> gate -> decode -> store
```

The copies have already drifted. Inspect treats cache corruption and write
failure as fatal, foreground falls back for every cache error including
resource failure, and prefetch counts cancellation as failure. Startup and
drag-and-drop also resolve paths differently. A dropped RAW excluded by the
10,000-item truncation can currently open item zero instead, and folder scans
run on the winit thread.

Epic 3 replaces duplicated mechanism with one headless `OpenMosaic` use case
and one asynchronous `OpenTargetResolver`. UI, GPU, queues, telemetry and
persistence scheduling remain policies outside the coordinator.

## Dependency direction

`rrrah-open` is a new lightweight crate that depends only on `rrrah-core`.
It owns use-case contracts, cancellation, coordinator logic and the
latest-wins mailbox. It never imports winit, wgpu, Rawler,
`DiskMosaicCache`, `SourceFingerprint`, `CacheKey`, HUD telemetry or a thread
runtime.

```text
rrrah-core
    ^
    |
rrrah-open        application ports and coordinator
    ^  ^
    |  |
legacy cache / Rawler adapters
    ^
    |
rrrah-app         target resolution, scheduling, UI and GPU commit
```

Adapters may live in the infrastructure crate that owns their concrete type
or in a narrow app composition module. V2 types are forbidden in coordinator
contracts.

## Public application contract

The UI-facing port is object-safe and does not expose the prepared-source
type, cache key type or backend error type:

```rust,ignore
pub trait OpenMosaicPort: Send + Sync + 'static {
    fn prepare(
        &self,
        request: OpenMosaicRequest,
        cancellation: CancellationToken,
    ) -> Result<OpenMosaicOperation, OpenPrepareError>;
}

pub struct OpenMosaicRequest {
    locator: RawLocator,
    image_index: ImageIndex,       // canonical u64
    cache_intent: CacheIntent,
}

// Construction is intentionally through validated builders so adapters cannot
// smuggle backend keys, unbounded diagnostics or an unchecked platform index
// across the application boundary.
```

`RawLocator` is an address, never an identity or stability proof. The legacy
backend may reopen its path and must be labelled accordingly. Epic 6 changes
only backend composition: one private resolver returns an opaque, owned,
non-Clone `PreparedRaw` used by identity, cache and decoder and consumed by a
post-use stability check. No downstream UI or queue API changes.

`ImageIndex` stores `u64`. A legacy/Rawler adapter performs one checked
`usize::try_from` and uses that exact value for both legacy key derivation and
decode. V3 retains the original `u64`.

Cancellation is non-optional. Inspect uses `CancellationToken::never()`.
Foreground and prefetch receive tokens owned by the scheduler. Cancellation
is a terminal outcome, never an error or failure counter.

Opening is deliberately two-phase. `prepare()` is called by the submitting
thread and immediately announces interactive/speculative decode intent; it
does no filesystem or decode work. It returns a non-Clone, `Send + 'static`,
one-shot `OpenMosaicOperation`. The latest-wins mailbox owns that operation
and its cancellation source, and the worker consumes it with `run()`. Dropping
an unrun operation releases its RAII admission intent. This prevents a queued
prefetch request from taking the decoder lane between foreground submission
and eventual worker dequeue.

```rust,ignore
#[must_use]
pub struct OpenMosaicOperation { /* boxed one-shot job */ }

impl OpenMosaicOperation {
    pub fn run(self) -> OpenMosaicReport;
}
```

## Internal ports

The generic engine is hidden behind `OpenMosaicPort`. It has coarse-grained
ports for one prepared source, cache policy and decode. Ports operate once per
stage, never once per byte or sample. The decoder recipe is captured once when
the engine is constructed.

```rust,ignore
pub trait SourceResolver: Send + Sync {
    type Source: Send;
    fn prepare(
        &self,
        locator: &RawLocator,
        cancel: &CancellationToken,
    ) -> Result<Self::Source, SourceFault>;
    fn revalidate(
        &self,
        source: &Self::Source,
        cancel: &CancellationToken,
    ) -> Result<(), SourceFault>;
}

pub trait DecodePort<S>: Send + Sync {
    fn manifest(&self) -> MosaicRecipeManifest;
    fn decode(
        &self,
        source: &S,
        image_index: ImageIndex,
        cancel: &CancellationToken,
    ) -> Result<DecodeProduct, DecodeFault>;
}

pub trait CachePort<S>: Send + Sync {
    fn lookup(
        &self,
        source: &S,
        image_index: ImageIndex,
        recipe: MosaicRecipeManifest,
        intent: CacheIntent,
        cancel: &CancellationToken,
    ) -> Result<CacheLookup, CacheFault>;
}
```

The injected admission adapter has two RAII phases: `announce(priority)`
returns an intent during `prepare()`, and the worker later consumes/borrows the
intent to acquire a lease during `run()`. The lease is held only around the
decoder call and is dropped before persistence, filesystem synchronization or
GPU work. This keeps inspect immediate, foreground priority-aware and prefetch
low priority without putting a thread or queue inside `OpenMosaic`.

## Pipeline and invariants

```text
accepted
  -> prepare source
  -> one identity/key lookup
  -> verified hit ---------------------> source revalidate -> Ready
  -> miss -> wait for decode admission
          -> revalidate miss/presence
          -> peer hit -----------------> source revalidate -> Ready
          -> decode one prepared source
          -> source revalidate
          -> Ready + one-shot StorePlan
```

Cancellation is checked before and after every blocking or externally
implemented stage. A decoder may be non-preemptible, but its result is dropped
after return when the token is cancelled. A cache hit is also revalidated
before publication. In the legacy backend this is only a best-effort metadata
check and is not described as a stable snapshot.

The first miss may return an opaque, bounded presence-recheck token so the
backend does not recompute source identity or key. No filesystem or payload
read is performed while holding the expensive-decode lease. Avoiding the
remaining publication window requires an in-memory/per-key single-flight
handoff; a blind second full load under the global decoder lease is forbidden.
A warm hit performs one lookup only.

The coordinator never executes persistence. A decoded miss returns a
non-Clone, consuming, `Send + 'static` `StorePlan` which captures the backend
target and key but borrows neither source nor coordinator. Foreground first
publishes the frame and moves the plan to a bounded write worker; inspect may
execute it synchronously but reports persistence separately from open-to-ready;
prefetch executes it under its own store admission. A V2 hit never produces a
V3 store plan.

The complete frame is shared as `Arc<DecodedMosaic>`. Sharing only its pixel
`Arc<Vec<u16>>` is insufficient because cloning `DecodedMosaic` deep-copies
metadata strings and vectors.

## Typed outcomes

The coordinator returns one report to inspect, foreground and prefetch.
Presentation differs; classification does not.

```text
terminal:
  Ready(frame, provenance, timings, optional StorePlan)
  PresentUnverified(cache provenance)       # prefetch only
  Cancelled(reason, stage)
  Failed(Source | Decode | Resource | Contract)

diagnostics:
  cache rejected/corrupt
  cache incompatible
  optional cache unavailable
  quarantine/invalidation failure
```

Rules:

- corrupt, incompatible or attacker-oversized cache data is rejected before
  allocation and falls back to source decode with a diagnostic;
- failure to reserve memory for an otherwise valid operation is a resource
  failure and must not trigger an unbudgeted decode fallback;
- optional cache I/O/permission failure may decode but normally suppresses an
  immediate write retry;
- store failure never revokes a usable `Ready` frame;
- cancellation is not a failure and creates no store plan;
- a presence probe is never called a verified hit.

Detailed counters and the HUD state machine are implemented in Epic 4, but
Epic 3 preserves these typed distinctions.

## V2 and V3 policy

Compatibility is hidden behind a composite cache adapter. The coordinator is
unchanged across rollout modes.

Target ordering after Epics 5 and 6:

```text
RAM exact MosaicKey
  -> preferred V3 schema
  -> older supported V3 schemas
  -> explicitly recipe-gated V2 fallback
  -> fresh decode
  -> write preferred V3 only
```

V2 uses sampled identity and a native-width frame field. It must never be
relabelled, copied or re-encoded as V3, and a V2 hit must not create a V3 write
plan. V3 corruption is not silently hidden by a V2 downgrade. V2 and V3 roots,
parsers, pruning and epochs remain disjoint.

Epic 3 initially composes the current legacy-only adapter. It does not derive
`SourceId`, claim V3 safety or expose `SourceIdHasher`. Epic 5 swaps in the V3
store and Epic 6 supplies stable identity.

## Latest-wins scheduling

The current `fetch_add -> drain receiver -> try_send` pattern is not
linearizable with multiple producers and can also cancel an active request
before discovering that the new request was not accepted. It is replaced by a
single-mutex mailbox with this bounded state:

```text
next ticket
current ticket + cancellation source
pending request: at most one
active ticket: at most one
ready result: at most one
closed/worker liveness
```

Ticket allocation, current replacement, cancellation of the old operation,
pending replacement and stale-ready removal are one critical section. Slow
destructors run after unlocking. The counter uses checked increment and returns
`GenerationExhausted`; it never wraps and cannot create ABA.

Identical current/pending demand is coalesced before a new operation or
diagnostic is allocated. Cancellation is a single atomic state change. No
backend callback, I/O, observer, arbitrary destructor or user-provided closure
runs while the mailbox mutex is held.

The worker may physically finish stale non-preemptible work, but publication
accepts only the current ticket. GPU publication is two-phase:

```text
begin_ready_claim(ticket) under mailbox mutex
  -> build/upload an unbound GPU candidate outside the mutex
  -> commit_ready_claim(ticket) under the same mailbox state
```

The second check rejects a ticket superseded during upload and drops the
candidate without changing the renderer's active bind group or logical UI
state. `RawRenderer` therefore prepares texture/bind-group/parameters into an
owned candidate and exposes a short consuming commit operation. The ready slot
is bounded, replacing the current unbounded channel that can retain multiple
full mosaics.

Worker death, queue closure and generation exhaustion are typed submission
errors. A failed submission changes neither current ticket, displayed path,
gallery selection nor prefetch state.

Mutex poison fails closed: the mailbox marks itself dead/poisoned, cancels
current work, wakes waiters and never resumes normal transitions from a
possibly partial mutation. A worker sentinel reports panic/death and wakes the
consumer. Port and store jobs are caught only at persistent-worker boundaries;
RAII intent/lease/active/store guards have infallible, non-I/O `Drop`.

## Target resolution and gallery transaction

CLI startup, no-path startup, drag/drop, picker and gallery navigation use one
typed resolver. Filesystem work never runs in a winit callback.

```text
OpenTarget: None | Path | CatalogAsset
OpenOrigin: Cli | Drop | Picker | Gallery | Inspect
ResolveEvent: Idle | Progress | Resolved(OpenPlan) | Failed | Cancelled
OpenPlan: Single | Gallery(catalog, selected AssetId)
```

Origin affects presentation and telemetry only, not security or format policy.
An explicit path is validated and anchored before its parent is scanned. A
bounded catalog must retain that anchor; it may never substitute item zero.
If the selected asset cannot be retained, it opens as a one-item catalog.

Directory scanning is cancellable and bounded before collection/sort. Ordering
has a deterministic total tie-breaker, including non-UTF-8 names. Empty,
unreadable and partially unreadable folders are distinct typed results. A
single format registry supplies extension hints; explicit files still require
decoder probing.

Quotas cover visited entries, retained items, retained path/name bytes,
progress snapshots and work time/checkpoints. Diagnostic count and diagnostic
message bytes are bounded as well; backend paths and unbounded error strings
never enter default telemetry. Final-component symlinks and non-regular inputs
(FIFO/device/socket) are rejected consistently for every origin. Parent-chain
and handle-level no-follow protection is completed by Epic 6.

The app retains the committed frame/session while a new target is pending.
Only a successfully admitted, current transaction may replace catalog,
selection and displayed-source labels. A failed decode cannot relabel the old
frame. Selection is tracked by stable `AssetId`, not only by vector index.

## Performance contract

Measured on the current Apple M4 Max:

```text
64 MiB full SourceId hash: approximately 54 ms p95 at 1181 MiB/s
RecipeId:                 approximately 211 ns p95
ArtifactKey:              approximately 312 ns p95
```

A 52.7 MP U16 mosaic is about 100.45 MiB. A second full payload pass can add
roughly another 85 ms at the measured memory throughput. Consequently:

- full source identity is preloaded/memoized per stable source session;
- a source/key is derived once per operation;
- V3 payload encoding/digest uses one streaming pass;
- cache `usage()` and prune scans never precede first open;
- usage is maintained by store/catalog deltas;
- source byte count travels in the result instead of a UI-thread `stat`;
- open-to-ready ends before asynchronous persistence;
- cache-hit and decode histograms remain separate.

Coordinator virtual dispatch occurs once per coarse stage and is negligible.
Virtual calls per byte/sample, async-trait boxing per chunk and deep mosaic
clones are prohibited.

## Verification gates

Coordinator tests use scripted ports, barriers/rendezvous channels and stable
stage checkpoints rather than sleeps. Required invariants include:

- hit calls lookup once and never admission/decode/store;
- miss derives one identity/key, revalidates after admission and decodes once;
- cancellation at every checkpoint prevents later publication/store;
- decoder and identity observe the same prepared-source instance;
- post-use validation failure suppresses both cache write and `Ready`;
- `StorePlan` is consuming, non-Clone and executes at most once;
- outcome and writer share the same frame via `Arc::ptr_eq`;
- mailbox depth is `active <= 1`, `pending <= 1`, `ready <= 1` under 10,000
  submissions and multiple producers;
- a stale result cannot claim UI commit;
- worker/result/store queue death is typed and leak-free;
- near-`u64::MAX` submission fails without wrapping;
- CLI/drop/picker resolve the same asset identically;
- an explicit 10,001st file is retained and selected;
- million-entry fake scans stay within configured memory/work quotas;
- no-target viewer is Idle and no-target inspect is a typed error.

Loom is optional when every mailbox transition stays under one mutex. A pure
reference model exhausts short submit/take/publish/claim/close traces. Real
benchmarks additionally gate that coordinator overhead is insignificant and
that warm V3 read/write traverses payload bytes once.

Interactive benchmark runs emit JSONL as the gate artifact; the HUD is
observational only. Each sample records fixture digest, decoder/recipe, worker
count, tile size, quality tier, cache state, queue wait, RSS high-water and
resident GPU bytes. Embedded JPEGs, mixed RAW formats and different decoder
profiles are not comparable. Initial M4 Max/Metal targets are coordinator p95
<100 us (p99 <250 us), mailbox submit p95 <50 us with cancellation propagation
<1 ms, validated local-NVMe hit p95 <10 ms for <=128 MiB, and first-visible
52 MP RAW p95 <150 ms after memoized identity, with zero stale GPU commits.
Memory runs must remain within configured budget +5% and prove `Arc::ptr_eq`
for promoted frames; other adapters use baseline-relative thresholds.
