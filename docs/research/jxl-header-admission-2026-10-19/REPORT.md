# JPEG XL header-only initialization before tracked frame allocation

## Reproduced early allocation

Before this patch, UninitializedJxlImage::try_init parsed metadata and then immediately fed its buffered codestream tail into frame processing. The rrrah caller supplied an unlimited initial codec tracker before knowing dimensions/working requirements, so compressed frame groups could allocate before working admission. A valid boxed-alpha control with tracker limit0 failed header initialization, reproducing the allocation boundary; before.log preserves the failed assertion.

## Optional local API, pinned version and caller ordering

Vendored the already-used jxl-oxide0.12.6 package (168KiB source), preserving original MIT/Apache licenses, registry checksum/provenance and all original files. Only upstream src/lib.rs changes; upstream.patch and origin.json record the delta. Cargo.lock removes that package's registry source/checksum when selecting the local path; versions/transitive dependencies do not change. This is not a replacement codec or new production libjxl dependency.

Added try_init_header_only plus resume_frame_loading. Header-only mode stores the already decontainerized buffered codestream tail without processing frames. After the caller reserves output/working memory and sets the finite tracker allowance, resume processes that tail exactly once through the inner frame reader. It bypasses the container parser for this already-parsed data, preserving raw/boxed framing. feed_bytes and finalize resume a pending tail automatically; explicit resume is idempotent after success. On failure, callers discard the image as for a failed feed operation.

Original try_init remains eager. Existing builder.read/image adapter callers retain that behavior; legacy RGB/alpha probes still match all6independent presentations. New public rrrah decoding specifically uses initial tracker0 and header-only mode, checks cancellation after header initialization, admits its existing output+working reservations, expands the tracker only then, checks cancellation again and resumes. Working-credit declaration precedes codec state so error/success destructors release tracked buffers before credit. Reservation formulas/global/user limits are unchanged by this turn.

## Runtime evidence

The new boundary test initializes six complete controls under zero codec capacity: raw still, boxed still, RGB8 animation, RGBA16 fractional/finite animation, signed linear RGBA32F animation and600frame history. Each is tested through explicit resume (called twice), automatic empty feed and automatic finalize:18paths. Before admission all have0loaded frames and are not loading-done. After positive admitted storage, all finish loading and the last displayed frame renders; root credit ends0. The unchanged eager API under tracker0 still refuses the buffered payload, verifying its legacy limit behavior rather than silently disabling tracking.

Final public decoder223passed/5ignored; ordinary app288passed/81ignored:511passes/86ignored/0failures. Actual Metal Apple M4 Max12presentation RAM/native+float swap metadata/bit/range gate passes. Initial raw/boxed JXL,16bit samples,ICC, prefixes/invalid-index refusal, signed float/HDR source and same-root recovery controls remain included. All final managed root credits0. No all-target/all-feature/strict-Clippy release gate is asserted.

Public selected-source probe requalified all12first/middle/last positions across3/30/300/3000frame controls against independent libjxl C native oracles, full count/selection and fractional repeat metadata. Exact-peak success/one-byte-short typed refusal/retry and final0 remain passing. New prepared pixel hashes match every prior successful source hash. Warm timing snapshots are exploratory, not a paired speedup claim. Full history is still loaded per uncached selection.

## Exact scope and remaining memory work

This closes early allocation for codec group/grid storage governed by AllocTracker in the rrrah path. It does not make every allocation tracked: source/header buffering, ICC decoding, frame/TOC descriptors, profile/transform metadata and allocator overhead remain outside the existing major-buffer reservation model. Preview and other producer-specific paths need further independent qualification. Zero codec capacity does not mean zero parser allocation or a whole-process RSS bound. These gaps must remain open before claiming complete transient memory control.

The optional upstream-compatible eager API is intentionally not converted to managed admission; rrrah opts into the deferred API. Broader JXL producers, malformed metadata, cancellation inside indivisible parser/render calls, real-window endurance, physical HDR and deferred NVIDIA/CUDA also remain unqualified. The100-format objective is not complete.

## Reproduction

Use the committed lockfile/local patch, stable toolchain and CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016. cargo +stable test -p rrrah-decode --lib --locked --offline; cargo +stable test -p rrrah --bin rrrah --locked --offline. Actual hardware: RRRAH_GPU_BACKEND=metal cargo +stable test -p rrrah --test pdf_readback --locked --offline jxl_animation_frames_preserve_metadata_ram_swap_and_metal -- --ignored --exact --nocapture. Rebuild raster_load_bench/jxl_selected_probe/jxl_animation_probe before reproducing external scale and legacy probe data; source manifests/generators are in the preceding frame-scaling evidence.
