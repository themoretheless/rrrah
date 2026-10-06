# minhash-dedup

MinHash/LSH near-duplicate removal for large text corpora that **keeps as much
distinct content as the duplicate evidence allows**.

Most MinHash pipelines collapse every connected component of the LSH match graph
to one document. When documents drift gradually (templated pages, successive
edits, boilerplate-heavy sites), those components chain unrelated documents
together, and the pipeline deletes text that has no near-duplicate left in the
output. minhash-dedup takes a different approach. It keeps **complete duplicate
buckets** instead of pairwise edges, and it chooses representatives so that
**at most one document survives per bucket**. That is the actual duplicate
constraint, and it is weaker than "one per connected component".

It runs as five stages on Slurm (or locally), with elastic worker counts,
source-priority-aware retention, verified stage recovery, and optimality
diagnostics.

## Why not one-per-component?

If A matches B and B matches C, union-find removes two of the three documents,
even if A and C share almost no text. minhash-dedup can keep both A and C
because no bucket contains both.

[`examples/chain_demo.py`](examples/chain_demo.py) measures this. It builds a
synthetic corpus of 40 *drift chains*: 8 documents each, where each document
edits 10 of 300 words in the previous one, so the two ends of a chain have
Jaccard similarity of about 0.28. The corpus also has 40 exact-duplicate groups
and 200 unique documents. Both policies run on the **same LSH buckets** from
stages 1–2.5:

```
$ python examples/chain_demo.py
corpus: 664 docs = 40 drift chains x 8, 40 exact-duplicate groups, 200 singletons
LSH: 14 bands x 9 hashes, 20-char shingles, threshold ~0.746; 965 duplicate buckets over 446 docs
mean Jaccard between the two ends of a chain: 0.28

policy                             kept   removed   over-removed*   worst removed J   bucket violations
union-find (one per component)      324       340             127              0.29                   0
minhash-dedup (bucket-feasible)     403       261              15              0.62                   0

upper bound on bucket-feasible retention among bucketed docs: 212.7 (minhash-dedup keeps 185)
* removed documents whose most similar retained document has true Jaccard < 0.746
```

Bucket-feasible selection keeps 24% more documents and removes 8× fewer
documents whose content is no longer represented in the output. Neither policy
ever keeps two members of the same bucket. The closed-form bound shows that the
greedy selection is within 13% of the best any bucket-feasible policy could do
on this input. Seeds 1 and 2 give the same picture (over-removal 129 → 9 and
153 → 5). The remaining over-removals are LSH false positives: pairs that share
a band but fall below the threshold.

This is a synthetic benchmark designed to isolate the policy difference. How
much it matters on a real corpus depends on how much near-duplicate chaining
the corpus contains.

## Compared with DataTrove

This project started from the MinHash stages in Hugging Face's
[DataTrove](https://github.com/huggingface/datatrove) and keeps its Apache-2.0
license and attribution ([NOTICE](NOTICE)). The two now differ in the parts
that decide what gets deleted:

| | DataTrove MinHash | minhash-dedup |
| --- | --- | --- |
| Match records | Pairs of document IDs | Complete buckets (every member of each band match) |
| Cross-band redundancy | Pairs emitted per band, no pruning | Stage 2.5 drops buckets contained in another band's bucket |
| Removal policy | Union-find: one survivor per connected component | Bucket-feasible greedy: at most one survivor per bucket |
| Which copy survives | Component root | Structural score first, then source priority you configure |
| Optimality diagnostics | None | Closed-form, covering, and iterative puncture upper bounds |
| Residual-duplicate audit | None | Exhaustive pairwise signature comparison of retained docs (Numba) |
| Shingles | Word n-grams via a language tokenizer | Character n-grams; no tokenizer, language-agnostic |
| Sharding | Executor tasks/workers per step | One persisted reader plan shared by stages 1 and 4, validated before filtering |
| Scope | Full data-processing framework | MinHash deduplication only; small dependency set |

DataTrove is the better choice when you want a general pipeline framework,
index-based matching against a previously deduplicated corpus, or word-level
shingling. See [docs/comparison-with-datatrove.md](docs/comparison-with-datatrove.md)
for a stage-by-stage comparison and the on-disk format differences.

## Quick start

```bash
git clone <this repo> minhash_dedup && cd minhash_dedup
python -m venv .venv && source .venv/bin/activate
pip install -e '.[analysis]'

python examples/chain_demo.py            # policy comparison above, ~3 s

export S3_INPUT='["/data/curated", "/data/web"]'   # JSONL(.gz) dirs, highest priority first
export DATA_LABEL='["curated", "web"]'
export OUTPUT_PATH=/tmp/dedup_run
scripts/run_local.sh                     # all five stages on one machine
```

Input records are JSONL with a `text` field and optional `meta.id`, `meta.*`,
and `subset`. Retained and removed documents are written in the same layout to
`$OUTPUT_PATH/deduped/` and `$OUTPUT_PATH/removed/`.

## Pipeline

| Stage | Work | Main artifacts under `OUTPUT_PATH` |
| --- | --- | --- |
| 1 | Generate signatures and sort each band independently. | `signatures/`, `reader_plan.json` and completion markers inside it |
| 2 | Merge matching band signatures into complete duplicate groups. | `buckets/*.dups` |
| 2.5 | Prune cross-band subset and equal buckets. | `buckets_pruned/*.dups` |
| 3 | Compute bounds and select representatives. | `remove_ids/*.remove`, `remove_ids/*.clusters`, optional covering-bucket exports |
| 4 | Re-read the same logical shards, filter, and write both streams. | `deduped/`, `removed/` |

Stage 2 partitions work by band and hash range. Stage 2.5 partitions by host
band and target files. Stage 3 runs in one process. Stage 4 applies Stage 3's
removal decisions; the bounds are diagnostics only.

Stage 2 buffers a multiway merge and reuses reader objects, so it never
materializes all pairwise edges.

## Representative selection

For duplicate buckets $B$, the retention objective is

$$
\max |R| \quad \text{subject to } |R \cap B| \le 1 \quad \text{for every } B.
$$

The implementation is a greedy heuristic, not an exact maximum-cardinality
solver. A document's weight is the number of buckets that contain it (not a
quality score).

