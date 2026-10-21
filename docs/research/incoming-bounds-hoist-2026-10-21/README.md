# Incoming bounds hoist — rejected

The pilot explicitly cached incoming triangle bounds once per paint. Production source was restored; `rejected-pilot.patch` preserves the experiment and `restored-state.json` records restoration evidence.

Six AA controls and sixteen alternating AB/BA pairs used optimized release code with one codegen unit and thin LTO. Mean time reductions: pair -1.473%, dense50 -1.770%, dense75 -1.189%, sparse64 +0.617%. The first three regress; sparse64 confidence interval crosses zero. See `release-summary.json` for intervals and `release.log` for raw observations. No performance improvement is accepted and no cause is claimed.
