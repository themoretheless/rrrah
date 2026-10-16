# Admit the returned PNG ICC copy before allocation

Current DecodedRaster already includes ICC capacity in cache weight, shares profile/credit through Arc, and the public outer raster wrapper already admitted ICC before returning the frame. The initial outer-layer control passed and is retained in `existing-outer-passed.log`. Thus this change does not fix a missing retained-frame ICC charge; it moves existing late admission earlier.

A regression targeting the inner decoder reproduced an uncredited1024-byte pixel-only return where1024pixels+588ICC bytes should already be held. The generic PNG inner route now borrows the stored codec profile length and reserves its application-owned copy before requesting icc_profile(). It attaches that reservation to the returned shared color metadata. The outer wrapper observes the attached credit and does not reserve twice. The cICP-priority path has no used ICC and acquires no profile credit. Other image formats keep their existing outer admission.

Exact1612-byte root budget passes for the borrowed-source PNG control; one byte less refuses with typed Memory, rollback and retry. Cloned frames share both allocations. Dropping the last raster owner releases588 profile bytes while a separately held pixel buffer retains1024; dropping it returns root use to0. Cache capacity remains the already-correct1612 bytes. Full public decode library: **203 passed /5 ignored /zero failures**.

This protects the additional owned profile copy before its allocation. The PNG codec has already parsed/inflated its private stored profile during construction; that private copy, row/unfiltering buffers and fixed decoder tables are still outside complete shared-root accounting. Source bytes are caller-borrowed in the inner control. No complete memory-budget, arbitrary ICC interoperability or hardware qualification claim.

Current viewer: **285 passed /80 ignored /zero failures** (`viewer.log`).