1. Buckets containing a weight-1 document are resolved first.
2. Documents with identical residual bucket support (incidence twins) are
   represented once and expanded again for coverage and cluster metadata.
3. Weight layers are processed from low to high. Within each layer, roots are
   chosen by a local equal-weight collision score.
4. Removed documents are assigned to a representative through a bucket they
   share with it.

Feasibility is checked against the discovered buckets, not by an exact
text-similarity test over every pair.

### Source-priority-aware retention

List input sources from highest to lowest priority in `S3_INPUT` (and in the
matching order in `DATA_LABEL`). The reader plan assigns consecutive logical
ranks in that order. Document keys are packed as `(read_id << 32) | doc_id`, so
among otherwise equal candidates the higher-priority source wins. Candidates
are ordered by

$$
(c_w(v),\;\pi(v),\;\mathrm{id}(v)),
$$

where $c_w(v)$ is the equal-weight collision score and $\pi(v)$ is source rank
(smaller is preferred). Within an incidence-twin class, the higher-priority
source is the representative.

Priority is a tie-break only. It never adds or removes a bucket constraint, and
it does not override a better structural score. This package does not compute
quality scores; you supply the order. The saved reader plan rejects changes to
source order, so reordering sources requires a new run.

## Bound diagnostics

For document degree $d(v)$, define bucket weight $w(B)=\min_{v\in B}d(v)$. Any
bucket-feasible retention satisfies $|R| \le \sum_B 1/w(B)$ over the bucketed
documents. Stage 3 tightens this in two ways and logs each step:

- **Puncture bound.** Starting after weight-1 preprocessing and
  support-dominance reduction, remove buckets whose calculated gain is
  nonnegative, rebuild the metrics, and repeat until a round removes nothing.
  The log reports the baseline bound, the bound after each reduction, the final
  bound, and the improvement.
- **Covering bound.** Greedily select buckets that cover all represented
  documents, then recompute the reciprocal-weight bound on that cover. Residual
  covers are exported as `cover_buckets.dups` for further analysis.

Comparing these bounds with the number of documents actually kept tells you how
much room the greedy selection left on your corpus.

## Elastic computing

Choose the logical world size (`TASK_SIZE`) once, to size per-shard memory. Then
choose worker counts per stage independently: launched worker `r` of `P`
processes logical ranks `r, r+P, r+2P, …`. For example, with `TASK_SIZE=2048` on
nodes that fit 32 tasks each:

| Nodes | Concurrent tasks | Rounds | Logical rank ranges |
| --- | --- | --- | --- |
| 32 | 1024 | 2 | 0-1023; 1024-2047 |
| 16 | 512 | 4 | 0-511; 512-1023; 1024-1535; 1536-2047 |

Stages 1 and 4 share `signatures/reader_plan.json`, which records the world
size, ordered inputs, file lists and sizes, and the MinHash configuration.
Changing only worker counts leaves signatures, buckets, and selections
byte-identical. The tests check this. See
[docs/elastic-computing.md](docs/elastic-computing.md).

## Recovery

The driver skips a stage only after verifying that it completed. Finding files
in an output directory is not enough. Stage 1 checks a plan-bound marker for
every active logical rank. Stages 2–4 publish atomic records under
`OUTPUT_PATH/.stage_checkpoints/` only after every worker exits successfully.
These records bind the reader plan, the relevant settings, the upstream
generation, and an inventory of output files.

When a stage needs a retry, the driver resubmits it and every downstream stage.
Each retried stage invalidates later records and clears only its own generated
output directories. Recovery is at stage granularity (there is no per-worker
resume), and inventories record names, sizes, and times, not content checksums.
Do not run two drivers against one `OUTPUT_PATH`.

