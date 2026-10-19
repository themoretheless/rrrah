# JPEG XL frame-count scaling and retained encoded-history admission

## Reproduced valid-source failure

New CC0 libjxl C producer creates5x3RGBA16 full-replacement animations with3/30/300/3000displayed frames. Every C-decoded frame is compared with authored samples; only first/middle/last raw oracles are retained outside the checkout. Manifest records source/oracle hashes, encoded sizes and C producer hash. This isolates frame-count/history cost, not photographic complexity.

Before fix,3000frames/444615encoded bytes fail at every selected index even with512MiBparent budget: codec Buffer(OutOfMemory {bytes:131}). A smaller600frame88892byte regression reproduces the same failure. Per-image codec tracker allowed only65536+64*width*height bytes (66496here), excluding retained compressed frame groups. Global parent memory was available; the failure was the incomplete internal working model.

## Production change

For sources declaring animation, add the source span length to the derived codec working reservation. For a valid stream, the aggregate compressed frame groups retained during feeding cannot exceed its source span; the caller-owned source and this copy are different overlapping allocations. The parent must admit the larger working reservation before normal frame feeding/rendering proceeds, then the codec tracker uses that derived bound. Existing global codec maximum and user root limits remain unchanged; no arbitrary per-fixture allowance or unconditional larger limit is used. Still images keep their prior calculation. The work is a compatibility/accounting fix, not a speedup claim.

## Native/count/timing and parent quota gates

All12selected positions across4sources now match every independent native16bit sample, exact selected index/full count, fractional delay and2play metadata. All final root credits0. Every successful case is repeated with the observed exact peak, then one byte below it: exact peak succeeds, shorter root returns typed parent-memory refusal and releases credit, then retry succeeds. Probe explicitly checks public decode_raster rather than bypassing the viewer decoder.

3000frame root peak955966bytes (caller source plus retained history, decoded-grid working bound and output/scratch). All first/middle/last selections agree. This admitted-byte peak is not process RSS; codec/header metadata and other parser allocations remain outside the existing major-buffer accounting model. The600frame normal regression runs three selections within256KiB, measured peak244520bytes/final0, with exact-peak retry/one-byte-short controls. Tiny reference files are CC0 and reproduced by the same C producer.

## Exploratory warm timing

One fresh process per case,3warmups/9timed load+prepare iterations; dev optimized/shared host, warm filesystem, no decoded-image cache. CPU-ready p50 after fix, first/middle/last in milliseconds:

| Frames | First | Middle | Last | Admitted peak bytes |
|---|---:|---:|---:|---:|
|3|0.042750|0.042416|0.057792|67712|
|30|0.056834|0.055875|0.059875|75546|
|300|0.234417|0.232084|0.218917|155552|
|3000|1.852875|1.920292|2.159125|955966|

No paired performance conclusion: small sources vary/noisy;3000previously failed so there is no valid before latency. All9previous successful prepared pixel hashes remain identical after fix. Timing excludes startup, destruction, pixel hashing and GPU. Full history is still read per uncached selection, so cost grows with frame count; directory/index-based skipping or bounded shared contexts remain optimization work.

## Validation and remaining admission boundary

Final decoder222passed/5ignored; app288passed/81ignored:510ordinary passes,0failures. Separate actual Metal Apple M4 Max12presentation native/prepared RAM+swap metadata/bit/range test passes after the model change; final root0. No fresh color-library/all-target/all-feature/strict-Clippy release claim. Benchmark/probe binaries were built after the production fix; later source edits only added the normal regression.

Source inspection also confirms a remaining boundary: jxl-oxide UninitializedJxlImage::try_init feeds its buffered remainder into image.inner.feed_bytes_inner before returning. Our initial chunk is4096bytes and its initial codec tracker is unlimited; a group may reserve its declared full size at that stage before the later working admission. Therefore header initialization cannot be claimed as parser-only scratch or strict pre-allocation admission. This turn does not fix that stage or untracked frame-header metadata. Next memory work must separate header-only initialization from buffered frame feeding (or provide allocation-aware admission) before claiming all transient codec allocations are covered. Exact-parent/final-credit tests above establish only the current declared major-buffer model, not whole-process safety/RSS.

Broader container layouts, multi-megapixel/complex references, corruption, cancellation within library operations, large live playback, physical HDR and deferred NVIDIA remain unqualified.100-format objective is still incomplete.

Reproduce python3 scripts/generate-jxl-scale-fixtures.py --output CORPUS [--counts 600]. Build raster_load_bench and jxl_selected_probe with cargo +stable build -p rrrah-decode --example raster_load_bench --example jxl_selected_probe --locked --offline. measure.py CORPUS --bench BENCH --report REPORT records warm cases; run jxl_selected_probe FILE INDEX COUNT for independent16bit/count/timing/peak gates. Standard decoder regression reads committed600frame control. CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 for these runs.
