# Projected component adaptive stream candidate — 2026-10-19

The opt-in admitted Tensor/Coons stream bounds affine projected source components against the original bilinear source field on restricted Bernstein control nets. Refinement checks eight candidate triangle stencils, uses anisotropic subdivision when curvature is dominant on one axis, and verifies every actual emitted triangle, including conformity fans. Existing default rendering APIs remain unchanged.

The first pilot refused ComponentAccuracy; its log is retained. The revised exact colored folded 3x3 control matched all 36 expected RGBA bytes, including second-column 177: 6096 triangles, 3962 fragments, peak accounted payload 3,547,760 bytes under an unchanged 8 MiB root, geometry 1/64 and component 1/1024 limits. These are managed payload measurements, not RSS.

Bounds use numerical Bernstein control hulls and floating-point safety margins, not interval certificates. They apply to source components before nonlinear functions or color transforms. This candidate does not solve general inverse branch selection or qualify full-page AI/PDF fidelity. Two full-PDF overlap controls remain ignored known failures in the existing viewer. Physical HDR, NVIDIA/CUDA, and prolonged live GUI endurance remain unqualified.

Delivery checks: application 294 passed / 84 ignored; mesh 55 passed; vector 206 passed / 13 ignored. Zero failures. Curved Coons and anisotropic UV edge conformity controls passed. See delivery-tests.log. Workspace formatting and git diff whitespace checks passed.
