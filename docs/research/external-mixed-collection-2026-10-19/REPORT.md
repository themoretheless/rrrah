# Mixed local collection: navigation, prefetch, TTL and swap — 2026-10-19

## Selection and scope

Added an opt-in application test driven by a metadata-only external source
manifest, and a selector that does not filter by rrrah decode success. No source
media was copied into the repository. Swap payloads existed only in the test's
automatically removed temporary directory; retained artifacts are metadata,
hashes, diagnostics and logs.

`select-external-raster-collection.py` enumerated 8144 raster paths in the local
ignored work corpora. After byte/header-size selection and SHA-256 deduplication,
6909 distinct encoded files were available. The fixed sample takes 1024 in a
round-robin over extensions, ordered by content SHA-256. Limits: source <=1 MiB;
known Pillow header dimensions <=1024 each. Unrecognized Pillow formats keep
unknown dimensions and run under the application's 512 MiB managed budget.
Symlinks are excluded. The sample has 12 extensions representing 11 format
families (TIFF has .tif and .tiff aliases).

This is a mixed local source/test collection, not 1024 independent photographs:
619 selected files are video-frame assets, 272 come from a frozen rrrah baseline
fixture tree, 120 from another repository source tree, and 13 from miscellaneous
repository/dependency assets. Encoded files are content-unique, but some decoded
samples coincide: the 962 accepted sources have 886 distinct prepared-frame
fingerprints, including samples and frame metadata. See `source-groups.json` and
`selection-metadata.json`; absolute source paths are omitted from retained data.

## Result and refusal boundaries

With strict application color defaults, 962 files were displayed and all 962 were
eligible for the bounded navigation test. Accepted extensions: AVIF40, BMP3,
EXR31, GIF5, HDR6, HEIC6, JPEG112, JXL3, PNG691, TIFF54, WebP11.

62 were refused without managed-memory retention. `refusals.json` records every
reason and count. These are not all malformed images: many lack qualified source
primaries/profile data under strict defaults, and some expose supported-format
feature gaps. The AVIF 2:1 and zero-aspect controls are both nonconforming:
[AVIF 1.2 section 9.1.2](https://aomediacodec.github.io/av1-avif/v1.2.0.html)
requires a present `pasp` to signal 1:1. The earlier report incorrectly classified
2:1 as a feature gap; that conclusion is withdrawn. The current refusal is
correct. EXR associated-alpha zero-alpha emission remains unsupported and is a
separate valid-source feature gap. The test does not count any refused case as
working-format proof.

## Transport qualification

For each accepted source a fresh display load supplies a reference fingerprint.
The fingerprint covers prepared sample bits, geometry, color description,
selection, rational timing, sample scale, pixel aspect and hotspot. It is a
same-decoder transport control, not an independent codec/color oracle. Assumed
color warning flags are checked separately during foreground, prefetch and swap
restore.

The test navigates all 962 accepted paths, alternating between the beginning and
end of the list, with previous/next prefetch (behind1/ahead2). RAM limits are
64 MiB and two entries; visible and nearest leases are protected. It verifies:

- 959 expected farther-prefetch count refusals while retaining nearest/visible;
- each visible RAM hit succeeds without decode while the remaining managed root
  is deliberately reserved; stale generations cannot change cache membership;
- 61 zero-TTL expirations, full-root typed restore refusals, and successful swap
  retries with no decoder fallback and unchanged samples/metadata/warning flags;
- swap limits: 64 MiB/eight objects, queue32 MiB/two waiting writes,
  restore32 MiB; observed disk charge 26035712 bytes/eight objects;
- 962 completed writes, 1878 reads, zero dropped writes and zero swap errors;
- final queue and managed root usage zero after worker shutdown/cache teardown.

TTL is fixed at insertion. An initial harness attempt incorrectly expected a
policy update to rewrite existing deadlines; the final test reinserts the held
frame under zero TTL, matching the documented contract. No production cache
behavior was changed in this milestone.

Count limits and zero-TTL expiry are saturated in this workload; RAM, queue and
disk byte caps are checked but not naturally saturated by the selected images.
The full-root pressure is a separate deliberate reservation.

Baseline decode peak: 11711760 bytes. Maximum observed usage immediately before
synthetic pressure: 11366400 bytes. The reported root high water of 536870912
bytes includes deliberate reservation-only pressure; it is **not** measured
physical RSS or a natural workload allocation peak.

Final opt-in test passed in 8.17 seconds, excluding selection, compilation and
process startup. OS cache state was uncontrolled; no cold-disk throughput,
paired speedup or physical slow-disk claim is made. The ordinary app suite also
passed: 288 tests /82 ignored. The opt-in corpus test is additional. Formatting
and source whitespace checks passed; an existing upstream lifetime-syntax warning
remains. See `qualification.log`, `app-suite.log`, `result.json` and `format.log`.

## Reproduction

Use Python with Pillow and rg available:

```
python3 scripts/select-external-raster-collection.py .dedup-work \
  --count 1024 --output /tmp/rrrah-external-mixed-collection/manifest-1024.json
RRRAH_COLLECTION_MANIFEST=/tmp/rrrah-external-mixed-collection/manifest-1024.json \
RRRAH_COLLECTION_REPORT=/tmp/rrrah-external-mixed-collection/report.json \
CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 \
  cargo +stable test -p rrrah --bin rrrah external_mixed_collection_preserves \
  --locked --offline -- --ignored --nocapture
```

This milestone runs CPU preparation/cache transport; it adds no new Metal,
physical HDR or NVIDIA proof. Full-resolution large-image libraries, physical
slow disks, hours-long live GUI endurance, independent appearance correctness,
remaining format/metadata cases and the overall 100-format/GPU goal remain open.
