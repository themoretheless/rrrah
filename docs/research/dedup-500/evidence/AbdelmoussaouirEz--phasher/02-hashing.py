"""
Perceptual hash computation and cache management.

Handles single-image hashing, parallel folder hashing with
a JSON-backed cache that auto-invalidates on hash_size change.
"""

import json
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

from PIL import Image
import imagehash
from tqdm import tqdm

from .config import DEFAULT_HASH_SIZE, DEFAULT_WORKERS, CACHE_FILENAME
from .ui import info, warn, error, get_images


def compute_phash(img_path: Path, hash_size: int = DEFAULT_HASH_SIZE):
    """Return (path_str, hash_hex) or (path_str, None) on failure."""
    try:
        img = Image.open(img_path)
        h = imagehash.phash(img, hash_size=hash_size)
        return (str(img_path), str(h))
    except Exception:
        return (str(img_path), None)


def hash_folder(folder: Path, hash_size: int = DEFAULT_HASH_SIZE,
                num_workers: int = DEFAULT_WORKERS,
                use_cache: bool = True) -> dict:
    """Hash every image under *folder*, backed by a JSON cache."""
    images = get_images(folder)
    if not images:
        error(f"No images found in {folder}")
        return {}

    info(f"Found {len(images)} images in {folder}")

    # Load cache
    cache_path = folder / CACHE_FILENAME
    cache = {}
    if use_cache and cache_path.exists():
        try:
            with open(cache_path, "r") as f:
                raw = json.load(f)
            if raw.get("hash_size") == hash_size:
                cache = raw.get("hashes", {})
                info(f"Loaded cache with {len(cache)} entries (hash_size={hash_size})")
            else:
                warn(f"Cache hash_size mismatch ({raw.get('hash_size')}), recomputing")
        except Exception:
            warn("Cache file corrupted, recomputing")

    to_hash = []
    results = {}
    for img in images:
        key = str(img)
        if key in cache and cache[key] is not None:
            results[key] = cache[key]
        else:
            to_hash.append(img)

    if to_hash:
        info(f"Hashing {len(to_hash)} new images ({len(results)} cached)")
    else:
        info(f"All {len(results)} images loaded from cache")
        return results

    failed = 0
    with ThreadPoolExecutor(max_workers=num_workers) as pool:
        futures = {pool.submit(compute_phash, p, hash_size): p for p in to_hash}
        for future in tqdm(as_completed(futures), total=len(to_hash),
                           desc="Hashing", unit="img", ncols=80):
            path_str, hash_hex = future.result()
            if hash_hex is not None:
                results[path_str] = hash_hex
            else:
                failed += 1

    if failed:
        warn(f"{failed} images failed to hash")

    if use_cache:
        try:
            with open(cache_path, "w") as f:
                json.dump({"hash_size": hash_size, "hashes": results}, f)
            info(f"Cache saved ({len(results)} hashes)")
        except Exception as e:
            warn(f"Could not save cache: {e}")

    return results
