# AVIF crop and transform pixel-overlap admission

A new public-file regression covers **31 pinned AVIF cases**:24 clean-aperture cases from the independent libavif1.4.2 metadata TSV and7 standalone producer orientations2–8. Standalone coded geometry8x4 is defined by `scripts/generate-avif-orientation-fixtures.py`; those cases explicitly have no clean aperture and do have rotation/mirroring. Thus crop cannot hide a missing standalone-transform reservation.

Every case admits exactly encoded-file bytes + full native RGBA8 coded pixels + final cropped/oriented RGBA8 pixels. Observed managed peak equals that sum; returned ownership is only final pixels. One-byte-short typed Memory refusal rolls back to0, retry succeeds, last-clone ownership releases to0, and source files remain byte-identical. ICC cases are explicitly excluded from this pixel-only matrix rather than silently treating profile storage as zero.

Every output byte is compared to the existing independent producer/Pillow oracle under that fixture's manifest tolerance (up to3code values); dimensions are exact. This is not a claim of bit-identical agreement with a different AV1 implementation. Full public decoder gate: **200 passed /5 ignored /zero failures**. No production source changed in this follow-up; it validates the previous pixel-overlap fix.

Private dav1d/codec allocations can occur in decoder construction before pixel reservations and are not counted by this matrix. Complete decoder heap limits, HDR/CUDA/NVIDIA and full-format interoperability remain unqualified. Prior broad immutable-input workspace results retain their historical hash scope; no repeat broad gate is claimed here.
