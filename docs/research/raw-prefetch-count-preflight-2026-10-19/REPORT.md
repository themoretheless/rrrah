# RAW retained-prefetch protected count admission

The foreground loader's RAW neighbour path previously probed temporary swap, persistent disk cache and native decode even when all RAM entry slots were protected by the selected frame and nearest consumer lease. MosaicRamCache now exposes the same non-evicting count-only admission check as raster cache. RAW retained prefetch checks it after recipe/fingerprint resolution and cancellation, before swap/disk restoration, gate acquisition or sensor decoding. Refusal preserves the existing (false,None) result.

This is a count-only optimization. Source recipe/fingerprint work remains, and byte-only policies may still require parsing/restoring to discover output capacity. Existing resident keys remain eligible even at full protected count. The independent gallery persistent-cache warmer is unchanged. This does not promise that no metadata/source bytes are read.

The new library regression protects a selected mosaic and a nearest lease, confirms existing keys stay eligible while a new key is refused, reconciles release and permits admission without evicting membership during the check.

The existing original CR3 scope test was strengthened with a before/after managed peak check for its count-limited cases. It passes both directions, independent count/byte policies, malformed neighbour, nearest lease protection, ready shared hit under full managed-credit reservation, far admission after release and final managed root zero. Temporary path copies establish distinct timestamp keys; this remains one sensor source, not a collection of independent RAW photographs. No source media is stored in this report or newly checked in. No new independent sensor/color oracle comparison is claimed.

150 cache library tests pass (three ignored),291 ordinary application tests pass (82 ignored), and the actual original-CR3 manual test passes. Formatting and whitespace checks pass. No timing/RSS, physical slow-disk, GUI or GPU performance claim is made.
