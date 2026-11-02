# Comparison with DataTrove's MinHash

minhash-dedup began from the MinHash stages of
[DataTrove](https://github.com/huggingface/datatrove) (Hugging Face, Apache-2.0).
This page describes how the two differ today, so you can pick the right tool
and compare results correctly.

Baseline: `huggingface/datatrove` `main` at commit
`1ca2583034b9e5cf49ec725baa60a6c1ef776867` (2026-09-17). About 12% of the
non-trivial lines in [`src/minhash_dedup/minhash.py`](../src/minhash_dedup/minhash.py)
also appear in upstream's `minhash.py`. Most of that overlap is signature
generation, the configuration classes, and hashing helpers. Provenance is
recorded in [NOTICE](../NOTICE).

## Stage by stage

| Stage | DataTrove | minhash-dedup |
| --- | --- | --- |
| 1: signatures | Word n-grams from a language tokenizer after text normalization. Writes band hashes and document ID. | Character n-grams after ASCII punctuation removal and whitespace normalization; case preserved. Adds a global-extremum secondary hash to each record. |
| 2: matching | Writes duplicate *pairs* (four uint32 fields); optional matching against an index. | Writes complete duplicate *groups*: a member count followed by (reader, document) pairs. Matches only within the input dataset. |
| 2.5: pruning | No counterpart. | Drops buckets strictly contained in another band's bucket; among equal buckets, keeps one by a deterministic tie-break. |
| 3: selection | Union-find over pairs; one survivor per connected component. | Bucket-feasible greedy selection over complete buckets, with weight-1 preprocessing, incidence-twin compression, and source-priority tie-breaks. Also computes covering and iterative puncture upper bounds. |
| 4: filtering | Reads removal IDs and optional separate cluster-ID and size files. | Reads removal IDs and combined (document, cluster, size) records; writes both retained and removed streams with `dup_signals.dup_doc_count`. |
| Diagnostics | None in the MinHash module. | `analyze_cluster_signature`: exhaustive (or sampled) pairwise signature comparison of retained documents, parallelized with Numba. |
| Execution | General pipeline executors (local, Slurm, Ray). | Dedicated stage scripts and Slurm wrappers; one reader plan shared by stages 1 and 4; verified whole-stage recovery. |

## The policy difference

The two stage-3 policies optimize different objectives.

- **Union-find** treats the match graph's connected components as clusters.
  A path A–B–C puts A and C in one cluster even if they are not similar.
- **Bucket-feasible selection** keeps at most one document per bucket, which
  is the constraint the LSH evidence actually supports. It maximizes how many
  documents are retained under that constraint.

Because the objectives differ, retention counts from the two tools are not
directly comparable. [`examples/chain_demo.py`](../examples/chain_demo.py)
runs both policies on the same buckets and reports each one's retention, its
over-removal (removed documents with no sufficiently similar survivor), and
bucket violations.

The Stage 1 shingle definition also differs (character n-grams instead of word
n-grams). To compare end-to-end results, control the shingle definition,
normalization, hash configuration, and seed.

## On-disk format compatibility

The artifacts are **not interchangeable**, even where file extensions match:

1. **Signatures** have an extra secondary-hash field.
2. **`.dups`** files hold length-prefixed groups here and fixed-size pairs
   upstream.
3. **`.clusters`** files hold (document, cluster, size) triples here and
   (document, cluster) pairs upstream.

## What DataTrove has that this project does not

- A general pipeline framework with many readers, filters, and writers, plus
  local, Slurm, and Ray executors.
- Word-level shingling using language-aware tokenizers.
- Matching against an index of a previously deduplicated corpus (stage 2) and
  creating such an index.
- Skipping signature files that already exist on resume.
- Checks of signature byte counts, and BLAKE2 verification of sorted writes.

This project deliberately stays small. It depends on numpy, fsspec, orjson,
xxhash, nltk, loguru, and tqdm, with Numba as an optional extra.
