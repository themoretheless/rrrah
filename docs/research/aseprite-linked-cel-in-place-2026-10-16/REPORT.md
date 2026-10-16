# In-place Aseprite linked-cel resolution

Replaced per-cel path Vec/visited HashSet and per-frame copied key Vec with in-place link reversal and ordered BTreeMap range iteration. A temporary Resolving back edge detects cycles and records the predecessor; reaching an image unwinds the chain, replacing only CelData. Position, opacity and z remain untouched. Forward references and path compression remain supported. Cancellation/error discards the partial Inventory; no unresolved state is published. The renderer consumes the resolved graph as before.

Qualification: forward and backward50000-frame chains, two sparse layer keys per frame,200000 checked cel states; cycle lengths1,2 and a tail entering a cycle; missing target and pre-cancelled request. Resolution itself requests no scratch credit; the test allocates the input graph outside the tracked budget, so zero scratch is not a claim of zero total graph memory. Existing63 independent selected-frame pixel oracles and96 corrupt-stream retries pass. Layered55 passed/1ignored, full viewer276 passed/74ignored.

No runtime speedup claim is made without paired measurements. Graph BTreeMap nodes remain outside complete parser memory accounting. Tag playback, physical HDR and NVIDIA qualification remain open.

## Exhaustive oracle and paired timing follow-up

An independent visited-array walker over35414 graphs of1..5 frames, two distinct image payload choices, links and a missing target. Two sparse layers use different pixels; all success/refusal outcomes, resolved payloads and per-cel transforms match. No production changes in this follow-up.

Paired microbenchmark reproduces the former path/key/hash implementation test-only. Eight pairs each of forward/backward50000-frame chains alternate old-first/new-first; graph creation and correctness comparison are outside timed regions. New code is faster in all16 pairs. Backward median old/new ratio=3.03; forward=2.13. Raw nanoseconds retained. This is resolver-only timing on the current Mac/test profile, not whole-file decode/viewer latency or a general hardware guarantee. Full layered suite56 passed/2ignored; the benchmark was separately enabled and passed.
