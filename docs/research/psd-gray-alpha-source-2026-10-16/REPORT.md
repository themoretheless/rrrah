# Original grayscale merged transparency

Pinned psd-tools commit9ea48dad04a32a6977b3109ebfae23b667590d42 gray0.psd and gray0.psb are400x359 grayscale8bit/RLE, twochannels and negative layer count indicating merged alpha. Source URLs/SHA256 in inventory.json; MIT license retained.

Independent stdlib extractor walks PSD/PSB sections, parses RLE rows and stored gray/alpha planes, removes white matting in integer arithmetic, and emits RGBA plus ICC1039. Both containers produce identical outputs. Separately installed test-only psd-tools1.25.0 topil(apply_icc=False) exactly matches all574,400 output bytes for both originals, including fractional alpha. FFmpeg's output differed in52,506channels and was not used as the straight-alpha oracle. Product gains no Python/psd-tools dependency.

Production previously refused every grayscale merged-alpha composite despite already expanding gray toRGB before8bit white-unmatting. The guard now refuses only non8bit merged alpha. New originals verify every native byte, profile bytes, fractional alpha, exact tracked peak862,032bytes /retained574,832, one-byte-short refusal/final0, clone-last-owner release0. The16remaining high-precision refusal cases still pass. Layered library69passed/2ignored.

This enables native8bit grayscale merged transparency for the supported saved-composite path. Public source-route, display ICC, cache/swap/Metal, Photoshop layer recomposition and high-precision merged alpha are separate follow-ups.


Pending follow-up: added public source/decoded output exact-peak tests for both originals and a143,600-pixel/alpha native display comparison against independently generated256-entry LittleCMS grayscale reference (6144bytes). Those new Rust tests have not yet run. The6MiB restore aggregate pipeline remains live waiting for shared Cargo build lock; no pass is inferred from oracle generation or test source. See color-oracle-status.json.


Reproduced independent oracle: retained verify-independent-oracle.py checks both pinned source hashes, every RGBA byte and ICC1039 through psd-tools1.25.0 (attrs25.3.0). Both originals have143,600pixels,17,502partial-alpha pixels and254distinct alpha codes; output differences0 and432ICC bytes identical. Reproduction passed; result reproduced-independent-oracle.json. Pending Rust public/color/Metal tests are still separate and not inferred from this oracle run.


High-precision future evidence: generated8small psd-tools1.25.0 writer/reader controls: RGB/gray alpha ×16/32bit ×PSD/PSB, four alpha values0/.25/.5/1, float32colors include2/4HDR. Generator supplies mathematically prematted channel planes, independent writer serializes RLE/sections, independent reader produces normalized native float references. Source/oracle hashes and values retained under high-precision-controls/manifest.json. These are authored producer controls, not original Photoshop files; native high-precision support remains refused/unqualified and has not been enabled from generation alone.


High-precision reference validation: retained stdlib verify-high-precision-controls.py independently parses PSD/PSB length widths, per-version RLE row lengths and PackBits, reconstructs16/32bit planes and performs f64 white-unmatting. All8controls agree with psd-tools normalized float reference within predeclared2e-6; observed maximum5.820855e-11, alpha matches. Result high-precision-controls/independent-f64-check.json. This validates authored reference data, not native support or interoperability with original Photoshop producer files.


Native high-precision implementation pending runtime validation: extended saved-composite white-unmatting toRGBA16 using wide integer arithmetic andRGBA32Float using f64 intermediates, preserving above-one/signed finite color and zero-alpha stored color. Removed obsolete blanket high-precision refusal and replaced its refusal-only test with8independent producer-control precision/HDR/exact-budget/one-byte-short/last-owner checks. Rustfmt parser accepted the source; Rust test execution is still pending shared Cargo-lock. This implementation is not yet validated or claimed as supported/qualified. The existing live session5651 remains waiting, not restarted.


Isolated current-source native validation: bypassed shared Cargo build lock without restarting waiting session5651 or writing shared artifacts. Compiled current psd.rs directly into a temporary rustc test harness, using fingerprint-matched cached layered/error aliases, core and flate2 dependencies. All16PSD-module tests passed, including8independent high-precision alpha controls, original8bit grayscale alpha, earlier original16/32bit composites, exact/short budgets, bounded ZIP/RLE and last-owner release. Initial independent producer32bit controls exposed74byte hdrt/version3 versus observed original112bytes; both observed lengths are now admitted, unknown lengths remain refused. Logs/harness retained. This is a current PSD-module check against cached dependencies, not a Cargo/workspace/public/Metal gate. High-precision native behavior is validated on authored controls; original high-precision alpha corpus and public/display/cache transport remain open.


