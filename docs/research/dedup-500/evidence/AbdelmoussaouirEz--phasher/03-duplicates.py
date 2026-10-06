"""
[2] Find duplicates — detect identical or near-identical copies.
"""

import os
import json
import shutil
from pathlib import Path
from collections import defaultdict

from ..ui import info, warn, error, success, ask, ask_int, ask_yes
from ..config import DEFAULT_HASH_SIZE
from ..hashing import hash_folder
from ..hamming import build_hash_matrix, vectorized_pairwise_below
from ..union_find import UnionFind


def find_duplicates():
    """Find exact or near-exact duplicate images."""
    print("\n-- Find Duplicates --\n")

    folder_path = ask("Folder to scan for duplicates")
    if not folder_path or not Path(folder_path).is_dir():
        error("Invalid folder path"); return

    hamming_thresh = ask_int("Hamming threshold (0=exact, 1-3=near-exact)", default=0)
    hash_size = ask_int("Hash size", default=DEFAULT_HASH_SIZE)

    folder = Path(folder_path)
    hashes = hash_folder(folder, hash_size)
    if not hashes:
        return

    info("Finding duplicates...")

    # Group by exact hash first
    exact_groups = defaultdict(list)
    for filepath, h in hashes.items():
        exact_groups[h].append(filepath)

    if hamming_thresh == 0:
        dup_groups = [files for files in exact_groups.values() if len(files) >= 2]
    else:
        # Vectorized merge using Union-Find
        group_keys = list(exact_groups.keys())
        n = len(group_keys)
        mat = build_hash_matrix(group_keys)

        uf = UnionFind(n)
        close_pairs = vectorized_pairwise_below(mat, hamming_thresh)
        for i, j, _ in close_pairs:
            uf.union(i, j)

        clusters = defaultdict(list)
        for idx in range(n):
            root = uf.find(idx)
            clusters[root].extend(exact_groups[group_keys[idx]])

        dup_groups = [files for files in clusters.values() if len(files) >= 2]

    dup_groups.sort(key=len, reverse=True)
    total_dups = sum(len(g) for g in dup_groups)
    wasted = sum(len(g) - 1 for g in dup_groups)

    print()
    if not dup_groups:
        success("No duplicates found!"); return

    success(f"Found {len(dup_groups)} duplicate groups ({total_dups} images, {wasted} removable)")
    print()

    action = ask("Action: (c)opy to folder / (d)elete dupes (keep 1) / (r)eport only", default="r").lower()

    if action == "r":
        out_path = Path(ask("Report output dir", default="duplicates_report"))
        out_path.mkdir(parents=True, exist_ok=True)
        report = {
            "total_groups": len(dup_groups), "total_duplicates": total_dups,
            "removable": wasted,
            "groups": [{"size": len(g), "files": g} for g in dup_groups]
        }
        rpt_file = out_path / "duplicates_report.json"
        with open(rpt_file, "w") as f:
            json.dump(report, f, indent=2)
        success(f"Report saved to {rpt_file}")

    elif action == "c":
        out_path = Path(ask("Output dir", default="duplicates_found"))
        out_path.mkdir(parents=True, exist_ok=True)
        pad = len(str(len(dup_groups)))
        for idx, group in enumerate(dup_groups, 1):
            grp_dir = out_path / f"dup_{idx:0{pad}d}"
            grp_dir.mkdir(exist_ok=True)
            for fp in group:
                shutil.copy2(fp, grp_dir / Path(fp).name)
        success(f"Copied {len(dup_groups)} groups to {out_path}/")

    elif action == "d":
        warn("This will DELETE duplicate files (keeping one per group).")
        if ask_yes("Are you sure?", default=False):
            deleted = 0
            for group in dup_groups:
                for fp in group[1:]:
                    try:
                        os.remove(fp)
                        deleted += 1
                    except Exception as e:
                        error(f"Could not delete {fp}: {e}")
            success(f"Deleted {deleted} duplicate files")
        else:
            info("Cancelled.")
