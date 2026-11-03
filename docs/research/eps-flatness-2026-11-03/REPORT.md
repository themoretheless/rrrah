# Native EPS flatness and imported-job graphics state

Native currentflat/setflat now operate on captured graphics state. Default 1.0;
finite values clamp to 0.2–100. Invalid native API nonfinite inputs refuse without
changing the state. Saves and VM rollback preserve flatness. initgraphics keeps
it while resetting its own parameters, matching the independent interpreter
and Adobe's operator contract.

## Device precision and clip cache

Fill, stroke, hairline and clip flattening use the tighter of the caller's
final-pixel error ceiling and captured flatness. Stroke preparation maps that
ceiling into user space with the existing affine norm. Each clip keeps the value
from clip creation, even if the active flatness later changes. This is an actual
rendering constraint: controls show different curve geometry and clipped pixels
when the caller permits coarse rendering. Tight caller precision remains intact.
The default caller ceiling (0.125 pixel) is already stricter than the minimum
PostScript flatness; source code cannot loosen that chosen precision. Invalid
caller tolerances are not normalized into successful rendering.

Clip-mask equivalence now includes effective precision, in addition to geometry
and fill rule. A coarse mask cannot replace a finer one. Conversely, masks with
identical geometry remain reusable when different nominal flatness values both
reduce to the same stricter caller ceiling. The regression primes the cache
with a coarse white clipped paint, then compares the fine green result with a
fresh scene; it separately verifies visibly different permitted coarse output
and equal tight results with mask reuse. There is no new unbounded cache.

## Import context

Document imports own an implicit initial graphics boundary, as in the reference
interpreter's job context. Empty-stack grestore and grestoreall restore that
initial supported state. Explicit VM boundaries still take priority. This does
not consume an explicit saved-state slot or create a user-visible VM save token;
the initial supported state is constant. Cancellation finishes before reset.
The lexical standalone graphics/evaluation API retains its previous unencapsulated
model. The distinction fixes the context without changing standalone callers.
The importer remains a bounded supported PostScript subset, not a full device,
job server, system dictionary or font implementation.

## Executed evidence

- 22 independent top-level Ghostscript 10.05.1 state/error traces match. Decimal
  real outputs are compared through the VM's round-trip f32 numeric representation.
  Controls cover clamping, aliases/bind, snapshots, initgraphics preservation,
  and explicit/implicit graphics boundaries.
- Eight state-controlled scenes match all 64 RGBA pixels through the public
  document path. Their predicates paint red on a wrong state and green on a
  correct state. The single shared RGB reference is analytically checked and
  stored once, instead of duplicating identical image evidence eight times.
- CPU controls verify real stroke/clip precision changes, caller-ceiling
  preservation, correct clip-mask invalidation and equivalent-precision reuse.
  Empty-slot import-boundary cancellation is atomic; invalid setters preserve
  state; managed roots return to zero after owners drop. Existing global
  cancellation/work/admission regressions also pass with the new state layout.
- Vector regression: 289 passed, 11 existing ignored. Ordinary viewer regression:
  364 passed, 109 ignored. Production check, public decoder example build,
  formatting and whitespace checks pass.
- All eight state-controlled scenes pass native RAM, pressure/cancellation,
  disk restore and exact SDR/HDR offscreen Metal on Apple M4 Max, with final
  native root credit zero. [Hardware log](metal.log). These verify the pipeline;
  the curve/clip precision effects are independently checked on CPU. They do
  not qualify physical HDR, all HDR values, NVIDIA or every PS rendering mode.
- An authored EPSF .ai path passes public decode_raster: 8x8, image zero of one,
  all native pixels composited on white agree with the independent reference.
  [Route evidence](ai-file-route.json). This is not broad Illustrator support.

Source/reference pins and terminal summaries are in [result](result.json).
Repeated full regression logs are not duplicated. Reproduce the reference with
python3 scripts/generate-eps-flat-reference.py and Docker available. There is
no measured speed/RSS improvement claim; backing capacity remains admitted and
new state metadata increases its stride. Contract: Adobe
[PLRM3](https://pdfa.org/norm-refs/PLRM.pdf), setflat/currentflat, initgraphics,
grestore/grestoreall and graphics-state/device parameters.

## Producer original

The same external hash-pinned Illustrator Tiger now passes currentflat and
finishes its earlier flatness-related prolog. The rebuilt decoder refuses at
instruction 285, clippath: [diagnostic](original-tiger.json). No substitute image
or original artwork is vendored. Native clippath and subsequent Illustrator
operations, broader AI compatibility and the full 100-format/cache/GPU objective
remain open. AI stays Pending and format counts do not change.
