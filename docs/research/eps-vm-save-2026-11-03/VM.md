Current follow-up: [name-keyed dictionaries and stack](../eps-dictionaries-2026-11-03/REPORT.md). The single-dictionary scope below records the preceding VM revision.

# Native VM save/restore for the supported EPS object model

This supersedes the earlier checkpoint-only prerequisite status in REPORT.md
and BOUNDARIES.md. Native `save` and `restore` are now connected to the scalar
and graphics VM. This qualifies the supported single-user-dictionary,
literal-array and diagnostic source-procedure model, not full PostScript or
producer-original Illustrator support. AI remains Pending; format counts do
not change.

## Semantics and admission

Each save has a monotone evaluation-local identity and a linked, admitted
snapshot. Dictionary entries, full array backing, bind/readonly metadata and
known source-procedure ranges are retained. The graphics checkpoint is captured
before an implicit protected graphics boundary is pushed. Snapshot storage and
the linked node's fixed payload are reserved in the caller's root budget before
publication. Nesting is bounded by the caller's execution-frame limit and a
hard 128-save cap. Existing work and saved-graphics limits still apply. This is
managed payload accounting, not a complete allocator/RSS measurement.

Restore finds an active token, validates surviving operand and execution
references and completes cancellation checks before publishing graphics or VM
rollback. It moves the admitted snapshot buffers back into the VM, discards the
target and younger snapshots, consumes its operand token and preserves other
operands and active execution. It does not roll back painted output. Save IDs
are never reused, so a token left after its target restore cannot restore again.

Empty composite objects and a duplicate target token have reference-compatible
behavior. A younger save token or a nonempty discarded array/procedure in a
surviving stack refuses with `InvalidRestore`. Completed one-shot execution
frames do not keep an unnecessary procedure reference; loop continuations still
do. Existing source-procedure values remain instruction-range objects; dynamic
strings, arbitrary executable-array mutation, dictionary objects/stacks and
stream-aware full PostScript are not implemented by this change. The single
static user dictionary itself survives, while its entries roll back.

## Independent reference and controls

[Thirty CC0 source cases](../../../tests/fixtures/eps/save-ghostscript-reference.json)
were executed by Ghostscript 10.05.1, test only. Each case uses a separate fresh
process and top-level source file. The harness intentionally avoids wrapping the
source in a procedure or `stopped`: doing that would allocate procedure literals
before save and invalidate the intended object-lifetime test. The fixture pins
the image, executable, generator and source hashes. Reproduce with
`python3 scripts/generate-eps-save-reference.py` with Docker available.

Controls cover dictionary/array/binding rollback, nested restores, operand
preservation, array views and zero-length objects, discarded live references,
stale/younger/duplicate save identities, tail execution and loop references,
protected graphics boundaries and retained paints. Direct live-machine tests
cancel every exposed capture/restore checkpoint and assert the changed dictionary,
array and graphics remain published until a successful atomic restore. A full
execution sweep checks every cancellation point and every insufficient work
allowance, then searches the exact memory quota and tests one byte below it.
All managed credit returns after the actual owners drop.

An [authored 8x8 EPSF AI fixture](../../../tests/fixtures/eps/save-prolog.ai)
mutates a preexisting RGB array, paints green and blue across nested snapshots,
then restores the red array and paints another rectangle. Ghostscript RGB also
matches an independent analytic map for all 64 pixels. The ordinary native
raster decode, RAM cache, disk swap restoration, full-budget refusal, cancellation
and exact SDR/HDR offscreen Metal rendering pass on Apple M4 Max. These are
ordinary pipeline components, not physical HDR-display or NVIDIA qualification.

Executed regression: 270 vector tests passed (11 ignored); 364 ordinary viewer
tests passed (109 ignored); the new actual-Metal lifecycle passed. Formatting,
whitespace checks, production vector check and public decoder example build
passed. [VM result](vm-result.json) records source hashes and terminal summaries;
only the short new hardware log is retained, avoiding duplicate full regression
logs. The previously committed logs describe their earlier source revisions.

## Producer-original result and remaining barrier

The same hash-pinned Illustrator 1.2d4 tiger.eps now passes instruction-one
`save`. The rebuilt public decoder explicitly refuses at instruction six,
`dict`, rather than producing a preview. [Diagnostic](vm-original-tiger.json).
The original artwork remains outside the repository. Native dictionary objects,
`begin`/`end` and further Illustrator operators remain necessary. No original
artwork or broad AI qualification is claimed, and the full 100-format/cache/GPU
goal stays open.