Current-source isolated color validation: compiled raster_color.rs directly as a temporary rustc test harness with fingerprint-matched cached moxcms/core/vector/image/thiserror artifacts. Initial standalone link needed the installed dav1d library search path (recorded); after adding it all15color-module tests passed. Original143,600pixel grayscale alpha display compares allRGB components against256-code independent LittleCMS double oracle; maximum absolute error1.192093e-7 <=2e-5, every alpha exact, final tracked credit0. Other module tests cover linear HDR/identity profiles, full-u16 gray and68cancellation points. This is not a full Cargo/workspace or public-route/Metal gate; session5651 remains waiting and public source test still pending. Commands/artifacts/harness/logs retained.


Current-source component Metal validation: compiled current psd.rs, raster_color.rs and icc_colorimetry.rs into a separate temporary binary with compatible fingerprint-matched cached core/cache/GPU/support libraries (commands retained), without touching shared Cargo artifacts or restarting session5651. Native original PSD/PSB→currentICC→RAM(count1)→swap→restore passes on Metal Apple M4 Max. Two143,600pixel sources,8full SDR/half readbacks bit-exact before/after restore;2writes/2reads; foreground resident RAM hit under completely filled8MiBroot; final root0. Restore aggregate6MiB supports foreground+next; RAM3MiB/disk16MiB. This is component integration, not public decode_image/full App DecodeGate/prefetch; full Cargo test still pending.


Explicit linear policy and high-precision component Metal: PSD/PSB decoder ignored assume_untagged_linear_srgb. It now honors that explicit flag for untagged data, with ICC still taking precedence (isolated16PSD tests passed). Current PSD/CMS component harness checks all8authored16/32bit RGB/gray alpha PSD/PSB controls against independent normalized samples, prepares explicit linear display, writes/restores swap and compares SDR/half frames exactly. Metal M4 Max:32high-precision readbacks,8writes/8reads; float samples and half readback retain above-one HDR; final credit0. Rechecked two original gray8files:8readbacks/2writes/2reads exact. Logs/source retained. This is component integration with fingerprint-compatible cached supporting libraries, not full Cargo/public/App navigation. Original Photoshop high-precision-alpha corpus remains unqualified; session5651 still pending independently.


Bounded alpha processing: saved-composite white-unmatting previously iterated the whole image without cancellation checks. Extracted the private conversion pass and now checks the request every4096pixels and before return forRGBA8/16/float32. Deterministic test cancels at the second block, proves exactly the first4096pixels processed and all4097tail pixels unchanged at every precision, preserves alpha, then drops the partial private buffer and verifies final credit0. Decode propagates cancellation before publishing a DecodedRaster. Current-source isolated PSD suite17passed; existing independent originals/high-precision controls still pass. Evidence white-unmatting-cancellation.log. Full Cargo/App gate remains pending.


Float alpha arithmetic boundary: finite stored float colors with tiny positive alpha could overflow to infinity during white-unmatting. The conversion now refuses newly generated nonfinite float32 output with a specific InvalidPsd diagnostic; it does not silently clamp HDR. Tests cover smallest normal and subnormal alpha with f32::MAX, final memory0 on refusal; alpha0/1 preserve representable f32::MAX, signed-2 and HDR4 exactly. Current-source isolated PSD suite18passed; existing independent controls still pass. Evidence white-unmatting-overflow.log. Cargo/public/App qualification remains separate pending.


Opaque float identity correction: with alpha1, arithmetic(c+1)-1 could erase tiny float samples and signed zero even with f64 intermediates. White-unmatting now runs only for strictly partial0<alpha<1, preserving opaque stored float bits. Regression checks negative zero, smallest subnormal, smallest normal, ±1e-30 and explicitNaN payload bits at alpha1; last-owner credit0. Current-source isolated PSD suite19passed, including independent high-precision controls and cancellation/overflow. Evidence opaque-float-bit-preservation.log. RawNaN preservation does not imply NaN is accepted by display preparation.


Restore aggregate quota verified directly: with3MiBrestore and one held2,297,600byte float frame, second restoration returns typed Capacity. Partial reservations release, disk usage remains identical, and current fullSDR/half Metal frames remain bit-exact. Dropping the held owner frees credit; same object restores exactly on retry. Two writes/two successful reads/zeroIOerrors/final0. This experimentally confirms the smaller quota issue underlying the initial blocked-App-gate test; it does not claim the App test passed. Extended component binary also rechecked original gray8and8high-precision controls against current PSD/CMS source. Evidence restore-quota-component-metal.log/result.json; retained standalone harness uses cached supporting libraries.


