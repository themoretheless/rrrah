# PNG narrow-range PQ RGB

PNG3 cICP permits narrow-range RGB and recommends the sign-preserving extension `sign(x) * transfer(abs(x))` for undershoot/overshoot. Source: https://www.w3.org/TR/2025/REC-png-3-20250624/#cICP-chunk . The normative 8-bit RGB legal interval 16..235 scales by 256 for 16-bit storage; alpha stays full-range.

The public PNG explicit PQ policy previously refused narrow-range cICP 9/16/0/0. It now invokes a dedicated integer-code RGB conversion. Range expansion happens inside the PQ transfer evaluation before the BT.2020-to-linear-sRGB matrix. No intermediate image allocation, clipping or inferred display-white policy is introduced. Both scalar and optional table paths use the same expanded codes. Normalized HDR APIs used by video decoders keep their prior behavior and do not expand range again. Narrow HLG remains refused.

Independent anchors use Python Decimal at precision 70, rational ST2084 constants, signed transfer extension and reference white 100 cd/m². decimal-reference.py reproduces decimal-reference.json byte-for-byte. The Rust regression compares 12 anchors across 8/16-bit neutral ramps, then covers every 8/16-bit code, alpha, monotonicity, undershoot/overshoot, exact scalar/table f32 agreement, one-byte-short refusal, cancellation after admission, exact output-only ownership and zero final root credit. Public PNG metadata/policy test now requires successful narrow PQ conversion and retains narrow HLG refusal.

The initial public focused test passed. This is authored source/color conversion evidence. Actual new PNG RAM/swap/Metal pipeline and physical HDR display qualification remain to be run; no full PNG/all-format completion claim.

Full gates: rrrah-develop 100 passed / 2 ignored / 0 failed; rrrah-decode 232 passed / 5 ignored / 0 failed. Existing full-range PQ/HLG and table/cancellation controls pass. The initial focused test compile diagnostic (missing collect type annotation) is preserved in core.log; corrected full evidence is develop.log. Source hashes and unchanged-reference comparison are recorded. Source whitespace checks pass.

Follow-up 2026-10-22: native narrow-HLG PNG conversion and the retained external W3C narrow source are now qualified in ../png-narrow-hlg-2026-10-22/REPORT.md. Earlier refusal/pending statements above describe this report's original revision.
