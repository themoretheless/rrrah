"""
[5] Match folder vs folder — find target images that look like reference examples.
"""

import json
import time
import shutil
from pathlib import Path
from collections import defaultdict

import numpy as np
from tqdm import tqdm

from ..ui import (
    info, warn, error, success, ask, ask_int, ask_yes,
    get_images, RICH, console,
)
from ..config import DEFAULT_HAMMING, DEFAULT_HASH_SIZE
from ..hashing import hash_folder
from ..hamming import build_hash_matrix, vectorized_hamming_best

try:
    from rich.table import Table
except ImportError:
    Table = None


def match_folders():
    """Find images in a target folder that are similar to reference examples."""
    print("\n-- Match Folder vs Folder --\n")

    ref_path = ask("Reference folder (known examples)")
    if not ref_path or not Path(ref_path).is_dir():
        error("Invalid reference folder"); return

    target_path = ask("Target folder (pool to search)")
    if not target_path or not Path(target_path).is_dir():
        error("Invalid target folder"); return

    ref_folder = Path(ref_path).resolve()
    target_folder = Path(target_path).resolve()
    if ref_folder == target_folder:
        error("Reference and target must be different"); return

    # Pre-scan
    print()
    info("== Pre-Scan ==")
    ref_images = get_images(ref_folder)
    target_images = get_images(target_folder)

    ref_name_counts = defaultdict(int)
    for img in ref_images:
        ref_name_counts[img.name] += 1

    target_subfolders = set()
    target_names = set()
    for img in target_images:
        target_names.add(img.name)
        parent = img.parent.resolve()
        if parent != target_folder:
            target_subfolders.add(parent)

    info(f"Reference: {len(ref_images)} images in {ref_folder}")
    info(f"Target:    {len(target_images)} images in {target_folder}")
    if target_subfolders:
        info(f"  Target subfolders: {len(target_subfolders)}")

    overlapping = set(ref_name_counts.keys()) & target_names
    skip_same_names = False
    if overlapping:
        warn(f"{len(overlapping)} filenames exist in both folders")
        skip_same_names = ask_yes(f"Skip these {len(overlapping)} overlapping names?", default=True)

    if not ref_images or not target_images:
        error("Both folders need images"); return
    if not ask_yes("Proceed?", default=True):
        return

    hamming_thresh = ask_int("Hamming distance threshold", default=DEFAULT_HAMMING)
    hash_size = ask_int("Hash size (8=fast, 16=accurate)", default=DEFAULT_HASH_SIZE)

    expand_folders = False
    if target_subfolders:
        info(f"Target has {len(target_subfolders)} subfolders (pre-clustered data).")
        info("Folder expand: if ONE image in a subfolder matches, ALL siblings are included.")
        expand_folders = ask_yes("Enable folder expand?", default=True)

    # Hash
    print()
    info("=== Hashing reference ===")
    ref_hashes = hash_folder(ref_folder, hash_size)
    if not ref_hashes:
        return

    print()
    info("=== Hashing target ===")
    target_hashes = hash_folder(target_folder, hash_size)
    if not target_hashes:
        return

    # Filter target: exclude reference images
    ref_full_paths = set(ref_hashes.keys())
    ref_name_set = set(Path(p).name for p in ref_hashes.keys())

    filtered_target = {}
    skipped = 0
    for t_path, t_hash in target_hashes.items():
        if t_path in ref_full_paths:
            skipped += 1; continue
        if skip_same_names and Path(t_path).name in ref_name_set:
            skipped += 1; continue
        filtered_target[t_path] = t_hash

    if skipped:
        info(f"Skipped {skipped} overlapping images")

    # Build folder map for expand mode
    target_folder_map = defaultdict(list)
    for t_path, t_hash in filtered_target.items():
        target_folder_map[str(Path(t_path).parent)].append((t_path, t_hash))

    # Deduplicate reference hashes
    ref_unique = {}
    for fp, h in ref_hashes.items():
        if h not in ref_unique:
            ref_unique[h] = []
        ref_unique[h].append(fp)

    ref_hex_list = list(ref_unique.keys())
    ref_hex_files = [ref_unique[h][0] for h in ref_hex_list]

    info(f"Comparing {len(ref_hex_list)} unique refs vs {len(filtered_target)} targets...")

    # Vectorized matching
    target_paths = list(filtered_target.keys())
    target_hex_list = [filtered_target[p] for p in target_paths]

    t0 = time.time()
    ref_matrix = build_hash_matrix(ref_hex_list)
    target_matrix = build_hash_matrix(target_hex_list)

    info(f"  Ref matrix:    {ref_matrix.shape}")
    info(f"  Target matrix: {target_matrix.shape}")
    info(f"  Comparisons:   {ref_matrix.shape[0] * target_matrix.shape[0]:,}")

    best_ref_idx, best_dist = vectorized_hamming_best(ref_matrix, target_matrix, hamming_thresh)
    info(f"  Done in {time.time() - t0:.2f}s")

    # Distance distribution
    below_thresh = int(np.sum(best_dist <= hamming_thresh))
    info(f"  Matches (dist <= {hamming_thresh}): {below_thresh}")

    # Build results
    matched_files = {}
    matched_folders = set()
    ref_hashes_matched = set()

    for t_idx in range(len(target_paths)):
        dist = int(best_dist[t_idx])
        if dist <= hamming_thresh:
            t_path = target_paths[t_idx]
            r_idx = int(best_ref_idx[t_idx])
            matched_files[t_path] = (dist, ref_hex_files[r_idx])
            ref_hashes_matched.add(ref_hex_list[r_idx])

    direct_count = len(matched_files)

    # Folder expansion
    if expand_folders and matched_files:
        for t_path in list(matched_files.keys()):
            t_parent = str(Path(t_path).parent)
            if t_parent not in matched_folders:
                matched_folders.add(t_parent)
                dist, ref_file = matched_files[t_path]
                for sib_path, _ in target_folder_map[t_parent]:
                    if sib_path not in matched_files:
                        matched_files[sib_path] = (dist, ref_file)
        info(f"  After expansion: {len(matched_files)} ({len(matched_files) - direct_count} siblings)")

    matches = sorted(
        [(t, d, r) for t, (d, r) in matched_files.items()],
        key=lambda x: x[1]
    )
    no_match = [t for t in filtered_target if t not in matched_files]

    # Unmatched refs
    unmatched_refs = []
    for h, files in ref_unique.items():
        if h not in ref_hashes_matched:
            unmatched_refs.extend(files)

    print()
    success(f"Found {len(matches)} matching target images")
    info(f"  Non-matching: {len(no_match)}")
    info(f"  Ref coverage: {len(ref_hashes) - len(unmatched_refs)}/{len(ref_hashes)}")
    if unmatched_refs:
        warn(f"  {len(unmatched_refs)} reference images had no match")

    if not matches:
        warn("No matches. Try a higher threshold."); return

    # Preview
    show_n = min(30, len(matches))
    if RICH and Table:
        table = Table(title=f"Top {show_n} Matches")
        table.add_column("#", style="dim", width=4)
        table.add_column("Dist", style="bold cyan", width=6)
        table.add_column("Target", style="white")
        table.add_column("Closest Ref", style="dim")
        for i, (t, d, r) in enumerate(matches[:show_n], 1):
            s = "green" if d <= 2 else "yellow" if d <= 5 else "red"
            table.add_row(str(i), f"[{s}]{d}[/]", Path(t).name, Path(r).name)
        console.print(table)

    # Output
    print()
    action = ask("Action: (c)opy / (m)ove / (s)eparate matched+unmatched / (r)eport / (n)one",
                 default="c").lower()

    if action == "n":
        return

    out_dir = ask("Output directory", default="folder_match_results")
    out_path = Path(out_dir)
    out_path.mkdir(parents=True, exist_ok=True)

    if action in ("c", "m"):
        matched_dir = out_path / "matched"
        matched_dir.mkdir(exist_ok=True)
        op = shutil.copy2 if action == "c" else shutil.move
        op_name = "Copying" if action == "c" else "Moving"

        done, errs = 0, 0
        for t_path, dist, _ in tqdm(matches, desc=op_name, ncols=80):
            src = Path(t_path)
            dst = matched_dir / src.name
            if dst.exists():
                dst = matched_dir / f"{src.stem}_d{dist}{src.suffix}"
            try:
                if src.exists():
                    op(str(src), str(dst)); done += 1
                else:
                    errs += 1
            except Exception:
                errs += 1
        success(f"{op_name}: {done} files -> {matched_dir}/")
        if errs:
            warn(f"  {errs} failed")

    elif action == "s":
        matched_dir = out_path / "matched"
        unmatched_dir = out_path / "unmatched"
        matched_dir.mkdir(exist_ok=True)
        unmatched_dir.mkdir(exist_ok=True)

        use_copy = ask("Use (c)opy or (m)ove?", default="c").lower() == "c"
        op = shutil.copy2 if use_copy else shutil.move

        done_m, done_u, errs = 0, 0, 0
        for t_path, dist, _ in tqdm(matches, desc="Matched", ncols=80):
            src = Path(t_path)
            dst = matched_dir / src.name
            if dst.exists():
                dst = matched_dir / f"{src.stem}_d{dist}{src.suffix}"
            try:
                if src.exists():
                    op(str(src), str(dst)); done_m += 1
                else:
                    errs += 1
            except Exception:
                errs += 1

        for t_path in tqdm(no_match, desc="Unmatched", ncols=80):
            src = Path(t_path)
            dst = unmatched_dir / src.name
            if dst.exists():
                dst = unmatched_dir / f"{src.stem}_dup{src.suffix}"
            try:
                if src.exists():
                    op(str(src), str(dst)); done_u += 1
                else:
                    errs += 1
            except Exception:
                errs += 1

        success(f"Matched: {done_m}, Unmatched: {done_u}")
        if errs:
            warn(f"  {errs} failed")

    elif action == "r":
        report = {
            "reference_folder": str(ref_folder),
            "target_folder": str(target_folder),
            "hash_size": hash_size, "hamming_threshold": hamming_thresh,
            "total_reference": len(ref_hashes),
            "unique_ref_hashes": len(ref_hex_list),
            "total_target": len(target_hashes),
            "filtered_target": len(filtered_target),
            "total_matches": len(matches),
            "total_non_matches": len(no_match),
            "unmatched_refs": [Path(p).name for p in unmatched_refs],
            "matches": [{"target": t, "distance": d, "closest_ref": r}
                        for t, d, r in matches]
        }
        rpt_file = out_path / "match_report.json"
        with open(rpt_file, "w") as f:
            json.dump(report, f, indent=2)
        success(f"Report saved to {rpt_file}")