## Outputs and diagnostics

Stage 4 writes gzip JSONL for both retained and removed documents, preserving
IDs, source labels, subset tags, and metadata. Clustered documents receive
`minhash_cluster_id` and `dup_signals.dup_doc_count` (cluster size minus one).
Stage 3 logs the distributions of cluster sizes, bucket sizes, and document
weights.

**Pairwise residual audit.** With `MINHASH_ANALYZE_CLUSTERING=true`, stage 3
reconstructs full signatures for the retained documents by `(read_id, doc_id)`.
It then compares **every unordered pair**, including pairs that never shared an
LSH bucket, and reports a histogram of MinHash-estimated Jaccard distances. This
lets you check for near-duplicates that LSH missed. Comparisons are blocked and
parallelized with Numba. Above `max_docs_exact=10_000_000` documents, a seeded
sample is used instead. The audit does not change removal decisions.

## Configuration

`scripts/dedup_params.py` defines the MinHash configuration and paths. It reads
these environment variables:

| Variable | Meaning |
| --- | --- |
| `S3_INPUT` | JSON list of input directories, highest priority first (local, `s3://`, `hf://` via fsspec) |
| `DATA_LABEL` | JSON list of labels, one per input directory |
| `OUTPUT_PATH` | Output root (must be a local or shared filesystem) |
| `TASK_SIZE` | Logical world size shared by stages 1 and 4 |
| `MINHASH_NUM_BUCKETS`, `MINHASH_HASHES_PER_BUCKET`, `MINHASH_SEED` | LSH bands, hashes per band, and seed (default 14, 9, 1) |
| `MINHASH_ANALYZE_CLUSTERING` | Enable the pairwise residual audit in stage 3 |

Character n-gram length (default 20) and hash precision (32 or 64 bit) are set
in `MinhashConfig` / `HashConfig`. Shingling removes ASCII punctuation,
normalizes whitespace, and preserves case. Install `.[io]` for Zstandard,
`.[s3]` for S3, or `.[hf]` for the Hugging Face Hub.

### Slurm

```bash
export S3_INPUT=... DATA_LABEL=... OUTPUT_PATH=... TASK_SIZE=2048
export SBATCH_PARTITION=my-partition        # site settings via standard sbatch variables
export MINHASH_CONDA_ENV=minhash-dedup       # or empty to skip conda activation
export MINHASH_DEDUP_DIR=$HOME/minhash_dedup # checkout location on compute nodes
scripts/minhash_driver.sh
```

`scripts/minhash_s*.sh` contain per-stage resource defaults. Adjust nodes and
tasks for your cluster. Stage 2's task count must be a multiple of the band
count. The driver launches stage 2.5 with one node per band.

## Library API

```python
from minhash_dedup import (
    MinhashConfig, HashConfig,
    SignatureStage, BucketStage, PruneStage, SelectionStage, FilterStage,
)
from minhash_dedup.runtime import Document
from minhash_dedup.jsonl import JsonlReader, JsonlWriter
from minhash_dedup.partition import ReaderPlan
```

`examples/chain_demo.py` shows the stages driven in-process.

## Layout

- `src/minhash_dedup/minhash.py`: the five stages and signature analysis.
- `src/minhash_dedup/partition.py`: reader allocation, persisted plans, elastic scheduling.
- `src/minhash_dedup/checkpoints.py`: stage completion records and output inventories.
- `src/minhash_dedup/{runtime,signatures,io,jsonl}.py`: documents, hashing and binary helpers, filesystem access, and JSONL I/O.
- `scripts/`: stage entrypoints, Slurm wrappers, driver, and `run_local.sh`.
- `examples/chain_demo.py`: policy comparison benchmark.

## Limitations

- Stage 3 runs in a single process and holds bucket membership in memory.
  Stage 1 sorts each rank's band file in memory.
- Selection is greedy. The bounds tell you how far from optimal it might be, but
  they do not certify optimality.
- Signature and bucket artifacts must be on a local or shared POSIX filesystem.
  Only inputs and final outputs can be remote.
- There is no matching against an external index of an already-deduplicated
  corpus.
- On-disk formats (`.dups`, `.clusters`, signatures) are not compatible with
  DataTrove's.

## Tests

```bash
pip install -e '.[analysis,testing]'
NUMBA_NUM_THREADS=2 python -m unittest discover -s tests -v
```

The tests cover:

- saved binary-output digests for every stage (32- and 64-bit hashes)
- all five stage entrypoints run on temporary data
- byte-identical results across worker counts
- source-priority tie-breaks and bucket feasibility
- recovery and invalidation (with stubbed Slurm)
- the pairwise signature audit
- the chain demo's headline claim

## License

Apache-2.0. See [LICENSE](LICENSE). This project began from MinHash code in
DataTrove (Hugging Face, Apache-2.0). Retained and adapted code, and its
provenance, are recorded in [NOTICE](NOTICE).
