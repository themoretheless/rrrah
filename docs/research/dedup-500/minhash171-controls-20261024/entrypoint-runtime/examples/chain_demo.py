"""Compare bucket-feasible selection with connected-component (union-find) dedup.

Builds a synthetic corpus containing *drift chains*: each document is a light
edit of the previous one, so neighbours are near-duplicates while the two ends
of a chain share little text. Both policies see the same LSH buckets produced by
stages 1-2.5; only the retention rule differs.

- union-find: keep one document per connected component of the bucket graph
  (the policy used by common MinHash pipelines such as DataTrove's).
- this project: stage 3 bucket-feasible selection (at most one retained
  document per duplicate bucket).

For every removed document we measure the true character-shingle Jaccard
similarity to its most similar retained document. A removed document with no
retained document above the LSH threshold is *over-removed*: its content is no
longer represented in the output.

Usage:
    python examples/chain_demo.py [--chains 40] [--chain-length 8] [--seed 0]
"""

import argparse
import random
import struct
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from minhash_dedup import BucketStage, MinhashConfig, PruneStage, SelectionStage, SignatureStage  # noqa: E402
from minhash_dedup.runtime import Document, logger  # noqa: E402


def make_corpus(chains, chain_length, exact_groups, singletons, words_per_doc, edits_per_step, rng):
    vocab = ["".join(rng.choices("abcdefghijklmnopqrstuvwxyz", k=rng.randint(3, 9))) for _ in range(20_000)]
    texts, groups = [], []
    for chain in range(chains):
        words = rng.choices(vocab, k=words_per_doc)
        for _ in range(chain_length):
            texts.append(" ".join(words))
            groups.append(f"chain-{chain}")
            words = list(words)
            for position in rng.sample(range(words_per_doc), edits_per_step):
                words[position] = rng.choice(vocab)
    for group in range(exact_groups):
        text = " ".join(rng.choices(vocab, k=words_per_doc))
        for _ in range(rng.randint(2, 5)):
            texts.append(text)
            groups.append(f"exact-{group}")
    for _ in range(singletons):
        texts.append(" ".join(rng.choices(vocab, k=words_per_doc)))
        groups.append("singleton")
    order = list(range(len(texts)))
    rng.shuffle(order)
    position = {old: new for new, old in enumerate(order)}
    chain_ends = [(position[c * chain_length], position[(c + 1) * chain_length - 1]) for c in range(chains)]
    return [texts[i] for i in order], [groups[i] for i in order], chain_ends


def read_buckets(folder):
    buckets = []
    for path in sorted(Path(folder).glob("*.dups")):
        data = path.read_bytes()
        offset = 0
        while offset < len(data):
            (size,) = struct.unpack_from("<I", data, offset)
            offset += 4
            members = [struct.unpack_from("<2I", data, offset + 8 * i)[1] for i in range(size)]
            offset += 8 * size
            buckets.append(members)
    return buckets


def union_find_keep(n_docs, buckets):
    parent = list(range(n_docs))

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for bucket in buckets:
        for other in bucket[1:]:
            a, b = find(bucket[0]), find(other)
            if a != b:
                parent[max(a, b)] = min(a, b)
    return {doc for doc in range(n_docs) if find(doc) == doc}


def feasibility_bound(buckets):
    degree = defaultdict(int)
    for bucket in buckets:
        for doc in bucket:
            degree[doc] += 1
    in_buckets = set(degree)
    # Any R with |R ∩ B| <= 1 for all B satisfies |R ∩ in_buckets| <= Σ_B 1 / min_{v∈B} d(v).
    return in_buckets, sum(1 / min(degree[doc] for doc in bucket) for bucket in buckets)


def evaluate(name, kept, n_docs, buckets, shingles, groups, threshold):
    removed = set(range(n_docs)) - kept
    kept_by_group = defaultdict(list)
    for doc in kept:
        kept_by_group[groups[doc]].append(doc)
    over_removed, best_similarities = 0, []
    for doc in removed:
        # Near-duplicates only arise within a generated group, so compare there.
        candidates = kept_by_group[groups[doc]]
        best = max((jaccard(shingles[doc], shingles[other]) for other in candidates), default=0.0)
        best_similarities.append(best)
        over_removed += best < threshold
    violated = sum(len(kept.intersection(bucket)) > 1 for bucket in buckets)
    worst = min(best_similarities, default=1.0)
    return name, len(kept), len(removed), over_removed, worst, violated


