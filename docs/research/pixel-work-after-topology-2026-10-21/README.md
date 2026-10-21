# Kernel work profile after rounded-corner correction

The exact same temporary category patches from ../pixel-work-profile-2026-10-21 were applied to the corrected topology implementation. After successful build, the executable was copied to a frozen external path and hashed; sources were restored before execution. The page render completed with exit 1, not a timeout. All four attempts remain incomplete at 100M work units. categories.json contains exclusive category counts plus sampler remainder; every row sums exactly to 100M. These are budget units, not CPU time.

Subtraction remains 46.11M/54.57M/41.44M/21.12M for 1455/323/245/788 patches. Retention and old-region traversal dominate the last attempt. Therefore exact redundant unit-domain clipping is the next experiment, now separately requalified after correcting the isolated geometry defect. No quota increase, full-page performance win, full-format or hardware completion is established by these measurements.
