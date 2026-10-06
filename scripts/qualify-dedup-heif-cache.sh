#!/bin/sh
# A persisted fingerprint must not hide the loss of the decoder plugin.
set -eu
validation_dir=${1:-/Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native}
sysroot_dir=${2:-/opt/rrrah-dedup-validation/sysroot}
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output_dir=${3:-$project_dir/docs/research}
mkdir -p "$output_dir"
sh "$project_dir/scripts/run-dedup-linux-native.sh" "$validation_dir" "$sysroot_dir" heif-cache > "$output_dir/dedup-heif-cache-present.log" 2>&1
outcome=0
docker run --rm --user 65534:65534 \
  -v "$project_dir:/workspace:ro" -v "$validation_dir:/validation" \
  -v "$sysroot_dir:/validation/sysroot:ro" \
  -e LD_LIBRARY_PATH=/validation/sysroot/usr/lib/aarch64-linux-gnu:/validation/sysroot/usr/lib/llvm-18/lib \
  -e LIBHEIF_PLUGIN_PATH=/no-plugins ubuntu:24.04 \
  /validation/target/debug/examples/font_cache_probe \
  /workspace/tests/fixtures/raster/profiled.heic /validation/font-cache-fixtures/heif-cache.bin \
  > "$output_dir/dedup-heif-cache-absent.log" 2>&1 || outcome=$?
if [ "$outcome" -eq 0 ]; then
  echo 'Expected an explicit unavailable-decoder error.' >&2
  exit 1
fi
if rg -q 'cached_result_hit=true' "$output_dir/dedup-heif-cache-absent.log"; then
  echo 'FAIL: persisted cache hid the missing decoder plugin.' >&2
  exit 1
fi
if ! rg -q 'InvalidHeif' "$output_dir/dedup-heif-cache-absent.log"; then
  echo 'Unexpected failure; decoder refusal was not established.' >&2
  exit 1
fi
echo 'PASS: missing decoder was reported without cached evidence.'
