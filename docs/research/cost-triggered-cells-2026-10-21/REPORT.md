# Rejected cost-triggered cell pilot; retained outcome diagnostics — 2026-10-21

## Decision

The 128k paint-threshold policy is removed from production. Its correctness controls passed, but it did not complete any of the four real sampled AI attempts under the unchanged shared 100M quota. No representative hot-cell work reduction or full-page throughput improvement was established. The patch and all intermediate results are retained for future analysis, not presented as an accepted optimization.

Accepted changes are diagnostic: every post-tessellation sampled work attempt now reports its outcome and spent/remaining quota, and the qualification runner preserves those structured records independently of its bounded log tail. The sampler algorithm is restored to its preceding policy. Its diagnostic closure borrows storage/credits; record storage is released before its credit and before the final cancellation check. Map storage and its reservation remain owned by the original function scope.

## Rejected pilot and evidence

The pilot allowed at most 128*1024 logical painting operations for a non-deepest cell, debited all spent work (including unsuccessful/cancelled attempts) from the original global quota, then discarded temporary partitions/candidates before refining through the existing children. At depth eight, the original shared allowance and 4096 fragment fallback remained available. Geometry was replayed, not dropped; the shared quota was never increased/refunded.

A forced zero-local-budget control covered all reached 25 admission refusals, 396 cancellation checkpoints and 394 shorter global quotas, with resident-byte restoration and successful retry. It checked exact full-square area and constant-color integrals, additional work charged for the coarse attempt, and the deepest-leaf bypass. Target controls: seven passed; full pilot suites: vector 225/11 ignored, interpreter 157/1, native 319/13. These correctness results did not establish performance.

pilot-poppler.json contains decoder/input hashes and full structured outcomes. All sampled attempts (1455, 323, 245 and 788 patches) return incomplete after spending 100M with zero left. Baseline paired profiles in ../disjoint-source-bounds-2026-10-21 show the same unsuccessful outcome classes. one.ai page 0 remains 2522 differing pixels/max58; page 1 exact. AI stays Pending.

## Retained diagnostics

ORDERED_SAMPLE_OUTCOME is emitted once per sampled work-phase exit under the existing trace switch, including returns from the main tiled loop. Outcomes from earlier setup failures are outside this work-phase counter; they retain their existing diagnostics. A failed sampled preparation is not by itself a proof about all alternative rendering paths or image fidelity.

qualify-ai-upstream.py now collects native stdout/stderr on success and failure. It extracts up to 64 outcome records from the complete captured stderr before retaining only its last 16384 characters; a truncation flag is set for additional outcome records. This bounds the report, not the subprocess capture allocation. Timeout byte output is decoded safely. Reference-renderer errors retain a separate tail and do not overwrite native outcomes.

Three parser controls cover events lost from the tail, text/timeout bytes, success records, record caps, empty/malformed and non-UTF8 data. This closes the earlier observation gap where only two explicit dense-cell messages were mistaken for an exhaustive count of unsuccessful attempts. Existing strict pixel comparison and exit criteria are unchanged.

## Remaining work

The current baseline still requires a fix to its global work use on VectorApple.ai. Before another default scheduling policy, measure main-loop scans versus painting/recursive replay and test a representative hotspot against identical source integrals. The 323-patch baseline has a large unclassified category in the earlier thresholded profile; source-order traversal/indexing may need separate work. No positive performance claim follows from this rejected pilot.

Full 100-format viewing, long memory-pressure collections, physical HDR and deferred NVIDIA remain incomplete. Restored-code verification and current normal-binary results are appended when complete.

## Final restored-code verification

Current source: vector 225 passed/11 ignored; interpreter 156/1; native 319/13. Zero failures. Three Python diagnostic parser controls pass. The normal decoder rebuilt successfully and its SHA matches canonical poppler.json. That report now preserves all four unsuccessful work-phase outcomes despite the bounded stderr tail. one.ai results remain unchanged. No local paint threshold or forced cost-refinement helper remains in production.
