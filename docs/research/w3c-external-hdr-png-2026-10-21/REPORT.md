# External W3C PNG HDR conformance attachments

Recovered public external sources from the PNG Working Group December 2023 archive:
https://lists.w3.org/Archives/Public/public-png/2023Dec/0031.html

The archived original Dropbox collection now returns a File Deleted page. The December archive's 79 messages were inspected for conformance attachments; revision att-0031 provides corrected PNG identity-matrix CICP signaling. Three retained original 1920x1080 RGB16 sources are pinned by SHA-256 in chunks.json and oracle-manifest.json: full/narrow HLG (9/18/0/1 and 9/18/0/0) and full PQ (9/16/0/1). This set contains no narrow-PQ source. External images remain in /Users/themoretheless/.codex/tmp/rrrah-w3c-png-conformance rather than copied into the repository.

FFmpeg 9.0.2 is used only to independently decode native RGB48LE codes, with no transfer/gamut conversion. The ignored opt-in w3c_png_conformance integration test verifies all 6,220,800 native RGB pixels, opaque alpha, 1920x1080 dimensions and exact CICP. Public explicit HDR policies accept full PQ/HLG, retain finite scene-linear HDR samples and alpha, and preserve clone credit until final release. Narrow HLG explicitly refuses its unimplemented display conversion and returns root usage to zero. All cases end at zero managed credit.

Test: 1 passed / 0 ignored / 0 failed in 0.16 seconds excluding compilation (native.log). Full-range float output was checked for policy acceptance/finite HDR/ownership only, not against an independent numerical color oracle. This does not qualify external narrow PQ, new GPU transport, physical HDR or all PNG variants.

Reproduction:

    python3 scripts/fetch-w3c-hdr-png-conformance.py /absolute/fixture-directory
    RRRAH_W3C_PNG_DIR=/absolute/fixture-directory cargo +stable test -p rrrah --test w3c_png_conformance --locked --offline -- --include-ignored --nocapture

Important observed source instability: a repeated fetch of the full-range HLG attachment returned an 88,307-byte PNG with IHDR/IDAT/IEND only, replacing the original 133,077-byte pHYs/cICP-bearing representation. FFmpeg native RGB48 remained bit-identical. repeated-fetch.json preserves both hashes and decoded equivalence. No cause is inferred. The helper prefers an existing verified original and fails closed on any downloaded hash mismatch; it never silently substitutes metadata or relabels a stripped file. A clean-machine fetch may therefore fail until the pinned original representation is available. Retained originals and oracles remain valid locally.

Follow-up 2026-10-22: native narrow-HLG PNG conversion and the retained external W3C narrow source are now qualified in ../png-narrow-hlg-2026-10-22/REPORT.md. Earlier refusal/pending statements above describe this report's original revision.
