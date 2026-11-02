import sys
import os
import argparse

_REPO_SRC = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "src"))
if _REPO_SRC not in sys.path:
    sys.path.insert(0, _REPO_SRC)

from minhash_dedup.runtime import logger
from minhash_dedup.minhash import BucketStage
import dedup_params as param

# Setup argument parsing
parser = argparse.ArgumentParser(description="Run Minhash Stage 2")
parser.add_argument("--task-id", type=int, required=True, help="Task ID (rank) for this worker")
args = parser.parse_args()

# Get rank from the parsed argument
rank = args.task_id

# Get world_size from SLURM_NTASKS environment variable
world_size = int(os.environ.get("SLURM_NTASKS", 1))

if world_size == 1 and rank != 0:
    logger.warning("world_size is 1 but rank is not 0. This might indicate a configuration issue.")
elif rank >= world_size:
    logger.error(f"rank ({rank}) is out of bounds for world_size ({world_size}).")
    sys.exit(1)

logger.info(f"Starting task with rank={rank}, world_size={world_size}")


stage2_step = BucketStage(
    input_folder = param.SIGNATURES_PATH,
    output_folder = param.BUCKETS_PATH,
    config=param.minhash_config,
)

# Call run WITH rank and world_size
# rank here corresponds to the bucket this task should process
stage2_step.run(rank=rank, world_size=world_size)
