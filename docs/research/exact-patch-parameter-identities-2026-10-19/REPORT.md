# Exact control-net parameter identities

A bounded native certificate now establishes when source u or v is affine in
position over an entire Tensor patch. The connected candidate shares each proved
field across all triangles. Production PDF sampling remains unchanged, and the
analytic colored-fold control still fails by one RGB8level; this is not complete
folded-color or AI qualification.

## Proof and representation

Each finite binary64 control coordinate is decomposed into an integer
significand and power of two. Controls are scaled exactly to a common dyadic
integer grid when checked i128 arithmetic permits it. A noncollinear control
basis defines a rational candidate for3u or3v. Every one of the sixteen
Bernstein controls must satisfy that rational identity with exact integer
multiplication/addition; no epsilon is used. The Bernstein first-moment identity
then establishes the affine parameter relation over the complete source net.
Overflow or unavailable representation produces no certificate and never
silently classifies a nearly affine net as affine.

The proved rational field is rounded once to binary64, composed with one checked
finite device inverse, and reused at every triangle in a pixel. This makes the
proved parameter ties coherent. The exact proof concerns the source control net;
floating device composition is not an interval certificate. Singular/nonfinite
transforms, nonfinite transformed controls and unrepresentable fields refuse
certification. Computation uses fixed stack storage and checked integers, with
no heap allocation or new runtime dependency.

`PatchParameterIdentity::apply` replaces only proved parameters, validates the
patch index, and calculates both replacements before mutating the owner. An
unproved u/v retains its original triangle field. This does not establish branch
ordering for a general nonaffine parameter and is not a general inverse solver.
Only Tensor source-net certificates are currently exposed by this API.

## Evidence and remaining color failure

The simple fold x(u)=6u(1−u), y(v)=3v proves global v=y/3 and does not prove affine
u. In the candidate, max v-field inconsistency falls from5.12e-13tozero. The
colored control now yields RGB230in the first column and176in the second on all
rows, versus analytic230and177. The previous candidate differed by up to10.
The strict oracle is unchanged and the remaining one-level discrepancy remains
a known ignored failure.

On that same nine-pixel diagnostic, observed retained fragments change from6495
to1996, managed peak5,407,984to1,717,680bytes, and work consumption3,165,802to
1,463,428units. These are bounded candidate diagnostics with changed, improved
pixels, not a paired speed benchmark or proof of practical full-page cost.
Final managed ownership returns to zero.

The remaining error requires a projected source-component accuracy bound:
parameter-space bilinear color error alone does not bound interpolation error
after a curved projection. No geometry tolerance, component tolerance, memory
budget or expected output was loosened to accept176instead of177.

Validation includes exact certificate rejection after a one-ULP source change,
a sheared source net plus reflected/sheared device transform, unavailable u
preservation, fixed-width/finite-device overflow, singular/nonfinite inputs and
transactional owner mutation failure. A generated family exercises63invertible
integer affine transforms of a folded source and one-ULP certificate rejection
for each. Final test results are appended below after completion.

## Next implementation gate

Bound source-component residuals in device-space interpolation against the
original control net, refine or refuse when that bound is not met, and retain
the analytic color oracle. Afterward production still needs bounded page-level
spatial candidate processing, nonlinear source integration, original AI and
RAM/swap/Metal requalification. The full-PDF overlap and folded regressions
remain unresolved because the new candidate is not yet connected to the viewer.

## Independent projected-color control

`projected-component-control.py` integrates the upper branch's chord-linear
inverse using exact rational arithmetic. The analytic average of the partially
covered second pixel is0.5+(1/3)^(3/2), or176.5747728811RGB8levels before rounding.
Uniform32-segment interpolation produces176;64segments produce177. This1Dcontrol
does not duplicate the stream's conforming boundary fans or certify its exact
runtime error, but establishes why source-parameter linearity is insufficient.

For one upper-branch cell[a,b], its component residual is
(u−a)(b−u)/(1−a−b). Its exact maximum magnitude is
(b−a)^2/[4(a+b−1)]. Thus the cell next to the fold has residual h/4, even though
the source corner color has zero bilinear cross term. This points to adaptive
projected-component certification under the existing component-error gate,
rather than globally changing an image acceptance tolerance. Raw rational and
80-digit Decimal results are retained in `projected-component-control.json`.


## Final validation

Current full vector suite:201passed, zero ordinary failures,14ignored. All63
invertible generated source transforms and their one-ULP rejection controls pass.
Explicit current execution of `patch_bridge_colored_fold -- --ignored --nocapture`
still fails against the unchanged analytic177second-column oracle, returning176.
Both original full-PDF rendering regressions also remain known ignored failures;
this certificate/candidate does not change the production sampler. Formatter
checks on the changed sources and patch whitespace validation pass. Logs are
`vector.log` and `colored-known-failure.log`.
