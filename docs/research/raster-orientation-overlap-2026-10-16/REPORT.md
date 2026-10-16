# Admit pixel copies during image orientation/transforms

A new RGBA PNG regression reproduced a missing copy credit: a2x3 RGBA8 EXIF5 frame tracked24 bytes although its allocating transpose overlaps two24-byte images.

The generic native-pixel overlap reservation now also applies when EXIF5–8 orientation, clean-aperture crop, AVIF rotation or AVIF mirroring creates a second pixel image. Final output credit plus original native decoder capacity bounds these sequential input/output overlaps. Ordinary RGBA transfer and EXIF1–4 in-place operations retain the previous single output reservation. Non-RGBA conversion retains the prior native+RGBA reservation. The separately managed CMYK route remains excluded.

All eight EXIF orientations are checked at8/16-bit RGBA precision (16cases). An independent explicit source-index permutation checks every channel and alpha, not another call to the orientation implementation. Dimensions swap only for orientations5–8. Exact peak24/48 bytes forRGBA8 and48/96 forRGBA16 (in-place/allocating) passes; one-byte-short typed Memory refusals release all credits. Last-clone ownership is checked. Source bytes are borrowed in these low-level controls; private codec allocations remain outside the measured pixel-credit scope.

Public decoder library: **199 passed /5 ignored /zero failures**. Initial failing test retained. Crop/AVIF paths share the production condition but this new fixture matrix proves EXIF PNG orientation only; independent exact admission controls for crop/AVIF and complete decoder-private memory accounting remain open. No physical HDR/NVIDIA or full-format qualification claim.

Viewer gate after this production change: **285 passed /80 ignored /zero failures** (`viewer.log`).
