#!/bin/bash
#SBATCH --job-name=minhash4
#SBATCH --nodes=8
#SBATCH --ntasks-per-node=8  # concurrent workers, independent of logical world_size
#SBATCH --mem=0
#SBATCH --time=120:00:00
# Site settings (partition, QoS, account, excluded nodes) are intentionally not
# hard-coded: pass them on the sbatch command line or via SBATCH_PARTITION,
# SBATCH_QOS, SBATCH_ACCOUNT, and SBATCH_EXCLUDE.

set -eo pipefail
# MINHASH_DEDUP_DIR: repository checkout (default ~/minhash_dedup).
# MINHASH_CONDA_ENV: conda environment to activate; set empty to skip conda.
export MINHASH_DEDUP_DIR="${MINHASH_DEDUP_DIR:-$HOME/minhash_dedup}"
export MINHASH_CONDA_ENV="${MINHASH_CONDA_ENV-minhash-dedup}"
activate_env() {
  if [[ -n "${MINHASH_CONDA_ENV}" ]]; then
    source "$(conda info --base)/etc/profile.d/conda.sh"
    conda activate "${MINHASH_CONDA_ENV}"
  fi
  cd "${MINHASH_DEDUP_DIR}/scripts" || exit 1
}
export -f activate_env
activate_env
# Stage 4 loads Stage 1's reader plan. Optional TASK_SIZE is a consistency check.
python3 minhash_stage_state.py begin 4
echo "Starting stage 4"
srun --kill-on-bad-exit=1 bash -ec '
  activate_env
  echo "Task $SLURM_PROCID on $(hostname)"
  python3 minhash_stage4.py --task-id "$SLURM_PROCID"
'
python3 minhash_stage_state.py complete 4
