# Live navigation diagnostics

Opt-in debug probe now emits phase, completed rounds, selected path, gallery size/index, pending scan, current/ready/presented generations, frame count and status every two seconds. Missing-gallery diagnostics retain each candidate path and its canonicalization outcome. No production rendering or cache logic was changed.

The diagnostic300 run completed all300 cycles, exit0, in13.32 seconds (11.90 seconds active probe). Each return to EXR recorded a foreground swap hit with fallback decoding forbidden; the gallery had two files at every heartbeat and the recovered Metal device remained unchanged. This passes the bounded300-cycle window gate, not a long-duration or large-collection GUI gate.

The diagnostic3000 run ended after108 completed cycles. Its active gallery had changed to the separate rrrah-jxl-scale directory, containing four real JXL paths. Canonicalization succeeded. This narrows the earlier missing-file failure to a gallery-context change, not demonstrated loss of a file or spontaneous cache corruption. Code routes folder switching through folder clicks or dropped/opened paths, but the exact initiating event was not logged; external interaction is not established as fact.

A further3000-cycle attempt used a nested isolated corpus with no unrelated sibling folders. It terminated normally after129 completed cycles (130 foreground swap hits) without a LIVE_RECOVERY_FAIL marker; the final recovery failure was None because the target was never completed. In the current source, an already-running window can exit on CloseRequested; initialization errors were absent. The event itself was not recorded.

No further windows were launched after this evidence; desktop/window availability was requested from the user. Neither3000-cycle attempt is qualified. The earlier240-second presentation stall also remains unexplained. Do not erase the failed pilot evidence or treat the300-cycle success as its root-cause fix.

RUST_LOG=rrrah=debug and RRRAH_QUALIFY_NAV_ROUNDS=300 or3000 were used with the prior guard/count1/swap configuration. Exact commands, complete logs and summaries are retained. Formatting and whitespace checks passed.
