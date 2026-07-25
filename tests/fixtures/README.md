# RAW reference corpus

Camera files are intentionally not committed to the repository: CR2/DNG
redistribution terms vary and a synthetic TIFF is not a valid RAW regression
fixture. Put licensed files in `tests/fixtures/data/` and list them in
`tests/fixtures/SHA256SUMS` using:

```text
<sha256> <url>
```

The manifest is consumed by `scripts/fetch-fixtures.sh`; it downloads missing
files, verifies SHA-256, and fails closed when `RRRAH_REQUIRE_FIXTURES=1`.

Required reference IDs are documented in
[`docs/DECODE_FORMAT_AUDIT.md`](../../docs/DECODE_FORMAT_AUDIT.md):

- Canon CR2 24 MP and 52 MP;
- CR2 with restart markers and a truncated negative corpus;
- uncompressed/tiled/lossless-JPEG/BigTIFF DNG;
- DNG black-level grid, OpcodeList and floating-point negative cases;
- one real camera file per TIFF-family format for the env-gated regression
  tests: CR2 (`RRRAH_CR2_FIXTURE`), NEF (`RRRAH_NEF_FIXTURE`), ARW
  (`RRRAH_ARW_FIXTURE`), ORF (`RRRAH_ORF_FIXTURE`), PEF
  (`RRRAH_PEF_FIXTURE`), RW2 (`RRRAH_RW2_FIXTURE`) and RAF
  (`RRRAH_RAF_FIXTURE`), plus the negative variants listed in the audit
  (Nikon lossless 34713, ARW 1.0 Huffman, PEF 65535, Fuji X-Trans).

For local runs:

```sh
RRRAH_FIXTURE_MANIFEST=tests/fixtures/SHA256SUMS \
RRRAH_FIXTURE_DIR=tests/fixtures/data \
  scripts/fetch-fixtures.sh
```

Do not use the embedded JPEG preview as an oracle. Each release manifest must
also record licence, camera/model, decoder version and an independent mosaic
oracle digest before enabling the fixture-gated CI job.
