#!/usr/bin/env bash
set -euo pipefail

raw_path="${1:?usage: bench-raw.sh FILE.CR2|FILE.DNG}"
root_dir="$(cd "$(dirname "$0")/.." && pwd)"
binary="$root_dir/target/release/rrrah"

cargo build --release --locked --manifest-path "$root_dir/Cargo.toml" >/dev/null
cache_dir="$(mktemp -d /tmp/rrrah-bench-cache.XXXXXX)"
trap 'rm -rf "$cache_dir"' EXIT

echo "== cold full RAW decode =="
/usr/bin/time -p "$binary" --inspect --no-cache "$raw_path"
echo "== cold decode + persistent cache write =="
/usr/bin/time -p "$binary" --inspect --cache-dir "$cache_dir" "$raw_path"
echo "== warm persistent cache =="
/usr/bin/time -p "$binary" --inspect --cache-dir "$cache_dir" "$raw_path"
