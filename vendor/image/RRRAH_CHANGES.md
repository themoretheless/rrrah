Upstream: image 0.25.10 from Cargo registry, licenses retained.

Local change: AvifDecoder::new_with_pixel_limit exposes dav1d Settings::frame_size_limit for both primary and alpha constructors. Existing new preserves zero/unlimited default. Pixel area limit is not a complete decoder byte/scratch bound.

Reject decoded alpha/primary bit-depth mismatch before ICC copying or pixel conversion. AVIF section4.1 requires identical bit depths. Reproduced malformed controls previously decoded in both8/10-bit directions.
