# Metal upload refusal and recovery

The existing result.json and metal-test.log retain the earlier4000×3000/192000000-byte source upload/cancellation evidence. They are not replaced by the small constrained-device case below.

## Constrained device limits — 2026-10-24

New required offscreen test raster::tile_admission_tests::constrained_metal_refusal_partial_cancel_and_retry_keep_previous_frame runs on actual Apple M4 Max Metal. Its wgpu device deliberately negotiates max_texture_dimension_2d64 and max_texture_array_layers2, both asserted after device creation. This exercises device limits with small allocations rather than pretending to allocate an enormous source.

Both Rgba8UnormSrgb and Rgba16Float attachment paths are checked. An admitted1×1 linear HDR source owns16 logical GPU bytes; exact4×4 attachment readback and all renderer uniform bytes establish the old visible state. A valid129×1 source would need three layers and returns TextureLimit before quota changes. A valid2×2 replacement returns typed Memory refusal while the192-byte GPU budget's remaining credit is held. After releasing that pressure, a generation predicate cancels only at the third post-admission callback: after quota admission, layer entry and one queue.write_texture transfer, immediately before publication.

The cancelled partial upload preserves every old attachment/uniform byte. A subsequent render/readback flushes queued work and checks GPU/upload reservations return to16/0. The wgpu validation error scope is empty. Retry uploads the2×2 replacement, changes the attachment, and retains64/0 logical GPU/upload bytes. Dropping the renderer after completed work returns both quotas to0.

Result:1 passed/0 ignored/0 failed, two attachment formats (constrained-device-current.log). Command: cargo +stable test -p rrrah-gpu --lib constrained_metal_refusal_partial_cancel_and_retry_keep_previous_frame --locked --offline -- --include-ignored --nocapture --test-threads=1.

This step adds a regression test; production admission/rollback behavior already satisfied it. It verifies rendering and queue/resource lifetime under negotiated limits and logical admission pressure. It does not reproduce physical VRAM exhaustion, device loss, viewer surface presentation, slow storage, physical HDR or NVIDIA. The CPU inputs and output attachments in this new case are deliberately small; the earlier12MP evidence remains separate.

Source SHA-256 (crates/rrrah-gpu/src/raster.rs): e806a3a66df65948e143fdf395bf5b302a8f3281a6a518883dc1dadb005e3b80.
