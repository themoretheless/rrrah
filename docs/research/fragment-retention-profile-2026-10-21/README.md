# Retained-fragment event profile

Pinned VectorApple.ai page 0 ran to terminal renderer exit 1 in a frozen counter-only executable. All four attempts still spent 100M and returned incomplete. Both instrumented source files were restored before the binary ran. Patches, decoder hash and full stderr are retained. These are events in tentative/cancelled paint operations, not final committed-region counts or CPU timing. Push counters are attempts immediately before a fallible push.

The old-region partition is incoming-empty/disjoint/dominant/general. One 245-patch visit exits during priority checking and remains unclassified; counts.json records that remainder rather than silently attributing it. Keep counters distinguish all calls, positive input classifications, actual convex-union attempts, successful tentative merges and attempted output pushes. Subtraction has separate calls/bbox rejection counts.

10.16M–32.12M old regions per attempt take the disjoint branch. Total old visits are 13.14M–33.39M. Positive keeps total 14.20M–34.07M, while successful merges are only 792–2561. Rechecking unchanged old polygon moments and copying fixed-size Fragment records therefore occur at much higher volume than actual merges. This motivates a cached publication-time area classification and, separately, future evaluation of immutable/shared fragment storage. No store/arena rewrite is implemented by this profile.

The source footprint during this profile is 496 bytes per Fragment, before adding an area-cache flag. No format/cache-pressure/hardware completion is established.
