# Admit native image output overlapping final RGBA conversion

The generic image route admitted final RGBA output but not the separate native RGB/gray allocation retained while `DynamicImage::into_rgba*` constructs its result. The regression recorded a 4x3 RGB8 PNG budget peak of48 bytes, although36 native RGB bytes overlap48 final RGBA bytes.

Before `DynamicImage::from_decoder`, managed requests now reserve `decoder.total_bytes()` when the decoder native color type is not RGBA8/RGBA16/RGBA32F. The reservation lives through conversion and is released before returning the managed final raster. Native RGBA direct-transfer paths keep their previous single output credit. The separately managed profiled CMYK route is excluded from this generic reservation.

Authored RGB PNG8/16 controls retain every sample and opaque alpha; exact final+native capacity passes (84/168 bytes), one byte less refuses with typed Memory and releases all credits. Final output owners retain48/96 bytes until the last clone is dropped. Tests borrow caller-owned encoded source; these measurements exclude source allocation and codec-private buffers.

Public decoder library: **198 passed /5 ignored /0 failures**. Viewer: **285 passed /80 ignored /0 failures**. Initial failing regression retained. This is a pixel-conversion admission fix, not full PNG/TIFF decoder heap accounting: constructor metadata, private codec scratch and allocating orientation/crop/AVIF transform overlaps still require separate audit. No latency speedup or full-format qualification claim.
