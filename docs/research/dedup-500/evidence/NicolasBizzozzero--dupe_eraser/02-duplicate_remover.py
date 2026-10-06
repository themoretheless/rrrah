import hashlib
import logging
import os
import time
from collections import defaultdict
from typing import Dict, List, Optional, Tuple

import blake3
import humanize
import mmh3
import xxhash
from tqdm import tqdm
from PIL import Image
import imagehash
from tabulate import tabulate

from core.comparison_method import ComparisonMethod
from core.hash_performance import HashPerformance


class DuplicateRemover:
    # Cryptographic hash functions
    CRYPTO_HASHES = {
        "md5": hashlib.md5,
        "sha1": hashlib.sha1,
        "sha256": hashlib.sha256,
        "sha512": hashlib.sha512,
        "sha3_256": hashlib.sha3_256,
        "sha3_512": hashlib.sha3_512,
        "blake2b": hashlib.blake2b,
        "blake2s": hashlib.blake2s,
        "blake3": lambda: blake3.blake3(),
    }

    # Non-cryptographic hash functions (faster)
    FAST_HASHES = {
        "xxh32": lambda: xxhash.xxh32(),
        "xxh64": lambda: xxhash.xxh64(),
        "xxh3_64": lambda: xxhash.xxh3_64(),
        "xxh3_128": lambda: xxhash.xxh3_128(),
        # mmh3 is not incremental in Python; handled specially in get_file_hash()
        "murmur3_32": lambda: "MMH3_SPECIAL",
    }

    # Perceptual hash functions for images
    PERCEPTUAL_HASHES = {
        "phash": imagehash.average_hash,
        "dhash": imagehash.dhash,
        "whash": imagehash.whash,
        "colorhash": imagehash.colorhash,
    }

    def __init__(
        self,
        comparison_method: str = ComparisonMethod.HASH,
        hash_algorithm: str = "sha256",
        hash_progress_threshold_mb: int = 10,
        perceptual_threshold: int = 5,  # hamming distance for perceptual hashes
        show_progress: bool = True,
        print_only: bool = False,
    ):
        self.comparison_method = comparison_method
        self.hash_algorithm = hash_algorithm
        self.perceptual_threshold = perceptual_threshold
        self.show_progress = show_progress
        self.print_only = print_only

        if comparison_method == ComparisonMethod.HASH:
            if hash_algorithm in self.CRYPTO_HASHES:
                self.hash_func = self.CRYPTO_HASHES[hash_algorithm]
                self.hash_type = "crypto"
            elif hash_algorithm in self.FAST_HASHES:
                self.hash_func = self.FAST_HASHES[hash_algorithm]
                self.hash_type = "fast"
            elif hash_algorithm in self.PERCEPTUAL_HASHES:
                self.hash_func = self.PERCEPTUAL_HASHES[hash_algorithm]
                self.hash_type = "perceptual"
            else:
                raise ValueError(
                    f"Unsupported hash algorithm. Available options:\\n"
                    f"Cryptographic: {', '.join(self.CRYPTO_HASHES.keys())}\\n"
                    f"Fast: {', '.join(self.FAST_HASHES.keys())}\\n"
                    f"Perceptual: {', '.join(self.PERCEPTUAL_HASHES.keys())}"
                )

        self.hash_threshold = hash_progress_threshold_mb * 1024 * 1024
        self.total_files_processed = 0
        self.total_bytes_processed = 0
        self.duplicates_found = 0
        self.space_saved = 0
        self.performance = HashPerformance(
            hash_algorithm if comparison_method == ComparisonMethod.HASH else "bytes"
        )

    # ---------- Public API ----------

    def find_and_remove_duplicates(
        self, root_dir: str, disable_progress: bool = False
    ) -> Dict[str, str]:
        """
        Recursively scan the entire tree, detect duplicates globally, and (optionally) delete.
        Returns a mapping {duplicate_path: kept_path}.
        """
        deleted_files: Dict[str, str] = {}

        logging.info("[SCAN] Calculating total files and size...")
        file_records: List[Tuple[str, int]] = []
        total_size = 0
        for dirpath, _, filenames in os.walk(root_dir):
            for name in filenames:
                p = os.path.join(dirpath, name)
                try:
                    if not os.path.isfile(p):
                        continue
                    size = os.path.getsize(p)
                except OSError:
                    continue
                file_records.append((p, size))
                total_size += size

        total_files = len(file_records)
        logging.info(
            f"Found {total_files} files (Total size: {humanize.naturalsize(total_size)})"
        )

        method_str = (
            "byte-by-byte comparison"
            if self.comparison_method == ComparisonMethod.BYTES
            else f"{self.hash_algorithm} hashing"
        )
        logging.info(f"Using {method_str} for file comparison")

        # Outer progress bar (one bar only)
        pbar = None
        if not disable_progress and self.show_progress:
            pbar = tqdm(
                total=total_files,
                desc="[SCAN] Files",
                unit="file",
                colour="green",
                leave=False,
                postfix=self._get_progress_stats(),
                position=0,
                dynamic_ncols=True,
            )

        # Build global buckets by file size across the whole tree
        size_buckets: Dict[int, List[Tuple[str, int]]] = defaultdict(list)
        for path, size in file_records:
            size_buckets[size].append((path, size))
            self.total_files_processed += 1
            if pbar:
                pbar.update(1)
                pbar.set_postfix(**self._get_progress_stats(current_dir=os.path.dirname(path)))
                pbar.refresh()

        # Now, within each size bucket, do deeper comparison
        for size, entries in size_buckets.items():
            if len(entries) < 2:
                continue

            if self.comparison_method == ComparisonMethod.HASH:
                # Compute digest per file, reusing results, and always log at DEBUG
                digest_groups: Dict[str, List[str]] = defaultdict(list)
                for path, _ in entries:
                    try:
                        digest = self.get_file_hash(path, size, show_inner_bar=self.show_progress)
                        digest_groups[digest].append(path)
                        logging.debug(f"HASH {digest}  SIZE {size}  FILE {path}")
                    except Exception as e:
                        logging.warning(f"[hash] skipped {path}: {e}")
                        continue

                # For each digest group with >1, keep first, mark all others as duplicates
                for digest, dpaths in digest_groups.items():
                    if len(dpaths) < 2:
                        continue
                    dpaths_sorted = sorted(dpaths)
                    keep = dpaths_sorted[0]
                    for duplicate in dpaths_sorted[1:]:
                        if self.print_only:
                            tqdm.write(f"Would delete duplicate: {duplicate} -> kept {keep}")
                        else:
                            try:
                                os.remove(duplicate)
                                tqdm.write(f"Deleted duplicate: {duplicate} -> kept {keep}")
                            except OSError as e:
                                logging.error(f"Error deleting {duplicate}: {e}")
                                continue
                        deleted_files[duplicate] = keep
                        self.space_saved += size
                        self.duplicates_found += 1
                        if pbar:
                            pbar.set_postfix(**self._get_progress_stats())
                            pbar.refresh()

            else:  # BYTES
                # Use first as canonical, compare every other to it (N-1 possible matches)
                sorted_entries = sorted(entries, key=lambda t: t[0])
                ref = sorted_entries[0][0]
                for other, _ in sorted_entries[1:]:
                    try:
                        if self.compare_files_bytes(ref, other, size, show_inner_bar=self.show_progress):
                            if self.print_only:
                                tqdm.write(f"Would delete duplicate: {other} -> kept {ref}")
                            else:
                                try:
                                    os.remove(other)
                                    tqdm.write(f"Deleted duplicate: {other} -> kept {ref}")
                                except OSError as e:
                                    logging.error(f"Error deleting {other}: {e}")
                                    continue
                            deleted_files[other] = ref
                            self.space_saved += size
                            self.duplicates_found += 1
                            if pbar:
                                pbar.set_postfix(**self._get_progress_stats())
                                pbar.refresh()
                    except Exception as e:
                        logging.warning(f"[compare] skipped {other}: {e}")

        if pbar:
            pbar.close()

        return deleted_files

    # ---------- Helpers ----------

    def compare_files_bytes(self, file1: str, file2: str, file_size: int, show_inner_bar: bool = True) -> bool:
        """Compare two files byte-by-byte with an inner progress bar to show long operations."""
        chunk_size = 1 << 20  # 1 MiB
        total_read = 0
        pbar_inner = None
        # Only create inner bar for large files
        if show_inner_bar and file_size > self.hash_threshold:
            pbar_inner = tqdm(
                total=file_size,
                desc=f"[CMP] {os.path.basename(file1)} vs {os.path.basename(file2)}",
                unit="B",
                unit_scale=True,
                colour="yellow",
                leave=False,
                position=1,
                dynamic_ncols=True,
            )

        with open(file1, "rb") as f1, open(file2, "rb") as f2:
            while True:
                b1 = f1.read(chunk_size)
                b2 = f2.read(chunk_size)
                if b1 != b2:
                    if pbar_inner:
                        pbar_inner.close()
                    return False
                if not b1:
                    break
                if pbar_inner:
                    total_read += len(b1)
                    pbar_inner.update(len(b1))

        if pbar_inner:
            pbar_inner.close()

        # Count a full compare as reading one file's bytes
        self.total_bytes_processed += file_size
        return True

    def get_file_hash(self, filepath: str, file_size: int, show_inner_bar: bool = True) -> str:
        """
        Calculate file hash (or perceptual hash). Show an inner per-file bar for large files.
        """
        start_time = time.time()

        if hasattr(self, "hash_type") and self.hash_type == "perceptual":
            if not self.is_image_file(filepath):
                return "non_image"
            try:
                with Image.open(filepath) as img:
                    hash_value = str(self.hash_func(img))
            except Exception:
                return "invalid_image"
        else:
            if hasattr(self, "hash_type") and self.hash_type == "fast" and self.hash_algorithm == "murmur3_32":
                with open(filepath, "rb") as f:
                    data = f.read()
                val = mmh3.hash(data, signed=False)
                hash_value = f"{val:08x}"
            else:
                hasher = self.hash_func() if hasattr(self, "hash_func") else hashlib.sha256()
                chunk_size = 1 << 20  # 1 MiB

                pbar_inner = None
                if show_inner_bar and file_size > self.hash_threshold:
                    pbar_inner = tqdm(
                        total=file_size,
                        desc=f"[HASH] {os.path.basename(filepath)} ({self.hash_algorithm})",
                        unit="B",
                        unit_scale=True,
                        colour="yellow",
                        leave=False,
                        position=1,
                        dynamic_ncols=True,
                    )

                with open(filepath, "rb") as f:
                    for byte_block in iter(lambda: f.read(chunk_size), b""):
                        hasher.update(byte_block)
                        if pbar_inner:
                            pbar_inner.update(len(byte_block))

                if pbar_inner:
                    pbar_inner.close()

                hash_value = hasher.hexdigest() if hasattr(hasher, "hexdigest") else str(hasher)

        elapsed_time = time.time() - start_time
        self.performance.times.append(elapsed_time)
        self.performance.sizes.append(file_size)
        self.total_bytes_processed += file_size

        return hash_value

    def is_image_file(self, filepath: str) -> bool:
        image_extensions = {".jpg", ".jpeg", ".png", ".gif", ".bmp", ".tiff"}
        return os.path.splitext(filepath)[1].lower() in image_extensions

    @staticmethod
    def count_files_and_size(root_dir: str) -> Tuple[int, int]:
        total_files = 0
        total_size = 0
        for dirpath, _, files in os.walk(root_dir):
            total_files += len(files)
            for file in files:
                try:
                    total_size += os.path.getsize(os.path.join(dirpath, file))
                except OSError:
                    continue
        return total_files, total_size

    @staticmethod
    def benchmark_hashes(sample_file: str, iterations: int = 3) -> List[HashPerformance]:
        results = []
        file_size = os.path.getsize(sample_file)
        all_hashes = {**DuplicateRemover.CRYPTO_HASHES, **DuplicateRemover.FAST_HASHES}

        for name, hash_func in all_hashes.items():
            if name == "murmur3_32":
                perf = HashPerformance(name)
                for _ in range(iterations):
                    start_time = time.time()
                    with open(sample_file, "rb") as f:
                        data = f.read()
                        _ = mmh3.hash(data, signed=False)
                    perf.times.append(time.time() - start_time)
                    perf.sizes.append(file_size)
                results.append(perf)
                continue

            perf = HashPerformance(name)
            for _ in range(iterations):
                hasher = hash_func()
                start_time = time.time()
                with open(sample_file, "rb") as f:
                    for chunk in iter(lambda: f.read(1 << 20), b""):
                        hasher.update(chunk)
                perf.times.append(time.time() - start_time)
                perf.sizes.append(file_size)
            results.append(perf)

        results.sort(key=lambda x: x.avg_speed_mbps, reverse=True)
        return results

    @staticmethod
    def print_benchmark_results(results: List[HashPerformance]) -> None:
        table_data = []
        for perf in results:
            table_data.append([perf.name, f"{perf.avg_speed_mbps:.1f} MB/s", f"{perf.avg_time_ms:.2f} ms"])
        logging.info("\\nHash Algorithm Benchmark Results:")
        logging.info(tabulate(table_data, headers=["Algorithm", "Speed", "Avg Time"], tablefmt="grid"))

    def _get_progress_stats(self, current_dir: Optional[str] = None) -> dict:
        stats = {
            "processed": f"{self.total_files_processed} files",
            "data": humanize.naturalsize(self.total_bytes_processed),
            "duplicates": str(self.duplicates_found),
            "saved": humanize.naturalsize(self.space_saved),
        }
        if current_dir:
            stats["dir"] = current_dir
        return stats
