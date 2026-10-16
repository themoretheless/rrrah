# Original PSD high-precision composite qualification

Pinned psd-tools commit9ea48dad04a32a6977b3109ebfae23b667590d42 tree supplied six original repository fixtures; inventory.json retains original URLs, SHA256 and header fields. None has merged alpha. MIT license retained. Four4x4 RGB/grayscale16/32bit originals and independent little-endian RGBA plane oracles are imported into tests/fixtures/raster. extract-planes.py independently walks length-prefixed sections and extracts raw composite channels, without calling native Rust or psd-tools.

Initial original32bit fixture decoding failed InvalidPsd(unexpected color mode data). Both32bit4x4 originals have112-byte color-mode data starting hdrt/version3; psd-tools primary documentation identifies hdrt as HDR toning data. Production now admits that specific depth32/112byte/signature-version variant, bounds it using the existing reader, and preserves float samples without applying Photoshop toning. Unknown nonempty variants remain refused.

All four original composites match every independently extracted native sample:16bit exact words,32bit exact float bits, derived opaque alpha. Managed pixels and clone-last-owner release asserted. Full layered library67passed/2ignored. Logs preserve initial failure and passing regression.

This is stored merged-composite precision/ownership qualification only. No16/32bit merged transparency support, layer compositing, arbitrary hdrt variants, Photoshop toning reproduction or complete display color qualification is claimed. Producer application is not independently proven merely by repository membership.

Primary source: https://psd-tools.readthedocs.io/en/latest/reference/psd_tools.api.html (depth32 hdrt note); source fixtures URLs and hashes in inventory.json.


## Public decode / cache / swap / Metal and positive HDR ICC

Added all four original RGB/grayscale16/32bit PSDs to the retained integration corpus, using independent raw-plane and independently extracted ICC resource1039 fixtures. Initial public preparation failed InvalidInput: original floatRGB contains1.000009775 above1 and its three ICC curves encode gamma1. Production now permits positive extended input only for exactly linear RGB/gray curves with no A2B LUTs; nonlinear/LUT profiles remain normalized, negatives/nonfinite values remain refused and alpha stays normalized.

Independent LittleCMS double relative/NOOPTIMIZE oracle for original linearRGB profile at f32 inputs1.00001/2/4 agrees within unchanged2e-5 absolute bound; alpha exact, negative input refused and managed final zero. Initial analytic identity assumption failed because an encoded ICC's fixed-point colorants do not define exact identity relative to the destination; replaced that invalid assumption with the independently generated oracle and retained failed log. Animation32passed/1ignored.

Actual Metal M4 Max expanded corpus2018cases/12108readbacks passed,2018RAM pressure hits/writes/restores with decode blocked and final tracked credit0. CMS expected and actual use the same native preparation in that transport corpus; independent color accuracy is the separate LittleCMS test. Merged alpha, arbitrary nonlinear HDR ICC/negative ICC extrapolation and Photoshop HDR-toning appearance remain open.


HDR toning admission boundary: six mutations of the original32bit PSD (signature, version,111/113byte blocks, integer-depth mismatch, truncated block) all return typed InvalidPsd before managed pixel/ICC allocation, at a zero-byte budget with peak/final zero. Original supported composites remain exact. Layered library68passed/2ignored. Evidence: hdrt-malformed.log.


Independent grayscale HDR extension: original32bit grayscale ICC at float inputs1.00001/2/4/16 compared against test-only LittleCMS double gray→linearRGB oracle. Maximum RGB absolute error1.907349e-6 <= unchanged2e-5 bound; every output stays above1, alpha exact, managed final zero. Retained generator gray-linear-hdr-lcms-oracle.c and binary fixture hash in pipeline-result.json. Animation33passed/1ignored. This verifies this positive linear gray profile; negative/nonlinear HDR ICC remains outside the admitted contract.


Live native HDR follow-up: all four original RGB/grayscale16/32bit PSDs decode, prepare ICC and upload to RGBA16Float/ExtendedSrgbLinear Metal windows successfully. OS HDR active; source credit after upload0; 670 presented frames total, 550 after confirmed resize, no validation errors. Evidence: live-psd-result.json and per-fixture live logs. Probe presentation is not calibrated physical color/luminance, Photoshop HDR toning, merged alpha or full viewer navigation qualification.


Identity-curve HDR follow-up: ICC.1 curveType count0 defines identity (ICC v4.1 section6.5.4). Extended linear input admission now recognizes empty Lut curves as well as one-entry gamma256/parametric gamma1, while absent curves remain unqualified. Two authored profile derivatives replace RGB or gray TRC tags with count0/size12, preserving colorants. Test-only LittleCMS HDR output is byte-identical to the corresponding gamma1 references; both native RGB and gray tests now run both encodings, preserving above-one values/alpha and final zero. Animation33passed/1ignored. Evidence: identity-icc-result.json, identity-icc-hdr.log. Primary source https://www.color.org/icc1-v41.pdf .


Original PSB32 follow-up: pinned psd-tools32bit5x5.psb/version2/RGB32/raw composite imported with independently extracted floatRGBA and ICC1039. Native comparison preserves every float bit and opaque alpha; clones release credit at last owner. Saved stdlib extractor now handles the PSB8byte layer/mask section length; regeneration byte-identical. Source URL/SHA in psb32-source.json. Expanded Metal corpus2019cases/12114readbacks,2019RAM pressure hits/writes/restores, decode blocked during restore, final0. Layered68passed/2ignored. Evidence: psb32-native.log, psb32-metal.log. This original is opaque and does not qualify high-precision merged alpha or Photoshop toning appearance.


Original-source exact admission follow-up: all five RGB/grayscale16/32bit PSD/PSB composites decode at exactly output+ICC bytes; tracked peak equals retained bytes, no raw-plane copy. Every one-byte-short budget returns memory refusal and releases all credit. ICC bytes independently extracted from resource1039 match; last-owner clone release returns zero. Exact per-fixture bytes in original-exact-budget.json. Layered68passed/2ignored; original-exact-budget.log. Borrowed source bytes and parser/internal allocator overhead are outside this output/profile accounting.


Public source-ownership follow-up: decode_image on all five original high-precision PSD/PSB files succeeds at exact source-file+RGBA+ICC credit and refuses one byte below with final0. File source credit is released before return; native words/float bits and ICC match independent references, clones keep onlyRGBA+ICC until last drop. Peaks/retained per source:26582/3272,21774/1040,21678/776,20986/640,21874/920bytes. Evidence: public-file-peak.log/json. This covers managed source/pixel/profile credit, not parser/CMS allocator total RSS.
