Current follow-up: [native VM integration and remaining scope](VM.md). The text below records the earlier prerequisite revision.

# Graphics checkpoint bridge for native PostScript save/restore

Current scope: an admitted `EpsGraphicsCheckpoint` primitive. PostScript VM
`save`/`restore` are still unsupported. This primitive is not a substitute for
VM rollback, save tokens or graphics save anchors. No new AI/format count or
hardware qualification is claimed.

## Producer-original diagnostic

The [hash-pinned original source](original-tiger.json) is Artifex's
[Illustrator-created tiger.eps](https://raw.githubusercontent.com/ArtifexSoftware/ghostpdl/9d76ba026c28e08dfdf9a6866dcf87514dab935b/examples/tiger.eps).
The original artwork remains outside the checkout. The current rebuilt public
decoder refuses at instruction one, the first `save`; no pixels/preview are
substituted. This identifies a real procedural prerequisite rather than
promoting an authored test as producer compatibility.

Adobe's [PostScript Language Reference](https://pdfa.org/norm-refs/PLRM.pdf),
sections 3.7 and 8.2, distinguishes VM save/restore from gsave/grestore.
Restore must validate stack references to objects that would be discarded,
roll back supported VM state and handle the graphics save boundary.
Simply aliasing save/restore to gsave/grestore would be incorrect.

## Implemented graphics primitive

A checkpoint copies only the active graphics-state stack's metadata. Current
state preserves matrix, style, path/subpath/current point and clip reference.
Immutable persistent node/paint arenas remain with the same live graphics owner.
Monotone checked process-local context identities reject a foreign/recreated
owner even when its coordinates and arena sizes match. These are not stable
serialized identities. A checkpoint does not own geometry; it may outlive its
owner as metadata, but cannot be restored into another owner.

Stack payload capacity is reserved before allocation through the caller's budget.
The actual `SharedBuffer` capacity remains charged through the last checkpoint
clone; clones share its backing. This does not account all Arc/allocator/RSS
overhead. Snapshot cancellation or admission failure leaves the graphics owner
unchanged. Restore validates context, required node/paint prefixes, stack capacity
and every cancellation checkpoint before publishing a bounded metadata copy.
It allocates no new geometry or paint records and deliberately retains later
paint output. Successful restore can be repeated while the same owner/arenas
remain valid; VM save-token invalidation is not implemented by this primitive.

## Executed evidence

- [Focused checks](checkpoint-tests.log): two new controls plus two existing
  cancellation controls pass. Exact metadata quota succeeds, one byte less
  refuses, clones share payload/credit, and all exposed capture/restore
  cancellation points preserve published state and saved-stack contents.
- The 8x8 analytic control compares every RGBA pixel: a post-checkpoint red
  rectangle remains visible; restoring the old green path and narrower clip
  paints exactly its clipped region. Rewinding paint storage, losing clip state
  or overwriting persistent geometry would fail the independent occupancy map.
- Foreign same-content contexts reject. Dropping the original owner does not
  make its snapshot valid in a new owner. Both geometry and separate snapshot
  budgets return to zero after their actual last owners drop.
- [Vector regression](vector-regression.log): 261 passed, 11 manual ignored.
  [Application regression](app-regression.log): 364 passed, 109 ignored.
- [Ordinary vector check](production-check.log) and
  [public decoder example build](example-build.log) pass. The diagnostic is
  repeated with the rebuilt binary and still explicitly refuses `save`.
  Format and whitespace checks pass. No wall-clock or GPU/HDR gain is claimed.

Reproduce with `cargo test -p rrrah-vector --lib checkpoint_ --locked`; full
source/log hashes are in [sources](sources.json) and [result](result.json).
Retrieve the pinned original to an external corpus, verify its recorded SHA256,
and run `raster_fixture_dump SOURCE 0 OUTPUT` to reproduce the current refusal.

## Required continuation

Integrate actual VM save objects and active save identities, admitted dictionary/
array/binding rollback, checks for invalid operand/execution/dictionary-stack
references, and protected graphics save anchors. Original Illustrator procsets
also require dictionary-stack and further graphics operators after the first
barrier is resolved. Until then the original stays unqualified and AI Pending.
The full 100-format/cache/GPU objective remains open.
