#!/bin/bash
set -euo pipefail

# Run all five stages sequentially on one machine, without Slurm. Intended for
# small corpora and for trying the pipeline; use minhash_driver.sh at scale.
#
#   export S3_INPUT='["/data/high_quality", "/data/general_web"]'  # priority order
#   export DATA_LABEL='["high_quality", "general_web"]'
#   export OUTPUT_PATH=/tmp/dedup_run
#   scripts/run_local.sh

: "${S3_INPUT:?set S3_INPUT}" "${DATA_LABEL:?set DATA_LABEL}" "${OUTPUT_PATH:?set OUTPUT_PATH}"
export TASK_SIZE="${TASK_SIZE:-1}"
bands="${MINHASH_NUM_BUCKETS:-14}"  # must match dedup_params.py when not overridden
python="${PYTHON:-python3}"
cd "$(dirname "${BASH_SOURCE[0]}")"

for ((rank = 0; rank < TASK_SIZE; rank++)); do
  SLURM_NTASKS="${TASK_SIZE}" "${python}" minhash_stage1.py --task-id "${rank}"
done
for stage in 2 2.5; do
  for ((rank = 0; rank < bands; rank++)); do
    SLURM_NTASKS="${bands}" "${python}" "minhash_stage${stage}.py" --task-id "${rank}"
  done
done
"${python}" minhash_stage3.py
for ((rank = 0; rank < TASK_SIZE; rank++)); do
  SLURM_NTASKS="${TASK_SIZE}" "${python}" minhash_stage4.py --task-id "${rank}"
done
echo "Retained documents: ${OUTPUT_PATH}/deduped  Removed documents: ${OUTPUT_PATH}/removed"
