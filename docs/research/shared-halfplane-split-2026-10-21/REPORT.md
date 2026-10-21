# Complementary half-plane split in one vertex walk

Subtraction previously clipped the same subject twice per cut: first to the outside and then to the inside. The new private split_by_id walks vertices once, evaluates the original support once per visited vertex and reuses one canonical intersection for both sides. Each side independently retains its edge tags and the previous topology-preserving hull finalization. Inclusive zero-distance vertices are retained on both boundaries. Signed-zero and endpoint intersections use the existing canonical arithmetic. No heap allocation, vertex/fragment/depth quota, memory cap, tolerance or area clamp is added.

The ordinary clip_by_id remains for general/source-priority clipping and serves as a direct equivalence control. Shared finish_clip only extracts the existing duplicate-closing-point and hull handling; it does not change that policy.

## Correctness and work evidence

4,120 cases compare both outputs against separate negative/positive legacy clips, bit for bit in all retained point coordinates and boundary IDs. Cases use all 515 independent intersection-oracle inputs with tagged/untagged triangles and axis/oblique fields. The earlier Fraction intersection and union controls remain enabled; equivalence alone is not an independent oracle. A square split gives area 0.5 on each side and retains both cut-boundary vertices on both outputs. It reduces work from 10 to 5 ticks and support evaluations from 10 to 5. All shorter new quotas fail explicitly.

On the same captured 138-record replay, every prefix has identical summed area and fragment count. Final area 1/93 fragments and max floating pair overlap 7.806255641895632e-18 are unchanged. Local work falls 49,606→37,108 (25.19%). This is reduced metered kernel work, not measured CPU time or page/display throughput. New-case rollback/retry passes all reached 5 admission failures, 128 cancellation checkpoints and 128 short quotas while preserving the old integral, fragment count and retained memory and releasing all credits.

## Real-file scope

The successful build completed before qualification began; the runner executes and hashes its private frozen executable. Pinned VectorApple.ai still returns incomplete at 100M for all four sampled attempts; the captured terminal stderr shows no coverage-over-one refusal. one.ai remains unchanged: 2522 different pixels/max58 on page 0; page 1 exact. Thus this does not qualify full AI support or a page-level speed gain. Full-format, long cache/swap pressure, physical HDR and NVIDIA work remain open. Temporary profiling/capture code is absent from the production source.

Final suites: vector 227 passed/11 ignored; interpreter 160 passed/1 ignored; native 319 passed/13 ignored; 15 focused kernel controls passed. No failures.
