# Live application GPU lifecycle

Current full rrrah application opened the pinned shifted-window EXR in its actual event loop. Existing explicitly opted-in debug recovery probe changed pan/zoom, split comparison and telemetry cards, presented them, then simulated a device-loss notification for both main and telemetry devices. Device epochs changed [1,2] to [3,4]; both new devices presented two frames and completed GPU waits. Selected source/page, view, comparison and card state remained bit-exact. Process exited successfully via the probe.

Command: RRRAH_QUALIFY_GPU_RECOVERY=1 cargo +stable run -p rrrah --bin rrrah --offline -- tests/fixtures/raster/float-shifted-window-negative.exr.

The two device-lost ERROR log lines are intentional injection evidence. This verifies simulated lifecycle recovery in the full application, not a real driver reset/hang, full navigation/cache/prefetch, physical HDR accuracy or NVIDIA. Live HDR surface evidence is in the separate live-raster and live-raw reports.
