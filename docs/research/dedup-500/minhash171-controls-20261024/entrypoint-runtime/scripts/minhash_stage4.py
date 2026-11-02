import os
import sys
import argparse

_REPO_SRC = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "src"))
if _REPO_SRC not in sys.path:
    sys.path.insert(0, _REPO_SRC)

from minhash_dedup.jsonl import JsonlReader, JsonlWriter
from minhash_dedup.minhash import FilterStage
from minhash_dedup.runtime import Document
from dataclasses import asdict
from minhash_dedup.partition import ReaderPlan
import dedup_params as param

from typing import Dict, Any
import hashlib
from pathlib import Path
from types import MethodType

def reader_adapter(
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
        "id": doc_id,
        "media": media,
        "metadata": metadata,
    }

def writer_adapter(self, doc: Document):
    """
    Convert an internal document

        {"text": ..., "id": ..., "media": [...], "metadata": {...}}

    back into the input JSONL layout

        {"text": "...", "meta": {"id": ..., "media": [...], ...}, "subset": ...}

    where `subset` is taken from metadata (if present) and all other metadata
    keys are kept under `meta`.
    """
    text      = doc.text  # Use attribute access
    unique_id = doc.id    # Use attribute access
    media     = doc.media # Use attribute access
    metadata  = dict(doc.metadata)   # Use attribute access, ensure it's a dict for .pop

    # Extract subset and remove it from metadata copy
    subset = metadata.pop("subset", None)

    # Build the 'meta' block
    meta_block = {
        "id":    unique_id,
        "media": media,
        **metadata,          # the remaining metadata fields
    }

    return {
        "text":   text,
        "meta":   meta_block,
        "subset": subset,
    }


parser = argparse.ArgumentParser(description="Run Minhash Stage 4")
parser.add_argument("--task-id", type=int, required=True, help="Launched worker rank, not a logical reader rank")
parser.add_argument("--task-size", "--task_size", type=int, default=None,
                    help="Optional assertion of logical world_size; Stage 1's reader plan is authoritative")
args = parser.parse_args()
rank = args.task_id
launch_world_size = int(os.environ.get("SLURM_NTASKS", 1))
task_size = args.task_size
if task_size is None and os.environ.get("TASK_SIZE"):
    task_size = int(os.environ["TASK_SIZE"])

plan = ReaderPlan.load(param.SIGNATURES_PATH)
plan.validate(param.S3_INPUT, task_size, asdict(param.minhash_config))
worker_ranks = plan.ranks_for_worker(rank, launch_world_size)
# Missing completion is not an inactive rank, even if no signature files exist.
for worker_rank in worker_ranks:
    plan.require_complete(param.SIGNATURES_PATH, worker_rank)
print(f"Stage 4: logical world_size={plan.world_size}, active ranks={plan.active_ranks}, launched workers={launch_world_size}")
print(f"Workers per directory: {[source.workers for source in plan.inputs]}")
for fold, worker_rank in enumerate(worker_ranks):
    dir_idx, files = plan.shard(worker_rank)
    reader = JsonlReader(
        data_folder=plan.inputs[dir_idx].path,
        files=files,
        adapter=reader_adapter,
        compression="infer",
        default_metadata={"data_rank": dir_idx, "data_label": param.DATA_LABEL[dir_idx]},
    )
    print(f"Worker {rank}, round {fold + 1}: logical rank {worker_rank}, directory {dir_idx}, files={len(files)}")

    # Create the pipeline
    pipeline = [
        FilterStage(                         # Stage 2: Filtering with world_size workers
            input_folder = param.REMOVE_IDS_PATH,
            exclusion_writer=JsonlWriter(            # Writer for removed documents
                output_folder=param.REMOVED_PATH,
                output_filename=f"${{data_label}}/${{data_label}}_{fold}_${{rank}}.jsonl.gz",
                compression="gzip",
                max_file_size=500_000_000,
            )
        ),
        JsonlWriter(                                # Stage 3: Writing with world_size workers
            output_folder=param.DEDUPED_PATH,
            output_filename=f"${{data_label}}/${{data_label}}_{fold}_${{rank}}.jsonl.gz",
            compression="gzip",
            max_file_size=1_000_000_000
        )
    ]

    # Bind writer adapters to both writers
    pipeline[-1].adapter = MethodType(writer_adapter, pipeline[-1])  # Main writer
    pipeline[0].exclusion_writer.adapter = MethodType(writer_adapter, pipeline[0].exclusion_writer)  # Exclusion writer

    data = reader.run()

    # Removal lookup and output names use the logical rank, not the launched worker.
    filtered_count = 0
    for stage in pipeline:
        data = stage.run(data=data, rank=worker_rank, world_size=plan.world_size)
        if isinstance(stage, JsonlWriter):  # Count documents in the final writer stage
            filtered_count = sum(1 for _ in data) if data is not None else 0

    print(f"Worker {worker_rank}: Finished processing {filtered_count} documents")
print(f"Worker {rank} completed stage 4")
