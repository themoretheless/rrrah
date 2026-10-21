# Full-pixel replacement: rejected target-file optimization

The candidate avoids per-region subtraction when a clipped incoming triangle is exactly the unit square and its source priority wins everywhere. Eligibility checks four exact corners and scans all old owners: a later patch wins, or identical same-patch u/v fields use the existing incoming-wins tie policy. Sample ID is irrelevant. Incoming allocation, scan/final debits and publication remain transactional. No quota, tolerance or depth/fragment limit changes were made.

## Controls

A 32-region analytic full-pixel case drops 271→78 work units and retains one blue unit-area fragment. Seven controls cover later source, same-field tie with a lower incoming sample ID, higher old source, half coverage, crossing same-patch UV priority, a four-vertex partial polygon with the full unit bounding box, and a higher old source encountered last. A full bounding box alone cannot qualify. All reached 1 admission/78 cancellation/78 shorter-budget pilot failures preserve old integrals, fragments and retained memory, retry and release credit. Candidate vector 229/11 ignored, interpreter 160/3 and native 319/13 passed. These are correctness and local work controls, not measured viewing speed.

## Target-file relevance

A temporary counter-only build records exact eligibility at the same pinned VectorApple.ai source. Sources were restored before the frozen instrumented binary ran; patches, hash, full terminal trace and parsed counts are retained. Completed preflight counters include queries, successes, total old regions replaced, failed queries/scans and maximum successful old group. Instrumentation does not debit work or alter geometry; this is not CPU timing.

| Patches | Queries | Successes | Old regions replaced | Failed queries |
|---|---:|---:|---:|---:|
| 1455 | 0 | 0 | 0 | 0 |
| 323 | 0 | 0 | 0 | 0 |
| 245 | 0 | 0 | 0 | 0 |
| 788 | 42 | 42 | 0 | 0 |

Thus the branch never replaces an existing target-file region. Its only real-file uses create empty pixels, and all four attempts still exhaust 100M. The measured benefit exists only in the analytic overdraw control; it does not address this target's dense partial fragments. The default optimization was therefore removed rather than accumulated as an unqualified target-file fix. The rejected patch and seven analytic/failure controls are retained for future evidence. one.ai remains unchanged in the candidate run.

## Restored state and next action

The normal decoder is byte-identical to the preceding accepted strict-side-classification decoder (restored-state.json). Profiling and full-pixel replacement code are absent. New analytic/rollback tests remain enabled against the normal policy, with its actual reached counts recorded in restored-vector.log. Full renderer source and existing interpreter/native gate scope are preserved; no full-AI, all-format, cache-pressure or hardware completion is claimed.

Next optimization must address dense partial-region work (including retained-fragment copying and traversal); unit-wide overdraw is ruled out as a meaningful cause of this file's current quota failures.

Restored-code verification: 229 vector tests passed/11 ignored; all reached 2 admissions, 271 cancellations and 271 shorter work quotas preserve and retry the full-pixel analytic case under the unchanged normal policy. No failures.