def jaccard(a, b):
    return len(a & b) / len(a | b) if a or b else 1.0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--chains", type=int, default=40)
    parser.add_argument("--chain-length", type=int, default=8)
    parser.add_argument("--exact-groups", type=int, default=40)
    parser.add_argument("--singletons", type=int, default=200)
    parser.add_argument("--words", type=int, default=300)
    parser.add_argument("--edits-per-step", type=int, default=10)
    parser.add_argument("--seed", type=int, default=0)
    args = parser.parse_args(argv)
    logger.remove()

    rng = random.Random(args.seed)
    texts, groups, chain_ends = make_corpus(
        args.chains, args.chain_length, args.exact_groups, args.singletons, args.words, args.edits_per_step, rng
    )
    config = MinhashConfig()
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        signatures, buckets_dir, pruned, clusters = (root / n for n in ("signatures", "buckets", "pruned", "clusters"))
        signature_stage = SignatureStage(signatures, config)
        signature_stage.run([Document(text, str(i)) for i, text in enumerate(texts)], 0, 1)
        for rank in range(config.num_buckets):
            BucketStage(signatures, buckets_dir, config=config).run(rank=rank, world_size=config.num_buckets)
        for rank in range(config.num_buckets):
            PruneStage(buckets_dir, pruned).run(rank=rank, world_size=config.num_buckets)
        SelectionStage(signatures, pruned, clusters, config).run()

        buckets = read_buckets(buckets_dir)
        removed = {doc for path in clusters.glob("*.remove") for (doc,) in struct.iter_unpack("<I", path.read_bytes())}
        shingles = [set(signature_stage.get_shingles(text).ravel().tolist()) for text in texts]

    n_docs = len(texts)
    in_buckets, bound = feasibility_bound(buckets)
    threshold = config.threshold
    rows = [
        evaluate("union-find (one per component)", union_find_keep(n_docs, buckets), n_docs, buckets, shingles, groups, threshold),
        evaluate("minhash-dedup (bucket-feasible)", set(range(n_docs)) - removed, n_docs, buckets, shingles, groups, threshold),
    ]

    end_to_end = [jaccard(shingles[a], shingles[b]) for a, b in chain_ends]
    print(f"corpus: {n_docs} docs = {args.chains} drift chains x {args.chain_length}, "
          f"{args.exact_groups} exact-duplicate groups, {args.singletons} singletons")
    print(f"LSH: {config.num_buckets} bands x {config.hashes_per_bucket} hashes, {config.n_grams}-char shingles, "
          f"threshold ~{threshold:.3f}; {len(buckets)} duplicate buckets over {len(in_buckets)} docs")
    print(f"mean Jaccard between the two ends of a chain: {sum(end_to_end) / len(end_to_end):.2f}\n")
    header = ("policy", "kept", "removed", "over-removed*", "worst removed J", "bucket violations")
    widths = (33, 6, 10, 16, 18, 20)
    print("".join(f"{h:<{w}}" if i == 0 else f"{h:>{w}}" for i, (h, w) in enumerate(zip(header, widths))))
    for row in rows:
        cells = [f"{v:.2f}" if isinstance(v, float) else str(v) for v in row]
        print("".join(f"{c:<{w}}" if i == 0 else f"{c:>{w}}" for i, (c, w) in enumerate(zip(cells, widths))))
    kept_in_buckets = rows[1][1] - (n_docs - len(in_buckets))
    print(f"\nupper bound on bucket-feasible retention among bucketed docs: {bound:.1f} "
          f"(minhash-dedup keeps {kept_in_buckets})")
    print(f"* removed documents whose most similar retained document has true Jaccard < {threshold:.3f}")
    return rows


if __name__ == "__main__":
    main()
