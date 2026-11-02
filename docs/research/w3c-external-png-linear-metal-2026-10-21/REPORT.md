# External W3C full-range PNG: linear oracle and Metal transport

Follow-up to the original W3C PNG conformance-source recovery. Uses the retained, hash-pinned 1920x1080 RGB16 full-range HLG and PQ attachments from https://lists.w3.org/Archives/Public/public-png/2023Dec/0031.html . The external narrow-HLG attachment remains explicitly unsupported for display conversion and is not relabeled.

The independent generator starts from FFmpeg native RGB48 codes, verifies their recorded hashes and calculates linear sRGB using Decimal at precision 70. BT.2020 and sRGB matrices are derived from xy primaries/D65, not read from production constants. PQ uses rational ST2084 constants with reference white 100 cd/m². HLG applies inverse OETF and luminance-dependent OOTF with zero black, peak 1000 cd/m², gamma 1.2 and reference white 100 cd/m². The generator caches unique codes/RGB combinations; all 2,073,600 pixels per source are written to the external float32 oracle. Independent matrix algebra is reused from the authored-fixture generator by selecting only its function definitions, without executing that PNG producer.

Required Apple M4 Max Metal test passed (1 passed / 0 ignored / 0 failed in 4.34s excluding compilation). All 4,147,200 linear pixels match the independent Decimal result within relative 1e-5 with a unit floor. Observed maximum absolute difference: HLG 0.00000095367431640625; PQ zero after float32 rounding. This is numerical agreement, not a speed benchmark.

Each full-resolution managed raster passes RAM pin/lease protection, eviction to byte-bounded/count-one queued swap, full shared-root refusal, cancellation after restore allocation and successful retry. Every native float bit is exact after restore, and final managed root usage is zero with one successful swap read/write and zero errors per source.

At a 256x144 output, complete SDR readbacks against the independent reference raster differ by at most one byte level; complete float16 HDR reference readbacks agree within 0.2% with floor 0.01. Actual before/after-swap SDR and HDR attachments match exactly. This checks GPU rendering of the external source against independently calculated input, and transport invariance. It does not establish a fully independent rasterization oracle, full-resolution display output, physical HDR presentation, narrow HLG or NVIDIA. Oracle vectors/readback/expected buffers are test-owned and outside the production managed root; root-zero refers to pipeline-owned allocations.

Reproduce after fetching/retaining verified originals (the source-representation instability from the previous report still applies):

    python3 scripts/qualify-w3c-png-linear-reference.py /absolute/fixture-directory
    RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 RRRAH_W3C_PNG_DIR=/absolute/fixture-directory cargo +stable test -p rrrah --test w3c_png_conformance external_w3c_linear_hdr --locked --offline -- --include-ignored --nocapture --test-threads=1

Oracle hashes, generator/source hashes and terminal test output are retained alongside this report. The execution started on 2026-10-21 and was confirmed terminal on 2026-10-22; no job was restarted merely because its original session handle had expired.

Follow-up 2026-10-22: native narrow-HLG PNG conversion and the retained external W3C narrow source are now qualified in ../png-narrow-hlg-2026-10-22/REPORT.md. Earlier refusal/pending statements above describe this report's original revision.

## Full-HD output requalification — 2026-10-24

The required current-code Metal gate now renders 1920×1080 attachments instead of 256×144. It includes all three original W3C RGB16 inputs: full-range HLG, narrow-range HLG and full-range PQ. All three original PNG SHA-256 pins, three native RGB48 oracle pins and three Decimal float oracle pins were revalidated before recording results. No source, metadata or oracle was regenerated or repaired.

Apple M4 Max Metal: 2 passed / 0 failed / 0 ignored, runtime 5.91 s (`metal-current.log`). Every native RGB16 source pixel matches the FFmpeg oracle. All 6,220,800 prepared linear pixels match the independent Decimal70 oracle within the existing tolerance; maximum absolute differences: HLG FR 9.5367431640625e-7, HLG NR 1.9073486328125e-6, PQ zero. Complete Full-HD SDR attachments agree within one code and float16 HDR within 0.2% (floor 0.01) with independently calculated input rendered through the same GPU rasterizer. SDR/HDR attachments before versus after swap are bit-exact for each source. RAM lease/pin protection, shared-root refusal, cancellation after restore admission and retry pass; each source ends at root=0 and one swap read/write with zero errors.

This extends attachment resolution and revalidates current code after wgpu changes. It does not provide an independent rasterizer, physical HDR display evidence, full-resolution 1:1 source sampling, NVIDIA execution or performance measurements. Readback, golden and oracle buffers remain test-owned outside the production managed root. Earlier lower-resolution logs remain historical evidence, not an additional current gate. Reproduction command above now executes the Full-HD output path; omit the test-name filter to run both tests.

## Direct per-pixel HDR oracle at 1:1 — 2026-10-24

The current gate additionally cancels the viewer's 32-pixel fit margin with `zoom=1/fit`, asserting the effective f32 scale equals exactly 1. Full 1920×1080 HDR attachments are compared channel-by-channel directly against the independent Decimal-derived linear source raster, not only against a second rendering through the same GPU shader. Exact output and reference lengths are asserted. This covers source pixel centers, borders, placement and retained signed/HDR values at 1:1; the prior GPU-reference-render comparison remains an additional control. Direct float16 agreement uses the existing 0.2% tolerance with floor 0.01.

Required Apple M4 Max Metal run: 2 passed / 0 failed / 0 ignored, runtime 3.59 s; current raw log replaces the superseded Full-HD fit-mode success log in `metal-current.log`. All three native/linear source checks, managed pressure/cancellation/retry, zero final managed credit and bit-exact SDR/HDR before/after-swap checks also pass. The earlier statement excluding 1:1 source sampling describes the preceding revision and is superseded here. Physical HDR presentation, independent SDR tone/transfer oracle, NVIDIA and timing remain separate.

## Direct per-pixel SDR encoding oracle — 2026-10-24

The current 1:1 gate now compares every SDR attachment channel directly with an f64 evaluation of the [W3C sRGB encoding specification](https://www.w3.org/Graphics/Color/srgb), starting from the independent Decimal-derived linear source pixels. RGB is clipped to normalized attachment range, encoded with the specified piecewise transfer and rounded to RGB8; opaque alpha must be 255. Exact vector lengths are required and error is limited to one code. This is an independent direct SDR oracle, in addition to the retained direct float16 HDR comparison and second-render controls; it supersedes the preceding note that a direct SDR transfer oracle was missing for these sources. It does not qualify tone mapping, transparent composition or physical presentation generally.

Required actual Metal Apple M4 Max: 2 passed / 0 failed / 0 ignored, 3.52 s runtime. All three sources pass; observed SDR code errors are reported individually in `metal-current.log`. HDR and full-source numerical checks, bit-exact before/after-swap attachments, lease/pin pressure, cancellation/retry and final root=0 all remain enabled. The current successful log replaces the preceding equivalent success log. No speed claim follows from these functional runtimes.
