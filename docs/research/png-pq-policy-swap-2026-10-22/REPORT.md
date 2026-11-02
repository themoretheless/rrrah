# Narrow-PQ PNG display-policy isolation across count-one RAM and swap

Verified the existing viewer cache key includes PQ reference-white float bits and all three HLG policy float bit patterns. Earlier PQ cache tests used AVIF and multiple RAM entries. The new app regression exercises the authored chromatic/alpha narrow-PQ 16-bit PNG using public display loading and the actual generic swap worker.

Two initial decodes use PQ reference whites 100/200 cd/m². Every RGB component must scale by one-half, including negative and extended values; alpha must remain unchanged. RAM is restricted to one entry/49,152 bytes. Swap index allows two entries within 1 MiB, queue one frame/one entry, restore quota two frame weights, all under a shared 512 KiB root.

Four alternating lookups then forbid decoder execution and must increment successful swap reads exactly once each. All restored float bits match their own policy's initial snapshot. After each queue drain only the one RAM frame retains credit. Final cache teardown returns root usage to zero; swap errors remain zero. Production cache implementation was unchanged: this follow-up adds cross-policy transport evidence.

Initial setup used a one-frame restore quota and failed at the decoder-forbidden boundary. Once a restored frame is visible, it consumes that entire restore pool; preserving it while admitting a replacement requires a second frame of restore headroom. The source implementation aggregates live restored ownership before admission. The corrected test keeps RAM count/bytes at one frame and changes only restore quota to two frames. rejected-one-frame-restore.log is retained; it is a rejected test admission setup, not evidence of policy-key collision.

Focused test: 1 passed / 0 ignored / 0 failed (focused.log). Full app binary gate: 297 passed / 84 ignored / 0 failed (app.log). Hardware/manual ignored cases were not exercised by this app sweep; preceding narrow-PQ Metal evidence remains separate.

The first compile was temporarily SIGSTOP/SIGCONT-coordinated with the shared GPU measurement window. The same process/session was polled and resumed rather than restarted on timeouts. Compiler child identities progressed from 50236 to 70186 under cargo49496. Explicit release was received before follow-up runs. Suspended compilation elapsed time is contaminated and is not a build/startup/runtime performance result. STATUS.md retains preparation and execution history; this report and terminal logs supersede its earlier pending statements.

## HLG policy lifecycle follow-up — 2026-10-24

`hlg_all_policy_fields_survive_count_one_swap` now compares four independent uncached interpretations of the existing chromatic HLG AVIF (white/peak/gamma changed separately). All four outputs are distinct. Count-one RAM and four disk entries preserve exact float bits through eight alternating restores with the decoder forbidden. Missing policy, zero white, NaN peak and infinite gamma are refused despite warm entries. Queue/restore quotas are one/two frames, swap error count is zero, and final managed root credit is zero. This is functional policy/cache qualification, not timing or broad HDR-format completion.

Focused gate: 1 passed / 0 failed / 0 ignored. Full application gate: 319 passed / 0 failed / 84 ignored; 27.81 s runtime, superseded by the later full-suite raw output in `../prefetch-count-swap-hit-2026-10-16/app-current.log`. Command: `cargo +stable --config 'profile.test.package.rrrah.opt-level=0' test -p rrrah --bin rrrah hlg_all_policy_fields_survive_count_one_swap --locked --offline -- --nocapture`; full suite ran the resulting executable with `--test-threads=1`. The full suite also executed `native_counter_tracks_selected_wgpu_allocations_metal`; no GUI window or physical HDR presentation was involved.

Initial compilation hit concurrent new edit-qualification source errors: missing macOS dependency (added concurrently) and wgpu30 mapped-view APIs. Two call sites were adapted to handle the Result and write through `slice(..)`. The successful build was used for all reported gates. The test process initially waited in dyld; it was preserved and subsequently completed normally.

## HDR RAM and disk TTL lifecycle — 2026-10-24

`hdr_expired_ram_and_swap_redecode_without_invalidating_held_pixels` exercises native narrow-PQ chromatic/alpha PNG and HLG chromatic AVIF. Each retained float frame is admitted to RAM, expires, spills to disk, and expires there under TTL=0. A foreground-helper lookup must decode once with no additional disk restore. Fresh and still-held old pixels remain bit-exact and independently owned; final managed usage is zero. Immediate expiry is deterministic; this gate does not establish wall-clock/idle-timer scheduling or slow-disk performance. Existing nonzero timer gates remain separate.

Focused TTL test: 1 passed / 0 failed / 0 ignored. Full application suite after correction: 320 passed / 0 failed / 84 ignored, 29.08 s runtime (`../prefetch-count-swap-hit-2026-10-16/app-current.log`). This supersedes the earlier successful full-app log rather than adding another duplicate. App-only test opt-level=0 override retained.

The first full run had 319 passed / 1 failed / 84 ignored (`ttl-first-full-failure.log`). The failure was in the concurrently added atomic-document crash test: commands/compaction children hold the recovery-journal lock, but the competing parent attempted an XMP write and incorrectly expected contention. The test now targets journal for all non-XMP stages. No production lock behavior was changed. Focused crash test passes all stage/kind combinations (`atomic-document-focused.log`), followed by the green full suite above.

The previous 320-pass successful suite log was superseded by the current 321-pass full-app run linked above; original failures and focused controls remain retained.
