"""Manage completion around successful whole-stage Slurm launches."""

import argparse
import os
import sys
from dataclasses import asdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

import dedup_params as param
from minhash_dedup.checkpoints import STAGE_OUTPUTS, StageCheckpoints
from minhash_dedup.io import get_datafolder
from minhash_dedup.partition import ReaderPlan


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("begin", "complete", "check", "invalidate"))
    parser.add_argument("stage", choices=("1", "2", "2.5", "3", "4"))
    args = parser.parse_args()
    if args.action == "invalidate":
        StageCheckpoints(param.OUTPUT_PATH, {}).invalidate(args.stage)
        return
    if args.stage == "1" and args.action != "check":
        parser.error("Stage 1 uses its existing per-logical-rank completion records")

    plan = ReaderPlan.load(param.SIGNATURES_PATH)
    task_size = int(os.environ["TASK_SIZE"]) if os.environ.get("TASK_SIZE") else None
    plan.validate(param.S3_INPUT, task_size, asdict(param.minhash_config))
    completions = []
    for rank in range(plan.active_ranks):
        plan.require_complete(param.SIGNATURES_PATH, rank)
        stat = plan._completion_path(param.SIGNATURES_PATH, rank).stat()
        completions.append([rank, stat.st_size, stat.st_mtime_ns])
    if args.stage == "1":
        return

    checkpoints = StageCheckpoints(
        param.OUTPUT_PATH,
        {"plan": plan.fingerprint, "stage1_completions": completions},
        stage_options={
            "3": {
                "analyze_clustering": os.environ.get("MINHASH_ANALYZE_CLUSTERING", "false").strip().lower()
                in {"1", "true", "yes", "y", "on"}
            },
            "4": {"data_labels": param.DATA_LABEL},
        },
    )
    if args.action == "check":
        checkpoints.require_complete(args.stage)
    elif args.action == "begin":
        for source in plan.inputs:
            folder = get_datafolder(source.path)
            if not folder.is_local():
                continue
            input_path = Path(folder.resolve_paths("")).resolve()
            for name in STAGE_OUTPUTS[args.stage]:
                output_path = checkpoints.root / name
                if input_path == output_path or input_path in output_path.parents or output_path in input_path.parents:
                    raise ValueError(f"Refusing to clear output overlapping source data: {output_path}")
        workers = int(os.environ.get("SLURM_NTASKS", "1"))
        bands = param.minhash_config.num_buckets
        if workers < 1 or (args.stage in {"2", "2.5"} and workers % bands):
            raise ValueError("Stage workers must be positive and stages 2/2.5 require a multiple of band count")
        if args.stage == "3" and workers != 1:
            raise ValueError("Stage 3 requires exactly one worker")
        expected_files = []
        if args.stage == "2":
            expected_files = [
                f"buckets/{band:03d}_{worker:02d}.dups" for band in range(bands) for worker in range(workers // bands)
            ]
        print(f"Stage {args.stage}: starting a fresh attempt; clearing {STAGE_OUTPUTS[args.stage]}")
        checkpoints.begin(args.stage, expected_files)
    else:
        checkpoints.complete(args.stage)


if __name__ == "__main__":
    main()
