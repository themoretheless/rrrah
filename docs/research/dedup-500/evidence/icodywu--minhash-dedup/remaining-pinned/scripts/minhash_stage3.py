import os
import sys

_REPO_SRC = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "src"))
if _REPO_SRC not in sys.path:
    sys.path.insert(0, _REPO_SRC)

from minhash_dedup.minhash import SelectionStage
import dedup_params as param

print("Starting Stage 3 script...")


def _parse_bool_env(name: str, default: bool = False) -> bool:
    raw = os.environ.get(name)
    if raw is None:
        return default
    value = raw.strip().lower()
    if value in {"1", "true", "yes", "y", "on"}:
        return True
    if value in {"0", "false", "no", "n", "off"}:
        return False
    return default


analyze_clustering = _parse_bool_env("MINHASH_ANALYZE_CLUSTERING", default=False)

def _has_dups(path: str) -> bool:
    if not os.path.isdir(path):
        return False
    for root, _, files in os.walk(path):
        for name in files:
            if name.endswith(".dups"):
                return True
    return False

input_buckets = param.BUCKETS_PRUNED_PATH if _has_dups(param.BUCKETS_PRUNED_PATH) else param.BUCKETS_PATH

SelectionStage(
        signature_folder=param.SIGNATURES_PATH,
        input_folder=input_buckets,
        output_folder=param.REMOVE_IDS_PATH,
        config=param.minhash_config,
).compute_punct_bound()

SelectionStage(
        signature_folder=param.SIGNATURES_PATH,
        input_folder=input_buckets,
        output_folder=param.COVERED_PATH,
        config=param.minhash_config,
).compute_cov_bound()

SelectionStage(
        signature_folder=param.SIGNATURES_PATH,
        input_folder=param.COVERED_PATH,
        output_folder=param.COVERED2_PATH,
        config=param.minhash_config,
).compute_cov_bound()

SelectionStage(
        signature_folder=param.SIGNATURES_PATH,
        input_folder=input_buckets,
        output_folder=param.REMOVE_IDS_PATH,
        config=param.minhash_config,
        analyze_clustering=analyze_clustering,
).run()
