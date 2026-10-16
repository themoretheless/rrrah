

### Current Metal cache/swap integration (2026-10-16)

Metal Apple M4 Max: LEGACY_RASTER_PIPELINE cases=2014 gpu=true readbacks=12084 ram_pressure_hits=2014 swap_writes=2014 swap_reads=2014 blocked_decoder=true final=0. Existing corpus gate passed on current decoder/color/cache code; GPU results checked before/after RAM and swap, with decode blocked during restores. No physical HDR/NVIDIA or full 100-format qualification implied. Evidence: `research/current-metal-cache-swap-2026-10-16/metal.log`.


Current viewer integration: 276 ordinary tests passed, 76 ignored. Separate actual M4 Max Metal HDR/TTL gate passed 128 restores without decoder fallback plus two real preloads under a blocked decoder; all 130 finite binary16 HDR frames were exact, including above-one values, with final tracked credit zero. Evidence: `viewer.log`, `hdr-ttl-metal.log`. These are offscreen GPU readbacks, not physical HDR-display validation.
