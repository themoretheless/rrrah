# Narrow PQ PNG: RAM/swap/Metal transport

The deterministic standard-library producer `scripts/generate-png-narrow-pq.py` writes two 60x48 PNG3 RGBA files (8/16 bit), cICP 9/16/0/0, six neutral bands including below-black and above-white codes, and opaque alpha. The filenames use `color` to exercise the existing signed-output check; these are neutral ramps, not independent chromatic-primary or translucent-alpha cases. Re-running the producer preserves fixture/reference SHA-256 hashes.

For each source the public explicit-PQ decoder is compared at every RGB/alpha pixel against 70-digit Decimal ST2084/range references (2,880 pixels per file, white=100 cd/m²). The existing HDR still lifecycle helper then checks actual Metal on Apple M4 Max:

- complete native float-bit retention through RAM admission, live lease protection, eviction to a count-one/byte-bounded swap;
- full shared-root occupancy refuses restore, releases correctly, then permits retry;
- callback cancellation after restore allocation yields no frame and zero retained credit; subsequent retry succeeds;
- full SDR byte readback and float16 HDR attachment readback match before/after swap, including negative values and extended peak luminance;
- clone lifetime keeps exactly the raster weight; final root=0, one successful write/read and zero swap errors.

Focused required-Metal gate: 1 passed / 0 ignored / 0 failed, covering both sources in 2.07s excluding compilation (metal.log). HDR GPU comparisons establish transport invariance plus signed/peak controls; native Decimal references establish source conversion. This does not establish a full independent GPU color/compositing oracle, all PNG variants, narrow HLG, physical HDR presentation or NVIDIA.

Broader current required-Metal HDR still suite: 24 passed / 4 ignored / 0 failed in 4.18s excluding compilation (hdr-suite.log). The new explicit/manual test is one of those ignored in the default sweep and passed separately above. External photographic/manual/large-buffer gates were not rerun by this default sweep.
