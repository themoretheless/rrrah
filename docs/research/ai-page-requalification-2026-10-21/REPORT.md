# Current AI page regression and projected visible-domain bounds

The rebuilt current raster_fixture_dump was compared against Poppler at 72 dpi using the pinned public VectorApple.ai and one.ai byte lengths/SHA256. Source artwork remains outside the checkout. Strict comparison tolerances were not changed.

## Findings

VectorApple.ai currently refuses with shading decode/precision limit, whereas the earlier pre-ordered-sampling renderer decoded it with known appearance differences. This is an unresolved compatibility regression, not successful format support. one.ai page 0 retains 2522 differing pixels, maximum channel error 58 and MAE 0.033139260004880426; page 1 is exact.

Depth-16 diagnostics isolate ill-conditioned projected component bounds on nearly degenerate cells, plus actual conforming fan triangles that exceed the preliminary planning bound. Merely increasing depth did not repair this file.

## Retained change

A separate visible_projected_component_bounds API bounds the residual on the intersection of the mapped source UV rectangle and the painted triangle. Bilinear source extrema occur at rectangle corners; painted triangle interpolation is a convex combination of its retained vertex components. The bound includes floating-point arithmetic magnitudes and takes the tighter of this range estimate and the original full-rectangle estimate. The original projected_component_bounds extrapolation contract is preserved. Invalid/nonfinite inputs, failed owner fitting, short work and cancellation still refuse.

The new numerical control independently tests mapped points inside projected triangles against this bound and checks work/cancellation refusal. Vector 215 passed/11 ignored; native 319/13 before the rejected global-replanning pilot. Ordinary sampling now uses original Coons boundary identity proof and the supported depth 16, keeping the 1/64 geometry and 1/1024 component tolerances. These improvements do not resolve the full Apple page regression.

## Rejected pilot and next action

An eight-attempt global planning preflight/tightening pilot was tried with actual output verification. Its strict report is fan-replan-poppler.json. It still refused the page with TriangleLimit, Depth and pixel-paint Limit cases. The pilot was removed: it increases processing cost without closing the full-page failure. Local refinement of offending emitted fans and bounded ownership processing remain necessary. No caller limits or expected pixels were relaxed.

## Qualification runner

The runner now enforces a finite positive per-render timeout (default 300 seconds), continues subsequent pages on timeout and retains up to 16 KiB of renderer stderr on failure. A sleeping-renderer control produced three explicit page failures in 0.27 seconds and strict exit 1; zero, negative, NaN and infinity arguments were rejected. This is a diagnostic timeout, not a performance target.

These results do not establish full AI, all 100 formats, GPU fidelity, NVIDIA or physical HDR qualification.

Final retained-code interpreter suite: 141 passed, 1 ignored, including exhaustive exercised Tensor/Coons admission failures and retry. Workspace formatting and whitespace checks passed. The obsolete interpreter run for the rejected global-replanning pilot was explicitly terminated after the pilot was removed; it is not counted as passing evidence.
