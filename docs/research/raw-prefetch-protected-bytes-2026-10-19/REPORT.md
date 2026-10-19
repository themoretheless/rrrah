# RAW protected-byte lower-bound admission

The shared cache policy now exposes a non-evicting minimum-weight admission query. It combines count eligibility with capacity remaining after other protected entries. Existing-key replacement excludes that key's old protected weight. Summation saturates on overflow; minimum above capacity is rejected. The lease layer reconciles released leases first. The query does not renew TTL, change recency or remove membership; its protected-weight scan is O(number of protected entries), not an O(1) byte-accounting claim.

MosaicRamCache forwards this query. Retained RAW prefetch uses the conservative minimum two bytes: every valid nonempty u16 sensor needs at least one sample. This detects an exhausted protected byte budget before swap, persistent restore or native sensor decode, while leaving resident keys eligible. Recipe and fingerprint resolution still occur. It preserves the separate gallery disk warmer.

This is not a dimension/decoded-size estimator. If protected entries leave a small positive allowance that exceeds two bytes but cannot fit a particular sensor, decode may still be attempted. The ordinary final byte admission remains authoritative. No throughput, RSS or general avoidance of all oversized RAW decoding is claimed.

The byte-only regression fills an eight-byte cache with selected and leased mosaics, confirms count-only admission would pass while minimum-weight admission refuses, checks existing-key and oversized-bound behavior, then verifies release reconciliation without eviction.

The original CR3 navigation test now checks unchanged peak managed sensor usage for both count-limited and byte-only cases in both directions. Malformed neighbour, protected nearest, ready hit under full managed pressure, far admission after release and final zero all pass. This remains one original sensor source with temporary distinct timestamp/path keys, not multiple independent cameras. No source media is stored in the report.

151 cache tests and44 memory tests pass (195 combined,three ignored);291 ordinary app tests pass (82 ignored); the strengthened original-CR3 manual case passes. Formatting and whitespace checks pass.
