# Live raster repeated navigation: incomplete endurance

The debug-only recovery probe now accepts RRRAH_QUALIFY_NAV_ROUNDS (default1, bounded1..10000). Each EXR→PSD→EXR cycle requires new actual presented generations on the same recovered Metal device. Before every return the cache-hit marker resets; source decoding for EXR remains forbidden after initial guard installation. The opt-in timer expands with round count. Ordinary operation and release builds do not enable the probe.

Two committed, authored/public fixture copies are used: associated-emission-rec709.exr and psd-tools-gray0.psd. One-entry 8MiB raster RAM cache, 16MiB/count4 swap, 8MiB restoration and64MiB managed root; prefetch disabled to isolate foreground swap return. The initial setup used symlinks, which folder discovery intentionally excludes; its failure log is retained and not treated as product evidence.

First real run presented294 full cycles and recorded294 guarded foreground swap hits. It then failed because the second file was absent from the active gallery; a swap-disk-quota warning preceded failure. Both source files still existed afterwards. The cause of the gallery context change is unresolved.

The repeat presented202 cycles, paused, resumed to203 cycles at118092ms, and stopped progressing again. It recorded203 guarded swap hits. A process sample during a pause showed the main thread in the macOS event wait; this does not identify the cause. The external240-second watchdog terminated this run. Window visibility/occlusion or another cause remains unconfirmed; user confirmation was requested.

Neither run satisfies the300-cycle gate. The successful intermediate presentations and swap-hit records are bounded observations, not long-duration GUI qualification or independent framebuffer/HDR proof. Device-loss notifications were simulated; physical device loss was not injected. No production navigation/cache fix has been made on this evidence alone.

Ordinary application regression:288 passed,82 ignored. Formatting and tracked whitespace checks passed. Runtime logs, launch commands and result summary are retained. Next steps: log active gallery contents/scan generations and presentation-wait state, confirm visible desktop, then reproduce before changing runtime behavior.
