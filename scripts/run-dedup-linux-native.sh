#!/bin/sh
# Uses the isolated Rust/sysroot environment prepared for Linux validation.
set -eu
validation_dir=${1:-/Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native}
sysroot_dir=${2:-/opt/rrrah-dedup-validation/sysroot}
mode=${3:-targeted}
source_dir_override=${4:-}
test_tmpdir=/tmp
feature_flag=--all-features
cargo_command=test
package=rrrah-dedup
target_dir=/validation/target
persistent_dir=$(dirname -- "$sysroot_dir")/cache-tests
case "$mode" in
  heif-cache) set -- --example font_cache_probe -- /workspace/tests/fixtures/raster/profiled.heic /validation/font-cache-fixtures/heif-cache.bin; cargo_command=run ;;
  heif-unit) set -- --lib heif::tests; package=rrrah-decode ;;
  raster-corpus) set -- --lib raster_corpus; package=rrrah-decode ;;
  raw-cancellation) set -- --lib camtiff::raf::tests::honours_cancellation_per_row; package=rrrah-decode ;;
  raw-raf) set -- --lib camtiff::raf::tests; package=rrrah-decode ;;
  raw-camera) set -- --lib camtiff -- --nocapture; package=rrrah-decode ;;
  raw-core) set -- --all-targets; package=rrrah-core ;;
  projective) set -- --test projective_geometry --test warp; feature_flag=--no-default-features ;;
  periodic-capture) set -- --test periodic_capture; feature_flag=--no-default-features ;;
  projective-regions) set -- --test projective_geometry --test warp --test photometric --test filtered; feature_flag=--no-default-features ;;
  regional-reuse) set -- --lib --test projective_files --test projective_geometry --test warp --test photometric --test filtered ;;
  minimal-full) set -- --all-targets; feature_flag=--no-default-features ;;
  projective-files) set -- --test projective_files --test local_scan ;;
  pyramid-files) set -- --test pyramid --test projective_files pyramid ;;
  spatial-pyramid) set -- --test pyramid ;;
  five-collection) set -- --test five_collection --test gradient_collection --test gradient_index --test five_lifecycle ;;
  spatial-gradient) set -- --test gradient --test gradient_interpolated --test gradient_empty --test gradient_collection ;;
  spatial-gradient-files) set -- --test projective_files pyramid_photometric_files_preserve_identity_refusals_and_cancel_retry ;;
  gradient-collection) set -- --test gradient_collection ;;
  gradient-metadata) set -- --lib gradient_id_scratch_exact_limit_cancellation_and_ownership ;;
  six-regions) set -- --lib --test five_collection --test five_region_admission ;;
  six-unique-proposals) set -- --lib --test five_collection --test five_region_admission --test gradient_index ;;
  candidate-union-files) set -- --test candidate_union_files --test correspondence_union --test gradient_candidate_smoothing --test gradient_scale_files ;;
  distinct-managed-files) set -- --test managed_evidence --test gradient_scale_files --test gradient_distinct_locations ;;
  gradient-scales-distinct) set -- --test gradient_scale --test gradient_scale_files --test gradient_distinct_locations --test projective_domains --test projective_geometry --test gradient --test gradient_interpolated ;;
  gradient-index) set -- --test gradient_index ;;
  region-grid) set -- --test region_grid --test region_grid_files ;;
  region-transform) set -- --test region_transform --test region_grid --test region_grid_files ;;
  five-gradient-regions) set -- --test five_collection --test five_region_admission --test five_gradient_region_lifecycle ;;
  five-union-memory) set -- --test five_collection --test five_region_admission ;;
  distinct-variants) set -- --lib --test visual --test local_index ;;
  binary-owner-slots) set -- --lib --test visual --test local_index --test five_collection --test five_region_admission ;;
  five-bidirectional-regions) set -- --lib --test five_collection --test five_region_admission --test local_index --test visual ;;
  five-regions) set -- --test five_collection --test five_region_lifecycle --test region_grid --test region_grid_files ;;
  proposal-lifetime) set -- --test five_collection --test five_region_lifecycle --test five_region_admission --test gradient_collection --test region_grid --test region_grid_files ;;
  gradient-core) set -- --test gradient --test gradient_empty --test gradient_interpolated; feature_flag=--no-default-features ;;
  gradient-pyramid) set -- --test gradient --test gradient_empty --test gradient_interpolated --test pyramid ;;
  five-lifecycle) set -- --test five_lifecycle ;;
  complementary-files) set -- --test projective_files complementary ;;
  projective-collection) set -- --test projective_files indexed_projective_collection ;;
  projective-phase) set -- --test projective_files registration_phase_cancel ;;
  projective-collection-lifecycle) set -- --test projective_files indexed_projective_collection_mutation_cancel_and_limits ;;
  projective-collection-exhaustive) set -- --test projective_files indexed_projective_collection_matches_all_pairs_on_six_source_corpus ;;
  projective-roots) set -- --test projective_files recursive_projective_collection ;;
  projective-spatial) set -- --test projective_files spatial_projective_registration_preserves_identity_and_admission ;;
  projective-spatial-collection) set -- --test projective_files indexed_spatial_projective_matches_direct_pairs ;;
  projective-sampled-files) set -- --test projective_files sampled_projective_files_preserve_pixels_and_terminal_refusals ;;
  projective-sampled-exhaustive) set -- --test projective_files sampled_projective_collection_matches_all_pairs_on_six_source_corpus ;;
  projective-sampled-lifecycle) set -- --test projective_files sampled_projective_collection_mutation_cancel_and_limits ;;
  projective-photometric-lifecycle) set -- --test projective_files projective_photometric_collection_mutation_cancel_and_limits ;;
  projective-light-photos) set -- --test projective_files combined_perspective_light_photos_require_all_six_positives_and_unrelated_rejection ;;
  projective-portfolio-lifecycle) set -- --test projective_files registration_portfolio_collection_mutation_cancel_and_limits ;;
  projective-portfolio-exhaustive) set -- --test projective_files registration_portfolio_collection_matches_all_pairs_on_six_source_corpus ;;
  projective-portfolio-files) set -- --test projective_files registration_portfolio_files_preserve_both_residuals_and_atomic_refusals ;;
  projective-photometric-files) set -- --test projective_files projective_photometric_file_fit_keeps_strict_errors_and_refusals ;;
  projective-anchored) set -- --test projective_files anchored_projective_file_objective_never_bypasses_pixel_acceptance ;;
  projective-anchored-exhaustive) set -- --test projective_files anchored_projective_collection_matches_all_pairs_on_six_source_corpus ;;
  projective-anchored-lifecycle) set -- --test projective_files anchored_projective_collection_mutation_cancel_and_limits ;;
  raw-fixtures)
    set -- --lib camtiff::fixture_regression -- --nocapture; package=rrrah-decode
    export RRRAH_CR2_FIXTURE=/raw-corpus/1294.cr2 RRRAH_NEF_FIXTURE=/raw-corpus/898.nef
    export RRRAH_ARW_FIXTURE=/raw-corpus/2414.arw RRRAH_ORF_FIXTURE=/raw-corpus/1084.orf
    export RRRAH_PEF_FIXTURE=/raw-corpus/831.pef RRRAH_RW2_FIXTURE=/raw-corpus/5025.rw2
    export RRRAH_RAF_FIXTURE=/raw-corpus/2883.raf ;;

  decode-cache) set -- --test decode ;;
  color-policy) set -- --test decode --test raster_equality ;;
  wal-dependency) set -- --test wal_dependency ;;
  heldout-photo) set -- --test photos_heldout ;;
  heldout-collection) set -- --test photos_heldout heldout_indexed_crop_collection ;;
  nested-visual) set -- --test scan nested_visual_roots_preserve_all_copy_edges_and_corrupt_file_attribution ;;
  raw-build) set -- --example raw_probe; cargo_command=build ;;
  targeted) set -- --test decode --test container --test presentation_digest ;;
  raster-equality) set -- --test raster_equality ;;
  jpeg-orientation) set -- --test jpeg_orientation ;;
  animation) set -- --test animated ;;
  exact-policy) set -- --lib --test exact_files; feature_flag=--no-default-features; target_dir=/validation/exact-policy-target ;;
  exact) set -- --lib --test exact_files; feature_flag=--no-default-features ;;
  font-cache) set -- --example font_cache_probe -- /validation/font-cache-fixtures/source.svg /validation/font-cache-fixtures/cache.bin; cargo_command=run ;;
  alpha-memory) set -- --example alpha_memory_scale_probe -- /validation/alphafixtures; cargo_command=run ;;
  raster-png) set -- --example raster_memory_scale_probe -- /validation/pngfixtures; cargo_command=run ;;
  raster-memory) set -- --example raster_memory_scale_probe; cargo_command=run ;;
  exact-scale) set -- --example exact_scale_probe; feature_flag=--no-default-features; cargo_command=run ;;
  exact-cancel-scale) set -- --example exact_cancel_scale_probe; feature_flag=--no-default-features; cargo_command=run ;;
  full) set -- --all-targets ;;
  cache) set -- --lib --test cache ;;
  cache-disk) set -- --test cache; test_tmpdir=/persist ;;
  *) echo "Expected targeted, raster-equality, animation, exact, exact-policy, exact-scale, exact-cancel-scale, raster-memory, raster-png, alpha-memory, full, cache or cache-disk validation mode" >&2; exit 2 ;;
