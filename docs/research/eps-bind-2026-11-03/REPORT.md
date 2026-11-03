# Native PostScript bind for EPS-framed artwork

The native Rust EPS evaluator now implements operator substitution for `bind`.
Operator values resolve in the current supported dictionary environment;
variables, undefined names and user procedures remain late-bound. Operator
aliases bind correctly. Repeated binding preserves existing operator objects.
Nested executable procedures become read-only for later binding, including
when reached through aliases or a literal array. Literal-array cycles are not
followed as executable procedures. Source instruction buffers remain immutable;
VM-local tables preserve procedure identity and alias effects during execution.

The model follows Adobe's [PostScript Language Reference, third edition,
section 3.12.1 and bind operator](https://pdfa.org/norm-refs/PLRM.pdf). The
[Adobe Illustrator format specification](https://13thmonkey.org/documentation/fonts/AI7FileFormat.pdf)
describes the procedural prolog/script structure. No specification or original
Adobe procset code was copied into the authored fixtures. Full dictionary stacks,
access-attribute operations and Illustrator procsets remain outside the current
VM contract. Idiom recognition is not implemented.

The table allocates lazily on the first nonempty procedure bind and admits
`instruction_count * size_of::<Binding>()` before allocation. Existing scalar
programs retain zero binding-table capacity. Traversal is iterative, preserves
read-only nested boundaries and debits the existing work/cancellation contract.
No operation/geometry limits or rendering tolerances were increased. Source
programs are reusable after failure because partial binding state belongs only
to the failed VM. Evaluation procedure values remain diagnostic references,
not standalone executable VM snapshots.

## Verified evidence

- [16 independently executed Ghostscript cases](../../../tests/fixtures/eps/bind-ghostscript-reference.json):
  operator redefinition, undefined/variable/procedure late lookup, aliases,
  nested read-only rebinding, literal arrays and a cyclic literal array.
  The CC0 [generator](../../../scripts/generate-eps-bind-reference.py) uses the
  pinned Alpine image and records Ghostscript 10.05.1/binary hashes. Both
  scalar and pixel invocations verify the same executable version/hash.
  Ghostscript is a test-only oracle; production adds no runtime dependency.
- [259 vector tests](vector.log) pass, 11 manual controls ignored. The new
  memory gate admits exactly the fixed VM plus binding-table capacity and
  refuses one byte less. All observed cancellation points and short work
  quotas discard temporary credits, permit retry and end at zero ownership.
- A CC0 [authored prolog](../../../tests/fixtures/eps/bind-prolog.ai) binds its
  drawing procedures, then redefines the original drawing/color operator names.
  All 64 source pixels match independent Ghostscript RGB and analytic alpha.
  This unit uses a 32 MiB root because the existing default graphics arena
  reserves more than 4 MiB; no smaller production admission claim is made.
- The [public raster fixture dump](public-inspection.json) opens this `.ai`
  source through its EPSF content framing as one 8x8 raster. EPSF `.ai`
  discovery was already routed by content; no new format detector was added.
- [Actual M4 Max/Metal lifecycle](metal.log): common decode, retained RAM,
  native swap, full-root refusal/retry, cancellation and complete SDR/half-float
  frame equality against the independent opaque-white-composite reference.
  Last-owner managed credit returns to zero. Physical HDR is not qualified.
- [Application regression](app.log): 364 passed, 109 manual/external ignored.
  [Production example build](production-build.log), formatting and diff checks
  pass. No wall-clock speedup is claimed.

Reproduce:

```sh
python3 scripts/generate-eps-bind-reference.py
cargo test -p rrrah-vector --lib bind_ --locked
RRRAH_GPU_BACKEND=metal cargo test -p rrrah --test eps_lifecycle \
  independent_bound_postscript --locked -- --ignored --nocapture
```

## AI remains Pending

The initial [frozen-binary AI audit](ai-baseline.json), before this EPS-only
change, confirms the existing gates: VectorApple refuses at the shading limit;
one.ai page zero differs on 2522 Poppler pixels and page one is exact. Prior
independent cubic-area evidence shows that some Poppler boundary differences
are not grounds to replace correct native area values. These PDF-compatible
AI failures were not addressed by adding a PostScript operator.

This change enables a necessary procedural building block for EPS-framed
PostScript artwork. It does not establish an original Illustrator procset or
whole-file producer qualification, resolve Tensor shading or increase the
100-format catalogue counts. The complete original objective remains open.
[Source pins](sources.json) and [result](result.json) retain exact provenance.
