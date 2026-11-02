import os
import sys
import argparse

_REPO_SRC = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "src"))
if _REPO_SRC not in sys.path:
    sys.path.insert(0, _REPO_SRC)

from minhash_dedup.jsonl import JsonlReader
from minhash_dedup.minhash import SignatureStage
from minhash_dedup.partition import ReaderPlan
from dataclasses import asdict
import dedup_params as param

from typing import Dict, Any
import hashlib
from pathlib import Path

def adapter(
    self, data: Dict[str, Any], source_file: str, id_in_file: int
) -> Dict[str, Any]:
    """
    Adapt data format, trying to use id_in_file for unique ID generation.

    Args:
        self: Instance of the reader class
        data: Input data dictionary
        source_file: Source file path
        id_in_file: ID of the document within the file

    Returns:
        Adapted data dictionary
    """
    text = data["text"]
    
    # Get id from meta field, or generate one if not present
    meta = data.get("meta", {}) if isinstance(data.get("meta"), dict) else {}
    doc_id = meta.get("id")
    if not doc_id: # None, "", or key missing
        # Use id_in_file if available and valid, otherwise hash
        if id_in_file is not None and id_in_file != -1:
             file_stem = Path(source_file).stem
             doc_id = f"{file_stem}::{id_in_file}"
        else:
             doc_id = hashlib.md5(text.encode("utf8")).hexdigest()
    else:
        doc_id = str(doc_id) # Ensure ID is string

    # Merge existing meta (if any) and add the subset tag
    metadata = dict(data.get("meta", {}))          # shallow copy
    if "subset" in data:
        metadata["subset"] = data["subset"]

    # Optionally pass through media (default empty list)
    media = data.get("media", [])

    return {
        "text": text,
        "id": doc_id, # Use the determined doc_id
        "media": media,
        "metadata": metadata,
    }


parser = argparse.ArgumentParser(description="Run Minhash Stage 1")
parser.add_argument("--task-id", type=int, help="Launched worker rank, not a logical reader rank")
parser.add_argument("--task-size", "--task_size", type=int, default=None,
                    help="Logical world_size; defaults to the saved plan or SLURM_NTASKS")
mode = parser.add_mutually_exclusive_group()
mode.add_argument("--prepare-only", action="store_true", help="Persist the reader plan without generating signatures")
mode.add_argument("--check-complete", action="store_true", help="Verify completion of every planned active rank")
args = parser.parse_args()

launch_world_size = int(os.environ.get("SLURM_NTASKS", 1))
task_size = args.task_size
if task_size is None and os.environ.get("TASK_SIZE"):
    task_size = int(os.environ["TASK_SIZE"])
if launch_world_size < 1:
    parser.error("SLURM_NTASKS must be positive")
if not (args.prepare_only or args.check_complete) and (args.task_id is None or not 0 <= args.task_id < launch_world_size):
    parser.error("--task-id must be in [0, SLURM_NTASKS)")

if args.check_complete:
    plan = ReaderPlan.load(param.SIGNATURES_PATH)
    plan.validate(param.S3_INPUT, task_size, asdict(param.minhash_config))
    for worker_rank in range(plan.active_ranks):
        plan.require_complete(param.SIGNATURES_PATH, worker_rank)
    print(f"Stage 1 complete: logical world_size={plan.world_size}, active ranks={plan.active_ranks}")
    sys.exit(0)
plan = ReaderPlan.prepare(param.SIGNATURES_PATH, param.S3_INPUT, task_size, launch_world_size, asdict(param.minhash_config))
print(f"Stage 1: logical world_size={plan.world_size}, active ranks={plan.active_ranks}, launched workers={launch_world_size}")
print(f"Workers per directory: {[source.workers for source in plan.inputs]}")
if args.prepare_only:
    print(f"Reader plan: {plan.path(param.SIGNATURES_PATH)}")
    sys.exit(0)

for fold, worker_rank in enumerate(plan.ranks_for_worker(args.task_id, launch_world_size)):
    dir_idx, files = plan.shard(worker_rank)
    reader = JsonlReader(
        data_folder=plan.inputs[dir_idx].path,
        files=files,
        adapter=adapter,
        compression="infer",
        default_metadata={"data_rank": dir_idx},
    )
    print(f"Worker {args.task_id}, round {fold + 1}: logical rank {worker_rank}, directory {dir_idx}, files={len(files)}")
    plan.begin_rank(param.SIGNATURES_PATH, worker_rank)
    SignatureStage(
        output_folder=param.SIGNATURES_PATH, config=param.minhash_config, language="eng"
    ).run(data=reader.run(), rank=worker_rank, world_size=plan.world_size)
    plan.complete_rank(param.SIGNATURES_PATH, worker_rank)
    print(f"Logical rank {worker_rank} completed stage 1")

print(f"Worker {args.task_id} completed stage 1")
