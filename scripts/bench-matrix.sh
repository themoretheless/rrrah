#!/usr/bin/env bash
# Reproducible latency harness for the full-RAW path.
#
# Usage:
#   BENCH_REPS=9 scripts/bench-matrix.sh corpus/a.cr2 corpus/b.dng
#
# The script intentionally measures the real process boundary. It never uses an
# embedded JPEG preview. Results are CSV so they can be loaded by R/Python.
set -euo pipefail

root_dir="$(cd "$(dirname "$0")/.." && pwd)"
binary="${RRAH_BIN:-$root_dir/target/release/rrrah}"
reps="${BENCH_REPS:-5}"
out="${BENCH_OUT:-$root_dir/target/bench/results.csv}"

if [[ $# -eq 0 ]]; then
  echo "usage: BENCH_REPS=9 $0 FILE.CR2|FILE.DNG [...]" >&2
  exit 2
fi
if [[ ! -x "$binary" ]]; then
  cargo build --release --locked --manifest-path "$root_dir/Cargo.toml" >/dev/null
fi
mkdir -p "$(dirname "$out")"
printf 'timestamp,fixture,mode,iteration,real_seconds,status\n' >"$out"

measure() {
  local fixture="$1" mode="$2" iteration="$3" cache_dir="$4"
  local timing status real
  timing="$(mktemp /tmp/rrrah-time.XXXXXX)"
  set +e
  if [[ "$mode" == "cold-no-cache" ]]; then
    /usr/bin/time -p "$binary" --inspect --no-cache "$fixture" >/dev/null 2>"$timing"
    status=$?
  else
    /usr/bin/time -p "$binary" --inspect --cache-dir "$cache_dir" "$fixture" >/dev/null 2>"$timing"
    status=$?
  fi
  set -e
  real="$(awk '$1 == "real" { print $2 }' "$timing" | tail -1)"
  rm -f "$timing"
  [[ -n "$real" ]] || real="NaN"
  # Quote the fixture as RFC-4180 CSV (paths may contain commas or spaces).
  local escaped_fixture="${fixture//\"/\"\"}"
  printf '%s,"%s",%s,%s,%s,%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$escaped_fixture" "$mode" "$iteration" "$real" "$status" >>"$out"
}

for fixture in "$@"; do
  [[ -f "$fixture" ]] || { echo "fixture is not a file: $fixture" >&2; exit 2; }
  cache_dir="$(mktemp -d /tmp/rrrah-bench-cache.XXXXXX)"
  trap 'rm -rf "$cache_dir"' EXIT

  for ((i = 1; i <= reps; i++)); do
    measure "$fixture" cold-no-cache "$i" "$cache_dir"
  done

  # Seed the persistent decoded-mosaic cache once, then measure warm opens.
  "$binary" --inspect --cache-dir "$cache_dir" "$fixture" >/dev/null
  for ((i = 1; i <= reps; i++)); do
    measure "$fixture" warm-cache "$i" "$cache_dir"
  done
  rm -rf "$cache_dir"
  trap - EXIT
done

echo "results: $out"
