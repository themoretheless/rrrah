# Transactional range-index prototype: not promoted

The previous per-record and incoming-only bounds caches did not qualify a net speedup. This trial tests a different operation: skip a whole represented-fragment span rather than looking at every old polygon. The complete prototype is retained in prototype.patch; it is removed from the normal source after qualification below.

## Mechanism and invariants

Each summary covers16consecutive records with conservative aggregate bounds and an eligibility proof that every represented polygon has positive area and cannot merge with its previous published neighbor. The fast path additionally requires that the new output tail equals the corresponding old tail, room for the whole span, and either no incoming remainder or disjoint bounds from the clipped original incoming triangle. This last polygon contains every incoming remainder. Otherwise the existing per-record geometry path remains.

A separately admitted journal records unchanged copied ranges. Pop/merge or prefix rollback marks the earliest damaged position; overlapping and later journal entries are invalidated before reuse. New summaries may union at most two old summaries, conservatively widening the bound; no geometry, tolerance or source fields change. Changed or unproved blocks are recomputed. Record vectors, journal and summaries use the existing admission mechanism; old vectors/index and line IDs remain published until the whole replacement and final cancellation/work checkpoint succeed. A failure discards temporary credits and keeps the old index.

The first32-region threshold increased work on the captured dense50/75 controls (82522→85052 and37590→38770). The measured trial therefore builds no index below128records, preserving both dense controls' previous operation counts. This is an experimental amortization choice, not a universal optimum. The additional metadata and journal are fully included in admission. Fragment layout itself does not grow.

## Geometry, admission and work evidence

Kernel22passed/2manual timing controls ignored. Each captured50/75-input prefix preserves owners, coordinate and affine-component bits, edge tags, represented area flags and merge flags against the disabled-index control. Published summary bounds must contain every actual vertex, and eligible summaries must satisfy the actual flags. Generated failure checks additionally compare published summary bits after rejection. The captured small-dense controls stay below the128activation threshold and mainly qualify the guard; they do not independently qualify indexed dense affine composition. The active256sparse cases exercise disjoint carries. Admission/cancellation during overlapping dense journal damage/merges would still need a targeted above-threshold control before any promotion.

The authored256triangle sparse control has independent exact area1/512 and identical represented geometry/owner/tag data. Its operations decrease46322→33963 (26.68%). This is an algorithmic work-counter result, not CPU time or viewer FPS. The public256fragment plus disjoint blue-triangle case checks red1/512, blue1/8192, total area,257retained fragments and final managed0. All reached16admission failures,158cancellations and158short work quotas preserve old integrals, counts and retained bytes; each then retries successfully. The original64fragment bulk control remains7/113/113. Focused public24/0ignored, whole candidate vector256/11ignored, zero failures.

## CPU and actual AI evidence

A frozen optimized probe compares index disabled/enabled in the same executable. Six AA controls and16alternating AB/BA pairs cover pair/dense50/dense75/sparse64 and sparse256. Mutation tracking and stack layout exist in both paths; disabled mode does not construct or retain summaries/journal. Zero-sized admission credits isolate kernel CPU work from real MemoryBudget bookkeeping. The analyzer now recognizes sparse256, preserving previous log formats.

No general speedup is established. Pair/dense50 AA is highly unstable. sparse64 and sparse256 candidate intervals cross or overlap control variation; sparse256 mean is -0.18%,95% interval -0.63..0.26. Raw results and all invalid/noisy controls remain in candidate-cpu.log/json. No selection throughput, page-time or GPU claim is made.

A freshly built normal decoder is frozen by the source-pinned Poppler script. VectorApple.ai still has four incomplete attempts (1455,323,245,788patches), each spending the unchanged100M quota. It returns InvalidPdf("shading decode/precision limit"). one.ai page0 remains2522different pixels/max58 and page1 is exact. The normal qualification remains negative; candidate-poppler.json preserves terminal outcomes and bounded stderr.

## Decision and useful retained work

Not promoted. The index reduces work in an authored long sparse case but still copies/materializes all unchanged records, and no net CPU or real-file completion benefit is established. The source is restored to its exact pre-trial bytes. Further work requires shared fragment storage/copy reduction, not a wider claim for this index.

The public large_partition_all_failures_rollback_and_retry regression is retained with a general name, since its independent analytic area, ownership and exhaustive rejection/retry contract applies to both the current implementation and future storage/index changes. Its prototype name was span_index_all_failures_rollback_and_retry; only the name/output label changes after removing the prototype. The analyzer's sparse256 support is also retained. AI/full100-format/GPU/hardware gates remain incomplete. No new Metal/CUDA/NVIDIA/HDR qualification is inferred from this CPU-only trial.

## Restored current implementation

The unchanged core source matches its pre-trial SHA256 exactly; the normal raster_fixture_dump rebuild succeeds and its separate SHA is pinned. With the retained general256fragment regression, the restored vector library has256passed/11ignored, zero failures; the original kernel20passed/2ignored. The new large regression on the restored implementation reaches9admission failures,307cancellations and307short quotas, all transactional/retry exact with final0. These counts differ from the rejected prototype because it skips individual checks/work via the span proof. Current-state raw logs replace no earlier unique candidate evidence and are named explicitly as restored-state checks.
