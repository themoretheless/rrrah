# Elastic Computing

Stages 1 and 4 separate the **logical reader world size** from the **number of
concurrent workers**. This extends the rank-folding design originally used in
stage 4 to signature generation. A fixed partition can run on more nodes for
fewer rounds, or fewer nodes for more rounds, without changing document identities.

## Logical Ranks Versus Workers

| Quantity | Meaning |
| --- | --- |
| `TASK_SIZE` / `--task-size` | Configured logical `world_size`, chosen before stage 1. |
| Active ranks | Logical ranks with input files, never more than the configured world size. |
| `SLURM_NTASKS` | Concurrent launched workers for this stage. May differ between stages 1 and 4. |
| `--task-id` / `SLURM_PROCID` | A launched worker's rank, not necessarily the logical rank being processed. |

For launch size `P`, launched worker `r` processes logical ranks:

```text
r, r + P, r + 2*P, ... < active_rank_count
```

Each worker runs its logical ranks sequentially. Partial final rounds are allowed;
workers without an assigned rank exit without creating signature or output files.
The rounds describe rank ranges, not synchronized barriers: a worker proceeds to
its next rank as soon as the preceding one finishes.

## Example: 2048 Logical Ranks

For nodes with 128 CPU cores and 2.5 TB memory per node,
reserving 4 CPUs per task allows 32 concurrent tasks per node. Fix logical
`world_size=2048` to keep individual input shards suitably sized for memory.
Assuming all 2048 ranks are active:

| Nodes | Concurrent workers | Rounds | Logical rank ranges |
| --- | --- | --- | --- |
| 32 | 1024 | 2 | 0-1023; 1024-2047 |
| 16 | 512 | 4 | 0-511; 512-1023; 1024-1535; 1536-2047 |

The same logical rank processes the same files and document sequence in either
configuration. Stage 1 may use 32 nodes and stage 4 may use 16, for example.
Their logical world sizes must match; their concurrent worker counts need not.

More logical partitions can reduce stage 1's per-rank signature-file sorting
memory. This is not a memory guarantee: input skew, large individual documents,
and concurrent tasks still matter. Runtime does not necessarily scale linearly
with node count because of I/O contention and load imbalance. `--cpus-per-task`
reserves CPUs; it does not itself make every operation use all those CPUs.

## One Shared Reader Plan

Both stages use `minhash_dedup.partition.ReaderPlan`, not copied allocation code.
Stage 1 publishes `signatures/reader_plan.json` atomically before writing signatures.
The plan records:

- The configured logical `world_size`, even when fewer ranks are active.
- Ordered input directories, sorted file lists, and their byte sizes.
- The logical reader allocation per directory.
- The MinHash configuration and plan format version.

The allocation preserves the existing algorithm: give each nonempty directory
one reader, allocate additional readers proportionally to estimated bytes, cap
by file count, and distribute the remainder in the existing tie order. A directory
containing `.gz` files retains the previous 2x size-estimation factor.

Stage 4 **loads this plan**. It neither counts `.sig` files to infer world size
nor recomputes a new allocation from the current directory sizes. Both stages
use the same logical-rank lookup and read the saved file sequence explicitly.

For example, a requested world size of 2048 with only 1800 input files has 1800
active ranks, but the saved world size remains 2048. Stage 4 adopts 2048, not 1800.
Passing `--task-size 1800` in this case is an error.

Changes to input directory order, an explicitly supplied world size, or the
MinHash configuration are rejected. Planned files that are missing or have
changed size are rejected before their shard is processed. Newly added files
are not part of this run. Keep input contents immutable: byte-size checks do not
detect edits that preserve file size.

## Completion Is Separate

Each successfully processed logical rank writes a plan-bound marker under
`signatures/stage1_complete/`. Reprocessing a rank first removes its marker, then
rewrites it only after signature generation and sorting finish successfully.

An active rank without a matching marker is incomplete, not inactive. Stage 4
checks completion of its assigned ranks before writing output. The stage 1 Slurm
wrapper also checks every active rank after `srun` succeeds, and the driver uses
that check rather than treating any file in `signatures/` as proof of completion.

The downstream Slurm wrappers publish whole-stage completion records only after
all workers succeed. The driver validates their configuration, upstream generation,
and output inventory; merely finding a file is never enough. A failed stage and
its dependents are retried as whole stages, with the retried stage's generated
output directories cleared to avoid stale shards after worker-count changes.
See [Recovery](../README.md#recovery) for scope and migration details.

These checks do not implement per-worker checkpoint resume or live resizing of a
running Slurm job. Do not run concurrent attempts or multiple drivers against the
same output directory. Finish stages 1, 2, 2.5, and 3 before starting stage 4.

## Running On Slurm

From `~/minhash_dedup/scripts`, keep the same input/output configuration for the
whole pipeline. Reserve logical partitions independently of the node allocation:

```bash
export TASK_SIZE=2048
sbatch --nodes=32 --ntasks-per-node=32 --cpus-per-task=4 minhash_s1.sh
```

After stages 2, 2.5, and 3 have completed, stage 4 can use fewer nodes:

```bash
sbatch --nodes=16 --ntasks-per-node=32 --cpus-per-task=4 minhash_s4.sh
```

Both wrappers preserve an exported `TASK_SIZE`. Stage 4 can also omit it and use
the saved plan; when supplied, it is a consistency assertion. The driver forwards
`TASK_SIZE` through its existing `--export=ALL`; adjust stage-specific Slurm
resource directives when using the driver to submit the whole pipeline.

The stage 1 wrapper prepares the plan once before `srun`. Direct Python launches
can prepare it explicitly too:

```bash
python minhash_stage1.py --prepare-only --task-size 2048
SLURM_NTASKS=1024 python minhash_stage1.py --task-id 0
python minhash_stage1.py --check-complete
```

The middle command runs only launched worker 0, covering ranks 0 and 1024 when all
ranks are active; launch all workers to complete the stage. On first creation,
stage 1 defaults to `SLURM_NTASKS` if neither `TASK_SIZE` nor `--task-size` is set.
On later invocations, an omitted task size adopts the saved plan. Explicit CLI
`--task-size` takes precedence over `TASK_SIZE` in both stages.

Use a new output directory when changing the logical partition or input inventory.
Changing only the launch size leaves stage 1 signature names/bytes and stage 3
identities unchanged. Stage 4 retains its existing fold-based output naming, so
output filenames can differ even though document contents and selection match.
Do not mix outputs from separate stage 4 runs in the same destination.

## Older Artifacts

Signatures produced before reader plans were introduced do not contain enough
information to reconstruct their original world size reliably. Stage 4 now fails
explicitly when the plan is missing, even if a task size is supplied. Stage 1 also
refuses to attach an inferred plan to an existing signature directory.

Use a preserved pre-plan checkout for such historical runs, or regenerate
signatures in a new output directory. Migrating an old run requires its verified original
partition and input inventory; there is deliberately no automatic file-count
fallback.

## Verification

Tests compare complete pipelines under different launch sizes with a fixed logical
world size. They check signature, bucket, and cluster bytes, plus kept/removed
document identities and metadata. Cases include multiple input directories,
compressed files, empty directories, fewer active ranks than the configured world
size, partial final rounds, missing completion, and mismatched world sizes.

The shared allocation was also checked against the original stage 1 code on
10,000 generated valid inputs, with identical assignments.