Catalogue reconciled with implemented native alpha/linear policy and isolated versus full-App evidence boundaries. Formatted only the owned unmatting helper; current-source isolated19PSD tests pass afterward (owned-helper-format-regression.log). No whole-file/repository formatting or completion-count promotion.


## Cargo follow-up completed in isolated target

Shared target contention persisted about50minutes with confirmed live neighbouring builds. Cancelled only own waiting Cargo PID59358 (exit130), preserving neighbouring processes and shared target. APFS copy-on-write cloned completed deps/build/fingerprint cache into /tmp/rrrah-alpha-isolated-target-20261016, without lock/incremental trees. Cargo then validated/rebuilt required artifacts independently. This migration responds to confirmed shared-resource contention, not an observation timeout or unverified process failure. Target-isolation metadata and allcommands/results retained.

All four ordinary Cargo stages passed: layered72/2ignored; animation34/1ignored; original gray8 public decode_image exact source/output/profile peak and one-byte-short test1passed for bothfiles; App original_gray_alpha CPU and actual Metal tests2passed. M4Max App pipeline:2original cases/12readbacks,2full-root-pressure RAM hits/2swap writes/2restores with DecodeGate held, final credit0. Restore6MiB permits foreground plus neighbour under unchanged8MiBroot. This completes the previously pending original grayscale8public/App gates; earlier blocked/isolated-only statuses are historical and superseded in this scope. Authored high-precision controls pass the Cargo layered suite and component Metal test; their full-App transport and original Photoshop high-precision corpus are still separate.

Evidence: cargo-qualified-result.json, cargo-stages.json, cargo-{layered,animation,public,app-metal}.log. This is not all-targets/all-features/workspace/release or all100format qualification.


## Authored high-precision App-loader transport qualified

Added a separate corpus variant for8authored RGB/gray16/32 alpha PSD/PSB controls, preserving the original gray8scenario independently. Exact native little-endian oracle samples are independently reconstructed by stdlib PSD/PSB/PackBits parser and arbitrary-precision rational16bit unmatting, checked against psd-tools normalized float reference; generator and manifest retained. App request explicitly enables linear-sRGB policy, so public decode/prepare uses current production paths.

Initial test reused gray scenario count4for8objects, correctly evicting early keys; preserved failed log. High-precision scenario now explicitly uses count16, RAMcount1/root8MiB; restore6MiB. Four Cargo App-loader tests passed serially: CPU+Metal for2original gray8sources and CPU+Metal for8authored high-precision sources. M4Max high precision48readbacks,8RAM pressure hits,8writes/8restores with DecodeGate held and final0. Original gray8retains12readbacks/2writes/2restores/final0. No fallback decode allowed. Both precision/alpha and prepared float bits survive transport.

Evidence high-alpha-app-result.json, alpha-app-qualified.log, high-alpha-app-count4-failed.log, tests/fixtures/raster/psd-tools-alpha-manifest.tsv. This closes component-only transport limitation for authored high-precision controls, not original Photoshop high-precision-alpha interoperability, live GUI navigation or full workspace/all100qualification.


Broader current Cargo regression: rrrah-decode full library196passed/5ignored; ordinary rrrah viewer278passed/78ignored. Initial viewer run had one obsolete text assertion("finite and normalized") after the ICC input-contract diagnostic became profile-dependent. Updated only this expectation to exact RasterColorError::InvalidInput.to_string(); the test still requires single load attempt, no new peak/retained credit, one cache entry and pointer-shared previous visible pixels. Full viewer suite then passed. Evidence cargo-decode-full.log, cargo-viewer-initial-diagnostic.log, cargo-viewer-full.log, regression-result.json. No ignored test is counted as validated.


## Live Metal/HDR alpha presentation verified (2026-10-16)

Completed ten native window probes: two original grayscale8 PSD/PSB sources with embedded ICC and strict untagged policy, plus eight authored RGB/gray16/32 alpha PSD/PSB controls with explicit linear-sRGB policy. All ten exited0, presented RGBA16Float / ExtendedSrgbLinear on Metal without validation errors, confirmed resize and active OS HDR headroom greater than1, and released tracked source credit to0 after upload. Total1715frames,1415after resize.

Revalidated every source/log SHA256, frame/resize counts, actual Metal adapter, OS HDR log and unchanged probe binary against live-hdr/result.json; evidence live-hdr/evidence-revalidation.json. The completed run was inspected without repeating it. These are native probe presentation and resize checks, not calibrated physical luminance/color, full viewer navigation, original Photoshop high-precision-alpha interoperability, NVIDIA/CUDA or qualification of all100formats. Authored producer controls remain identified separately from original grayscale sources.
