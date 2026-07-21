#!/usr/bin/env bash
set -euo pipefail
manifest="${RRRAH_FIXTURE_MANIFEST:-tests/fixtures/SHA256SUMS}"
outdir="${RRRAH_FIXTURE_DIR:-tests/fixtures/data}"
verify_only=0
[[ "${1:-}" == "--verify-only" ]] && verify_only=1
if [[ ! -f "$manifest" ]]; then
  if [[ "${RRRAH_REQUIRE_FIXTURES:-0}" == "1" ]]; then
    echo "fixture manifest required but not present: $manifest" >&2
    exit 2
  fi
  echo "fixture manifest not present: $manifest (skipping; set RRRAH_REQUIRE_FIXTURES=1 to fail)" >&2
  exit 0
fi
mkdir -p "$outdir"
while read -r sha url; do
  [[ -z "${sha:-}" || "${sha:0:1}" == "#" ]] && continue
  file="$outdir/${url##*/}"
  if [[ "$verify_only" -eq 0 && ! -f "$file" ]]; then curl --fail --location --retry 3 --output "$file" "$url"; fi
  [[ -f "$file" ]] || { echo "missing fixture $file"; exit 1; }
  echo "$sha  $file" | shasum -a 256 -c -
done < "$manifest"
