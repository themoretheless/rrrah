# Aseprite pixel buffer admission

The Aseprite direct decoder previously allocated composition output, isolated group canvases and inflated cel buffers before budget admission. The generic raster path managed output only afterward. Added reservations before these large pixel allocations whenever DecodeRequest.memory_budget is present. Output reservation transfers into managed shared pixel ownership; group/cel reservations expire with scratch storage. The fast whole-canvas cel path avoids separate scratch as before. No-budget direct callers retain the existing bounded legacy route.

All63 selected-frame manifest cases verify managed output, exact retained output credit, cloned owner lifetime and zero final usage.36 cases exercise scratch above output.189 zero/output-short/peak-short refusals release all credit; exact observed peak budget succeeds and preserves exact pixels. Layered52 passed/1ignored, full viewer276 passed/74ignored. Actual M4 Max nine aspect cases/36 SDR-float readbacks/9 swap restores pass after the ownership change.

The pixel buffer accounting does not claim full accounting of parser maps, tag/layer strings, zlib internal state or allocator overhead. Cancellation checks already surround parsing/rendering/inflate loops; these runs do not add a timed mid-inflate cancellation proof. Tag playback, original producer aspect fixtures, physical HDR and NVIDIA remain open.

## Corrupted stream and destruction-order follow-up

Retain the unpacked Cow and its reservation as an intact tuple: Rust tuple fields drop in declaration order, so pixel storage is destroyed before budget credit. Previously the newly added destructuring created separate local bindings whose reverse drop order could release credit before storage destruction. Group/output allocations already keep the reservation declared before the pixel allocation.

The new regression mutates every compressed cel header or zlib checksum in48 selected-frame cases.96 decodes fail specifically during zlib processing after nonzero pixel admission, return root used to zero, then retry the original source using the same budget and compare to the independent manifest oracle.96 retries pass with final0. Layered53 passed/1ignored, full viewer276 passed/74ignored. Mid-inflate asynchronous cancellation and allocator/parser metadata remain outside this proof.

## Frame index admission and borrowed chunk scans

Reserve the flat frame-map and delay vector storage before their allocation and keep its credit in Inventory until those fields are destroyed. Reject declared frame counts exceeding the minimum frame-header capacity of the source first. Frame BTreeMap nodes, strings and other parser structures remain outside this reservation.

Removed the transient Vec of chunk descriptors. Two borrowed scans first validate framing and detect newer palette precedence, then decode in source order. A fixture with100000 additional admitted non-rendering metadata chunks succeeds at the exact same144-byte tracked peak as its original when decoding an already borrowed source; pixels match and final usage is0. This does not include the externally supplied source allocation or all allocator metadata. Existing63 independent selected-frame oracle cases and96 corruption/retry cases remain green; zero budget refuses before allocating frame tables. Layered54 passed/1ignored, viewer276 passed/74ignored. No throughput improvement claim or new physical GPU qualification.

## Public file source budget follow-up

The public decode_aseprite file path already uses read_bounded managed source ownership when a request budget is supplied. Added63 file-entry checks: total required credit equals source length plus the measured borrowed decode peak; exact credit passes, source-short and combined-peak-short limits fail with no retained credit (126 refusals). Every successful output matches the independent fixture oracle. On return source/parser table credit is released and only output pixels remain; clones retain that output credit until the final owner drops. Layered57 passed/2ignored. No production changes in this follow-up; unrelated viewer/Metal suites were not repeated.

These checks establish tracked source + flat-index + pixel/scratch accounting on the supplied fixtures. Untracked map nodes, strings, zlib state and allocator overhead remain explicit limitations.

## Bounded low-output inflate cancellation

Inspection confirmed the existing decompressor already limits both input and output slices to65536 bytes per call. Factored its unchanged loop into a progress-observable internal helper; the production wrapper supplies a no-op callback. The controlled test uses an independently authored zlib stream with100000 empty stored deflate blocks then one four-byte pixel, so input progresses long before any output appears. Every observed call consumes at most64KiB, the full500015-byte stream decodes exactly in8 calls, and cancellation set after the first call is observed before a second call or any pixel output. Six truncated prefixes fail, then retry succeeds. Layered58 passed/2ignored including existing pixel, budget, corruption and graph regressions.

This is deterministic cancellation at a decompressor checkpoint, not a native UI latency or OS scheduling measurement. The test does not directly combine cancellation with public file-path scratch lifetime. No production chunk limit change was needed.

## ICC profile reservation transfer

Aseprite ICC chunks now reserve profile bytes before copying; Inventory retains that credit alongside its color declaration, then transfers it into shared raster metadata. Flat-index credit remains temporary. The63-case peak/retained/source-budget tests now count pixels plus ICC capacity when present, instead of expecting pixels alone.

Three native selected frames preserve the pinned588-byte profile and60-byte RGBA storage, share648bytes across raster clones, avoid double charging in try_manage_pixels, and return used0 after final owner. An isolated color-chunk check passes at exact588byte budget and refuses587bytes with peak0 before copying. Layered63 passed/2ignored, full viewer276 passed/74ignored. Existing independent pixel fixtures and corruption/budget cases remain green. Source/profile producer fidelity is not inferred from credit tests; parser nodes/strings and zlib internals remain outside full accounting.