esac
if [ -n "$source_dir_override" ]; then
  project_dir=$(CDPATH= cd -- "$source_dir_override" && pwd)
else
  project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
fi
test -f "$project_dir/Cargo.toml"
exec docker run --rm --user 65534:65534 \
  --tmpfs /tmp:rw,exec,size=2147483648,mode=1777 \
  -v "$project_dir:/workspace:ro" -v "$validation_dir:/validation" \
  -v "$sysroot_dir:/validation/sysroot:ro" -v "$persistent_dir:/persist" \
  -v /Users/themoretheless/.codex/tmp/rrrah-dedup-raw-independent:/raw-corpus:ro \
  -e RRRAH_CR2_FIXTURE -e RRRAH_NEF_FIXTURE -e RRRAH_ARW_FIXTURE \
  -e RRRAH_ORF_FIXTURE -e RRRAH_PEF_FIXTURE -e RRRAH_RW2_FIXTURE -e RRRAH_RAF_FIXTURE \
  -e "TMPDIR=$test_tmpdir" -w /workspace \
  -e PATH=/validation/toolchain/bin:/validation/sysroot/usr/bin:/usr/bin:/bin \
  -e LD_LIBRARY_PATH=/validation/sysroot/usr/lib/aarch64-linux-gnu:/validation/sysroot/usr/lib/llvm-18/lib \
  -e CARGO_HOME=/validation/cargo -e "CARGO_TARGET_DIR=$target_dir" \
  -e CARGO_BUILD_JOBS=1 \
  -e CARGO_HTTP_CAINFO=/validation/ca-bundle.pem \
  -e RUSTC=/validation/toolchain/bin/rustc \
  -e CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=/validation/sysroot/usr/bin/gcc \
  -e 'RUSTFLAGS=-C link-arg=--sysroot=/validation/sysroot' \
  -e CC=/validation/sysroot/usr/bin/gcc -e CXX=/validation/sysroot/usr/bin/g++ \
  -e AR=/validation/sysroot/usr/bin/ar \
  -e CFLAGS=--sysroot=/validation/sysroot -e CXXFLAGS=--sysroot=/validation/sysroot \
  -e BINDGEN_EXTRA_CLANG_ARGS=--sysroot=/validation/sysroot \
  -e LIBHEIF_PLUGIN_PATH=/validation/sysroot/usr/lib/aarch64-linux-gnu/libheif/plugins \
  -e LIBCLANG_PATH=/validation/sysroot/usr/lib/llvm-18/lib \
  -e PKG_CONFIG_SYSROOT_DIR=/validation/sysroot \
  -e PKG_CONFIG_LIBDIR=/validation/sysroot/usr/lib/aarch64-linux-gnu/pkgconfig:/validation/sysroot/usr/share/pkgconfig \
  ubuntu:24.04 cargo "$cargo_command" -p "$package" "$feature_flag" "$@"
