import argparse
import os
import sys

_REPO_SRC = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "src"))
if _REPO_SRC not in sys.path:
    sys.path.insert(0, _REPO_SRC)

import dedup_params as param
from minhash_dedup.minhash import PruneStage
from minhash_dedup.runtime import logger


parser = argparse.ArgumentParser(description="Run Minhash Stage 2.5")
parser.add_argument("--task-id", type=int, required=True, help="Task ID (rank) for this worker")
parser.add_argument(
    "--buffer-size",
    type=int,
    default=16 * 1024 * 1024,
    help="Buffered reader size in bytes while scanning .dups files",
)
args = parser.parse_args()

rank = args.task_id
world_size = int(os.environ.get("SLURM_NTASKS", 1))

if rank >= world_size:
    logger.error("rank ({}) is out of bounds for world_size ({})", rank, world_size)
    sys.exit(1)

logger.info("Starting stage 2.5 with rank={}, world_size={}", rank, world_size)

PruneStage(
    input_folder=param.BUCKETS_PATH,
    output_folder=param.BUCKETS_PRUNED_PATH,
    buffer_size_bytes=args.buffer_size,
).run(rank=rank, world_size=world_size)
