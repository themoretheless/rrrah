import json
import os
import sys
from typing import List, Sequence

_REPO_SRC = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "src"))
if _REPO_SRC not in sys.path:
    sys.path.insert(0, _REPO_SRC)

from minhash_dedup.minhash import MinhashConfig
from minhash_dedup.signatures import HashConfig

minhash_config = MinhashConfig(
    hash_config=HashConfig(precision=64),
    num_buckets=14,
    hashes_per_bucket=9,
    seed=1,
)  # 64-bit hashes reduce collision false positives

_override_num_buckets = os.environ.get("MINHASH_NUM_BUCKETS")
_override_hashes_per_bucket = os.environ.get("MINHASH_HASHES_PER_BUCKET")
_override_hash_seed = os.environ.get("MINHASH_SEED")

if _override_num_buckets or _override_hashes_per_bucket or _override_hash_seed:
    try:
        num_buckets = int(_override_num_buckets) if _override_num_buckets else minhash_config.num_buckets
        hashes_per_bucket = int(_override_hashes_per_bucket) if _override_hashes_per_bucket else minhash_config.hashes_per_bucket
        hash_seed = int(_override_hash_seed if _override_hash_seed else minhash_config.seed)
    except ValueError as exc:
        raise ValueError("MINHASH_* overrides must be numeric") from exc
    if num_buckets <= 0:
        raise ValueError(f"MINHASH_NUM_BUCKETS must be positive; got {num_buckets}")
    if hashes_per_bucket <= 0:
        raise ValueError(f"MINHASH_HASHES_PER_BUCKET must be positive; got {hashes_per_bucket}")
    if hash_seed < 0:
        raise ValueError(f"MINHASH_SEED must be non-negative; got {hash_seed}")

    minhash_config = MinhashConfig(
        hash_config=minhash_config.hash_config,
        num_buckets=num_buckets,
        hashes_per_bucket=hashes_per_bucket,
        seed=hash_seed,
    )
            
# Input directories in decreasing source priority, one label per directory.
# Normally supplied through the S3_INPUT / DATA_LABEL / OUTPUT_PATH environment
# variables as JSON lists (or comma-separated values).
S3_INPUT: List[str] = []
DATA_LABEL: List[str] = []
OUTPUT_PATH = ""


def _parse_override_list(raw_value: str, current: Sequence[str]) -> List[str]:
    value = raw_value.strip()
    if not value:
        return list(current)
    try:
        parsed = json.loads(value)
        if isinstance(parsed, list) and all(isinstance(item, str) for item in parsed):
            return parsed
    except json.JSONDecodeError:
        pass
    return [item.strip() for item in value.split(",") if item.strip()]


_override_s3 = os.environ.get("S3_INPUT")
if _override_s3:
    S3_INPUT = _parse_override_list(_override_s3, S3_INPUT)

_override_labels = os.environ.get("DATA_LABEL")
if _override_labels:
    DATA_LABEL = _parse_override_list(_override_labels, DATA_LABEL)

_override_output = os.environ.get("OUTPUT_PATH")
if _override_output:
    OUTPUT_PATH = _override_output.strip()

if not S3_INPUT or not OUTPUT_PATH:
    raise ValueError("Set S3_INPUT and OUTPUT_PATH (see scripts/minhash_driver.sh)")
if len(DATA_LABEL) < len(S3_INPUT):
    raise ValueError(
        f"DATA_LABEL must have at least one label per S3_INPUT path; "
        f"got {len(DATA_LABEL)} labels for {len(S3_INPUT)} inputs"
    )

SIGNATURES_PATH = f"{OUTPUT_PATH}/signatures"
BUCKETS_PATH = f"{OUTPUT_PATH}/buckets"
BUCKETS_PRUNED_PATH = f"{OUTPUT_PATH}/buckets_pruned"
REMOVE_IDS_PATH = f"{OUTPUT_PATH}/remove_ids"
DEDUPED_PATH = f"{OUTPUT_PATH}/deduped"
REMOVED_PATH = f"{OUTPUT_PATH}/removed"

COVERED_PATH = f"{OUTPUT_PATH}/buckets_covered"
COVERED2_PATH = f"{OUTPUT_PATH}/buckets_covered2"
