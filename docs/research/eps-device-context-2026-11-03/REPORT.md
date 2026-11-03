# EPS import device context

The native EPS importer now passes its actual output viewport and rounded pixel
extent into the graphics owner. `EpsDeviceContext` validates finite, invertible
mapping and nonzero dimensions before publication. It retains the initial
physical device rectangle as four canonical EPS points, including sheared or
rotated mappings, without replacing that polygon with its bounding box.

`device_matrix` composes viewport and source CTM in the same order used by the
rasterizer. The device context remains outside source graphics/save state;
`initmatrix`, `initgraphics`, `grestore` and `grestoreall` cannot change device
ownership. It requires no heap allocation or additional arena slots.

The new control exercises fractional page dimensions rounded to pixels, negative
origins, flipped device Y, nonuniform rotated source transforms, shear, inverse
roundtrips, reset/save lifetime, zero dimensions, singular/nonfinite inputs and
inverse overflow. The managed root returns to zero.

This is infrastructure for native `clippath` and device-coordinate operators.
It does not implement those operators or qualify general AI imports. In
particular, the stored initial polygon does not replace the current clip chain;
that chain still requires geometric intersection respecting nonzero/even-odd
membership, open subpaths and the captured curve precision.

The operator semantics being targeted are the Adobe
[PostScript Language Reference, third edition](https://www.adobe.com/jp/print/postscript/pdfs/PLRM.pdf),
`clip` and `clippath` (printed pages 542–543). No external interpreter is used by
the production path. Previous flatness fixtures are reused without copying them.

Final validation summaries and current source/log hashes are in `result.json`.
Physical HDR display and NVIDIA qualification remain outstanding.
