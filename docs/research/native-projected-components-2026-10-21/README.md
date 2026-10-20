# Projected component bounds for native compact patch capture

Native Tensor/Coons compact geometry and its separately checked source-UV metadata now use the same projected-component stream. Previously native capture used only parameter-space bilinear color bounds, which omit the color residual introduced by nonlinear projection. The new compact API owns its admitted output and releases partially checked geometry on any refusal or cancellation.

The 1/64 pixel and 1/1024 source-component tolerances remain unchanged. Native capture uses the tessellator's supported depth 16, retaining its triangle limit and existing caller memory caps. The rejected depth-12 pilot refused an asymmetric sheared fold. At depth 16 that control retains 5252 triangles, with reported maximum projected component bound 0.0009581229115921317 and pixel bound 0.0029296875004104495. These numerical control-hull bounds have floating-point margins; they are not interval certificates and do not certify nonlinear color transforms or source inversion.

A new root control checks curved Tensor and Coons compact output against the checked stream vertex-for-vertex and component-bit-for-component-bit. It exercises each output admission failure, cancellation at start/middle/end, zero retained root accounting and retry under 8 MiB. Existing native point-source inversion, boundary integral and spatial ownership controls retain their original expected values.

Native full suite: 319 passed, 13 ignored. Rejected pilots and successful raw logs are retained separately. Increased subdivision buys a stronger source-component bound, not a measured speedup; broad page latency, AI fidelity, format completeness, GPU and physical HDR are not established here.

Final vector suite: 214 passed, 11 ignored. Interpreter suite: 141 passed, 1 ignored, including complete exercised Tensor/Coons admission-failure sweeps and metadata cancellation/retry.
