#!/bin/sh
# Execute the already-built native Linux probe against the pinned task corpus.
set -eu
case "$#" in 3|4|5) ;; *) echo 'Expected SOURCE WIDTH HEIGHT [BUDGET_MIB [--verify-failure-retry]]' >&2; exit 2 ;; esac
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
validation_dir=/Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native
raw_corpus=/Users/themoretheless/.codex/tmp/rrrah-dedup-raw-independent
sysroot_dir=/opt/rrrah-dedup-validation/sysroot
if [ ! -x "$validation_dir/target/debug/examples/raw_probe" ]; then
  echo "Native Linux raw_probe executable is missing; run raw-build first" >&2
  exit 2
fi
source_name=$(basename -- "$1")
case "$source_name" in
  IMG_9043.CR3|IMG_9074.CR3) native_source=/workspace/tests/$source_name ;;
  *) native_source=/raw-corpus/$source_name ;;
esac
if [ "$#" -eq 5 ]; then
  [ "$5" = --verify-failure-retry ] || { echo 'Unknown verification flag' >&2; exit 2; }
  set -- "$native_source" "$2" "$3" "$4" "$5"
else
  set -- "$native_source" "$2" "$3" "${4:-2048}"
fi
exec docker run --rm --user 65534:65534 \
  --tmpfs /tmp:rw,exec,size=2147483648,mode=1777 \
  -v "$project_dir:/workspace:ro" -v "$validation_dir:/validation:ro" \
  -v "$sysroot_dir:/validation/sysroot:ro" -v "$raw_corpus:/raw-corpus:ro" \
  -e LD_LIBRARY_PATH=/validation/sysroot/usr/lib/aarch64-linux-gnu:/validation/sysroot/usr/lib/llvm-18/lib \
  -e LIBHEIF_PLUGIN_PATH=/validation/sysroot/usr/lib/aarch64-linux-gnu/libheif/plugins \
  ubuntu:24.04 timeout --kill-after=2s 175s /validation/target/debug/examples/raw_probe "$@"
