#!/bin/sh
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mode=$1
mount_arg=$2
folder=$3
docker run --rm --user 65534:65534 --tmpfs /tmp:rw,exec,size=2147483648,mode=1777 \
 -v "$project_dir:/workspace:ro" \
 -v /Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native:/validation \
 "$mount_arg" "$folder" -w /workspace \
 -e RUSTUP_TOOLCHAIN=1.98.0 -e CARGO_HOME=/validation/cargo \
 -e CARGO_TARGET_DIR=/validation/cache-storage-target \
 -e CARGO_HTTP_CAINFO=/validation/ca-bundle.pem \
 rust:1-bookworm cargo run -p rrrah-dedup --no-default-features --example cache_storage_probe -- "$mode" /probe
