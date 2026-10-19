# Raster/model retained prefetch under fully protected byte limits

Retained model prefetch now checks the shared minimum-weight policy after its resident hit and count checks, before swap/native decode. One byte is a conservative lower bound: current model capacity accounting includes a nonzero model header even for empty geometry. Raster retained worker policy uses four bytes (a nonempty RGBA8 pixel), before swap restore and again before cold decode. Count refusals retain their prior diagnostic; byte refusals are explicit. RasterRamCache forwards the existing non-evicting minimum-weight query.

These lower bounds detect exhausted protected byte budgets, not the exact decoded size of arbitrary files. Partially available capacity can still be insufficient for a specific raster or model and require later parsing/admission. No dimension guess, unqualified source fallback or quota relaxation was added. Low-level ephemeral swap requests without worker retention remain available.

Existing STL/OBJ/PLY/OFF cold and swap-prefetch regressions now run independent count-only and byte-only policies. They prove decoder bypass for blocked cold loads, zero blocked swap reads, warm resident hits, exact native payload restoration once after nearest lease release, preserved selected pin and final managed release. The associated-emission EXR swap test likewise runs both policies with exact payload comparison.

An additional valid-empty-STL regression proves header-only models remain admissible with exact header space: it loads an empty model alongside the selected triangle, refuses a third header while both entries are protected, then admits it after the first empty lease is released. Empty geometry is not silently rejected.

The broad pilot found a DICOM RLE test expecting throwaway unretained decode under its byte-only limit. The worker policy intentionally changes that behavior; the test now requires nearest retention and far refusal for both count and bytes, with policy-specific diagnostics. Existing explicit ephemeral raster swap behavior was not changed. The pilot failure is retained.

Final ordinary application suite:293 passed,83 ignored. The DICOM matrix retains its original source/pixel/selection checks and adds the byte refusal expectation; existing model/raster foreground swap, cancellation, TTL and low-level ephemeral-hit regressions pass. Formatting and whitespace checks pass. No throughput, physical disk, GUI, NVIDIA or HDR claim is made.
