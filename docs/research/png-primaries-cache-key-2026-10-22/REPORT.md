# Explicit PNG primaries belong to viewer RAM/swap identity

`DecodeRequest::png_assumed_rgb_primaries` affects gamma-only RGB PNG interpretation, but the internal viewer raster key omitted it. The new asymmetric authored RGBA16 PNG has only gAMA=0.5; sRGB and Display-P3/D65 primary assumptions produce different uncached linear pixels. Before the fix, warming the cache with sRGB and requesting P3 returns the first interpretation's bits (red.log, selected=1). This is a reproduced cache-isolation failure, not a gamut-math disagreement.

The key now includes the optional eight f64 coordinate bit patterns. None and each explicit policy remain distinct without rounding coordinates or relying on floating-point equality. The payload codec and color conversion were unchanged. Four manually constructed default keys in existing pressure tests were extended with the new None field.

The regression uses uncached display loading as its transport reference; it does not claim a new independent color-math oracle. It requires both initial cached loads to match their own uncached pixels, a NaN-coordinate request to fail even with a warm cache, then four alternating swap restores with decoder execution forbidden and an exact read-count increment for every lookup. RAM is one frame/one entry, disk has two entries, queue one frame and restore quota two frames so the current restored frame stays protected while its replacement is admitted. Per-step ownership returns to one RAM frame; final root=0 and swap errors=0.

Validation: focused regression 1 passed / 0 ignored / 0 failed; full app binary 298 passed / 84 ignored / 0 failed. Both use:

    cargo +stable --config 'profile.test.package.rrrah.opt-level=0' test -p rrrah --bin rrrah ... --locked --offline

Only the app package's test optimization was overridden; dependency settings and project configuration were unchanged. These are functional gates, not performance measurements. Hardware/manual ignored gates and live UI were not exercised.

Fixture generation is in fixture.py. The original existing gamma/cHRM fixture was found to contain neutral samples only, unsuitable for reliable primary discrimination. Its obsolete build was deliberately interrupted with SIGINT before executing, then replaced with the asymmetric fixture; obsolete-neutral-build-interrupted.log records that stage. schema-update-required.log retains the subsequent compile diagnostics that identified four old manual test-key tuples. red.log is the actual pre-fix colored-pixel failure; green.log/app.log are terminal corrected results. Source/fixture hashes are recorded alongside this report.
