#!/bin/bash
set -euo pipefail

# Submit the five MinHash stages to Slurm with afterok dependencies, skipping
# leading stages whose completion is already verified. Configure with:
#
#   export S3_INPUT='["/data/high_quality", "/data/general_web"]'  # priority order
#   export DATA_LABEL='["high_quality", "general_web"]'
#   export OUTPUT_PATH=/scratch/dedup_run
#   export TASK_SIZE=2048          # logical world_size shared by stages 1 and 4
#   export MINHASH_NUM_BUCKETS=12  # optional overrides of scripts/dedup_params.py
#   export MINHASH_HASHES_PER_BUCKET=14
#   export MINHASH_SEED=4
#   export MINHASH_ANALYZE_CLUSTERING=false  # exhaustive pairwise diagnostics
#   export EXCLUDE_NODES=""                  # optional sbatch --exclude list
#   ./minhash_driver.sh
#
# Slurm site settings can be supplied via SBATCH_PARTITION, SBATCH_QOS, etc.

# Stage 2.5 is launched with one node per band; keep in sync with dedup_params.py.
export MINHASH_NUM_BUCKETS="${MINHASH_NUM_BUCKETS:-14}"

missing_vars=()
[[ -z "${S3_INPUT:-}" ]] && missing_vars+=("S3_INPUT")
[[ -z "${OUTPUT_PATH:-}" ]] && missing_vars+=("OUTPUT_PATH")
[[ -z "${DATA_LABEL:-}" ]] && missing_vars+=("DATA_LABEL")

if [[ "${#missing_vars[@]}" -gt 0 ]]; then
  echo "Missing required environment variables: ${missing_vars[*]}" >&2
  exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
log_dir="${OUTPUT_PATH}/log"
mkdir -p "${log_dir}"
stages=(
  "minhash_s1.sh"
  "minhash_s2.sh"
  "minhash_s2.5.sh"
  "minhash_s3.sh"
  "minhash_s4.sh"
)

stage_is_complete() {
  local stage="$1"
  case "${stage}" in
    minhash_s1.sh)
      python3 "${script_dir}/minhash_stage1.py" --check-complete >/dev/null 2>&1
      ;;
    minhash_s2.sh)
      python3 "${script_dir}/minhash_stage_state.py" check 2 >/dev/null 2>&1
      ;;
    minhash_s2.5.sh)
      python3 "${script_dir}/minhash_stage_state.py" check 2.5 >/dev/null 2>&1
      ;;
    minhash_s3.sh)
      python3 "${script_dir}/minhash_stage_state.py" check 3 >/dev/null 2>&1
      ;;
    minhash_s4.sh)
      python3 "${script_dir}/minhash_stage_state.py" check 4 >/dev/null 2>&1
      ;;
    *)
      return 1
      ;;
  esac
}

prev_job_id=""
for stage in "${stages[@]}"; do
  stage_path="${script_dir}/${stage}"
  if [[ ! -r "${stage_path}" ]]; then
    echo "Stage script not found or unreadable: ${stage_path}" >&2
    exit 1
  fi
  stage_base="${stage%.sh}"

  if [[ -z "${prev_job_id}" ]] && stage_is_complete "${stage}"; then
    echo "Skipping ${stage}; completion verified."
    continue
  fi

  sbatch_args=(sbatch --parsable --export=ALL)
  if [[ "${stage}" == "minhash_s2.5.sh" ]]; then
    if [[ -z "${MINHASH_NUM_BUCKETS:-}" ]]; then
      echo "MINHASH_NUM_BUCKETS must be set for ${stage}" >&2
      exit 1
    fi
    if ! [[ "${MINHASH_NUM_BUCKETS}" =~ ^[0-9]+$ ]] || [[ "${MINHASH_NUM_BUCKETS}" -lt 1 ]]; then
      echo "MINHASH_NUM_BUCKETS must be a positive integer for ${stage}; got '${MINHASH_NUM_BUCKETS}'" >&2
      exit 1
    fi
    sbatch_args+=("--nodes=${MINHASH_NUM_BUCKETS}")
  fi
  if [[ -n "${EXCLUDE_NODES:-}" ]]; then
    sbatch_args+=("--exclude=${EXCLUDE_NODES}")
  fi
  if [[ -n "${prev_job_id}" ]]; then
    sbatch_args+=("--dependency=afterok:${prev_job_id}")
  fi
  sbatch_args+=("--output=${log_dir}/%A_${stage_base}.out")

  job_id=$(
    S3_INPUT="${S3_INPUT}" OUTPUT_PATH="${OUTPUT_PATH}" DATA_LABEL="${DATA_LABEL}" \
      "${sbatch_args[@]}" "${stage_path}"
  )
  echo "Submitted ${stage} as job ${job_id}"
  prev_job_id="${job_id}"
done
